//! The build script for the database.
//!
//! This builds pgvector so that it can be embedded in the executable and installed into the
//! PostgreSQL server at run time. It is built here rather than downloaded because the only
//! prebuilt packages available are for PostgreSQL 16 from a repository which has not been updated
//! since 2024, and because building it is no more than the project already does for libpq.
#![allow(clippy::needless_return)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use postgresql_archive::blocking::{extract, get_archive};
use postgresql_archive::configuration::theseus::URL;
use postgresql_archive::{Version, VersionReq};

/// The directory of the pgvector source, which is a submodule.
const PGVECTOR_DIR: &str = "pgvector";

/// The environment variable which says which version of PostgreSQL to build against.
///
/// This is pinned in `.cargo/config.toml` so that the same archive is reused rather than resolved
/// over the network on every build.
const POSTGRESQL_VERSION_VAR: &str = "POSTGRESQL_VERSION";

fn main() {
    // The `embed_migrations!` macro reads the migrations at compile time, but proc macros cannot
    // tell Cargo what they read, so without this a changed migration does not trigger a rebuild.
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed={}", PGVECTOR_DIR);
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed={}", POSTGRESQL_VERSION_VAR);

    let out_dir: PathBuf = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set"));

    let postgresql_dir: PathBuf = install_postgresql(&out_dir);
    build_pgvector(&out_dir, &postgresql_dir);
}

/// Return the directory holding a PostgreSQL server to build against, installing it if needed.
///
/// Only the headers and the PGXS makefiles are wanted, but the archive is the only place they come
/// from. It is the same archive which the server itself is run from, so it is usually already
/// cached by the time this runs.
fn install_postgresql(out_dir: &Path) -> PathBuf {
    let postgresql_dir: PathBuf = out_dir.join("postgresql");

    if postgresql_dir.join("bin").join("pg_config").is_file() {
        return postgresql_dir;
    }

    let requirement: String = env::var(POSTGRESQL_VERSION_VAR).unwrap_or_else(|_| "*".to_string());
    let version_req: VersionReq = VersionReq::parse(&requirement).unwrap_or_else(|error| {
        panic!(
            "Failed to understand {}={}: {}",
            POSTGRESQL_VERSION_VAR, requirement, error
        )
    });

    // Cargo replays build script warnings on every build, so this is not one.
    let (_version, bytes): (Version, Vec<u8>) = get_archive(URL, &version_req)
        .unwrap_or_else(|error| panic!("Failed to download PostgreSQL: {}", error));

    extract(URL, &bytes, &postgresql_dir)
        .unwrap_or_else(|error| panic!("Failed to extract PostgreSQL: {}", error));

    return postgresql_dir;
}

/// Build pgvector and put what has to be installed alongside the server where the crate can embed
/// it.
fn build_pgvector(out_dir: &Path, postgresql_dir: &Path) {
    let pgvector_dir: &Path = Path::new(PGVECTOR_DIR);
    let makefile: PathBuf = pgvector_dir.join("Makefile");
    // A clone without `--recurse-submodules` leaves the directory empty, which would otherwise
    // fail further along as a missing file with nothing to say about why it is missing.
    if !makefile.exists() {
        panic!(
            "The pgvector source is not there. It is a submodule, so run `git submodule update \
             --init` to fetch it."
        );
    }

    let version: String = pgvector_version(&makefile);
    println!("cargo:rustc-env=PGVECTOR_VERSION={}", version);

    let source_dir: PathBuf = out_dir.join(PGVECTOR_DIR);

    // PGXS builds in the source directory, and Cargo does not allow a build script to write into
    // the crate, so the source is copied out first.
    let _ = fs::remove_dir_all(&source_dir);
    copy_dir(pgvector_dir, &source_dir);

    let pg_config: PathBuf = postgresql_dir.join("bin").join("pg_config");
    let mut command: Command = Command::new("make");
    command
        .arg("--directory")
        .arg(&source_dir)
        .arg(format!("PG_CONFIG={}", pg_config.display()))
        // pgvector builds with `-march=native` by default, which would bake the features of
        // whichever machine built it into the release and crash on an older one.
        .arg("OPTFLAGS=")
        // PGXS appends `$(PROFILE)` and `$(COPT)` to the compiler flags, and Cargo puts `PROFILE`
        // in the environment of every build script. Without these the value of that ends up on
        // the gcc command line as if it were a file to link.
        .arg("PROFILE=")
        .arg("COPT=");
    for flags in macos_flags(postgresql_dir) {
        command.arg(flags);
    }

    let output: Output = command.output().unwrap_or_else(|error| {
        panic!(
            "Failed to run make, which is needed to build pgvector: {}",
            error
        )
    });

    if !output.status.success() {
        panic!(
            "Failed to build pgvector:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // What the module is called is not the same everywhere: it is `vector.so` on Linux and
    // `vector.dylib` on macOS. The server looks for whichever of those its own build used, so the
    // name is taken from the makefiles it came with rather than guessed at, and passed along for
    // installing it under.
    let module: String = format!("vector{}", dlsuffix(postgresql_dir));
    println!("cargo:rustc-env=PGVECTOR_MODULE={}", module);

    for (from, to) in [
        // The module is embedded under a name which says nothing about the platform, so that what
        // embeds it does not have to know which one it was built on.
        (source_dir.join(&module), out_dir.join("vector.module")),
        (
            source_dir.join("vector.control"),
            out_dir.join("vector.control"),
        ),
        // The built statements are named after the version, but they are embedded under a name
        // which is not, so that what embeds them does not have to know the version at all.
        (
            source_dir
                .join("sql")
                .join(format!("vector--{}.sql", version)),
            out_dir.join("vector.sql"),
        ),
    ] {
        fs::copy(&from, &to)
            .unwrap_or_else(|error| panic!("Failed to copy {:?} to {:?}: {}", from, to, error));
    }
}

/// Return the suffix which a loadable module of the given PostgreSQL is named with.
///
/// This is `.so` on Linux and `.dylib` on macOS. It is read from the makefiles the server came
/// with rather than worked out from the platform, since it is the server which decides what to
/// look for and these are what it was built with.
fn dlsuffix(postgresql_dir: &Path) -> String {
    let makefile: PathBuf = postgresql_dir
        .join("lib")
        .join("pgxs")
        .join("src")
        .join("Makefile.global");

    let contents: String = fs::read_to_string(&makefile)
        .unwrap_or_else(|error| panic!("Failed to read {:?}: {}", makefile, error));

    for line in contents.lines() {
        if let Some(suffix) = line.strip_prefix("DLSUFFIX = ") {
            return suffix.trim().to_string();
        }
    }

    panic!(
        "Failed to find what a module is named with in {:?}.",
        makefile
    );
}

/// Return the flags to build with on macOS, and nothing at all anywhere else.
///
/// `configure` writes the SDK it was run against into both the preprocessor and the linker flags
/// of the PostgreSQL it builds, as `-isysroot`, and PGXS hands those along to anything built
/// against it. The archive was configured on somebody else's machine, so that SDK is not the one
/// here and usually is not there at all, which the compiler and the linker give up over rather
/// than falling back to one which is. The last `-isysroot` is the one clang takes and nothing
/// lands after these, so the only way to be rid of it is to replace the lot.
///
/// Little of theirs is worth keeping: the server headers are put back by PGXS itself, pgvector
/// adds what it needs through `PG_CFLAGS`, and the rest are the paths of libraries it does not
/// use. The one which does have to be put back by hand is where the server libraries are, since
/// that came in the flags which are being replaced. The SDK on this machine goes in their place,
/// or nothing at all when the developer tools cannot say where it is, which leaves clang to find
/// one on its own.
///
/// This is only done on macOS. Elsewhere the flags carry things which are needed, like
/// `-D_GNU_SOURCE` for the server headers and the rpath of the server libraries.
fn macos_flags(postgresql_dir: &Path) -> Vec<String> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }

    let output: Option<Output> = Command::new("xcrun").arg("--show-sdk-path").output().ok();
    let path: String = match output {
        Some(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => String::new(),
    };

    let sysroot: String = match path.is_empty() || !Path::new(&path).exists() {
        true => {
            println!("cargo:warning=Could not find the macOS SDK to build pgvector against.");
            String::new()
        }
        false => format!("-isysroot {}", path),
    };

    return vec![
        format!("CPPFLAGS={}", sysroot),
        format!(
            "LDFLAGS=-L{} {}",
            postgresql_dir.join("lib").display(),
            sysroot
        ),
    ];
}

/// Return the version of pgvector which the given makefile builds.
///
/// The submodule is the one place the version is written down, so it is read from there rather
/// than repeated here, where it could come to disagree with the source it is built from.
fn pgvector_version(makefile: &Path) -> String {
    let contents: String = fs::read_to_string(makefile)
        .unwrap_or_else(|error| panic!("Failed to read {:?}: {}", makefile, error));

    for line in contents.lines() {
        if let Some(version) = line.strip_prefix("EXTVERSION = ") {
            return version.trim().to_string();
        }
    }

    panic!("Failed to find the version of pgvector in {:?}.", makefile);
}

/// Copy a directory and everything in it.
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|error| panic!("Failed to create {:?}: {}", to, error));

    let entries =
        fs::read_dir(from).unwrap_or_else(|error| panic!("Failed to read {:?}: {}", from, error));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("Failed to read {:?}: {}", from, error));
        let source: PathBuf = entry.path();
        let destination: PathBuf = to.join(entry.file_name());

        if source.is_dir() {
            copy_dir(&source, &destination);
        } else {
            fs::copy(&source, &destination).unwrap_or_else(|error| {
                panic!(
                    "Failed to copy {:?} to {:?}: {}",
                    source, destination, error
                )
            });
        }
    }
}

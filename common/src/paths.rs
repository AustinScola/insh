use std::fs::{set_permissions, DirBuilder, Permissions};
use std::io::{Error as IOError, ErrorKind as IOErrorKind};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

lazy_static! {

    pub static ref HOME_DIR: PathBuf = {
        dirs::home_dir().expect("Could not determine home directory for user.")
    };

    /// The directory that insh related files are stored in for a user.
    pub static ref INSH_DIR: PathBuf = {
        let mut path = HOME_DIR.clone();
        path.push(".insh");
        path
    };

    /// The inshd directory.
    pub static ref INSHD_DIR: PathBuf = {
        let mut path = INSH_DIR.clone();
        path.push("daemon");
        path
    };

    /// The inshd socket file.
    pub static ref INSHD_SOCKET: PathBuf = {
        let mut path = INSHD_DIR.clone();
        path.push("inshd.sock");
        path
    };

    /// The directory that the embedded PostgreSQL database is stored in.
    ///
    /// The server binaries are installed in a directory named after their version in here, and the
    /// unix socket is created in here too.
    pub static ref INSHD_POSTGRES_DIR: PathBuf = {
        let mut path = INSHD_DIR.clone();
        path.push("postgres");
        path
    };

    /// The directory that the PostgreSQL data is stored in.
    pub static ref INSHD_POSTGRES_DATA_DIR: PathBuf = {
        let mut path = INSHD_POSTGRES_DIR.clone();
        path.push("data");
        path
    };

}

/// The permissions to use for the insh directories.
pub static INSH_DIRS_PERMS: u32 = 0o700; // rwx --- ---
/// The permissions to use for files in the insh directory.
pub static INSH_FILES_PERMS: u32 = 0o600; // rw- --- ---

/// Create a directory in the insh directory, and the directories above it, so that only the user
/// can use them.
///
/// Directories which already exist have their permissions corrected, because the daemon and the
/// things it depends on have created them with looser permissions in the past.
pub fn make_private_dir(dir: &Path) -> Result<(), IOError> {
    let relative: &Path = dir.strip_prefix(&*INSH_DIR).map_err(|_| {
        IOError::new(
            IOErrorKind::InvalidInput,
            format!("{:?} is not in the insh directory.", dir),
        )
    })?;

    // Walk down from the insh directory, so that a directory which is made along the way is not
    // left with looser permissions. Nothing above the insh directory is ever touched.
    let mut path: PathBuf = INSH_DIR.clone();
    make_one_private_dir(&path)?;
    for component in relative.components() {
        path.push(component);
        make_one_private_dir(&path)?;
    }

    Ok(())
}

/// Create a single directory which only the user can use, if it does not already exist, and take
/// away any permissions an existing one gives to the group or others.
fn make_one_private_dir(dir: &Path) -> Result<(), IOError> {
    if !dir.exists() {
        return DirBuilder::new().mode(INSH_DIRS_PERMS).create(dir);
    }

    if dir.metadata()?.permissions().mode() & 0o777 != INSH_DIRS_PERMS {
        set_permissions(dir, Permissions::from_mode(INSH_DIRS_PERMS))?;
    }

    Ok(())
}

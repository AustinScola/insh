//! The build script for inshd.

fn main() {
    // The `embed_migrations!` macro reads the migrations at compile time, but proc macros cannot
    // tell Cargo what they read, so without this a changed migration does not trigger a rebuild.
    println!("cargo:rerun-if-changed=migrations");
}

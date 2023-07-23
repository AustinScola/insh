use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;

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
}

/// The permissions to use for the insh directory.
static INSH_DIR_PERMS: u32 = 0o700; // rwx --- ---
/// The permissions to use for files in the insh directory.
pub static INSH_FILES_PERMS: u32 = 0o600; // rw- --- ---

/// Ensure that the Insh directory used for storing data exists.
pub fn ensure_insh_dir_exists() {
    if !INSH_DIR.exists() {
        // TODO: Should we have a umask?
        DirBuilder::new()
            .mode(INSH_DIR_PERMS)
            .create(&*INSH_DIR)
            .expect("Failed to create the insh directory.");
    }
}

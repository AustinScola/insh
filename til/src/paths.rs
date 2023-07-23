use std::path::PathBuf;

lazy_static! {

    pub static ref HOME_DIR: PathBuf = {
        dirs::home_dir().expect("Could not determine home directory for user.")
    };

    /// The directory that insh related files are stored in for a user.
    pub static ref TIL_DIR: PathBuf = {
        let mut path = HOME_DIR.clone();
        path.push(".til");
        path
    };
}

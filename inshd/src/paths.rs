//! Common paths.
use common::paths::{INSHD_DIR, INSH_DIR};
use std::path::PathBuf;

lazy_static! {
    /// The inshd pid file.
    pub static ref INSHD_PID_FILE: PathBuf = {
        let mut path = INSH_DIR.clone();
        path.push("inshd.pid");
        path
    };
    /// The directory inshd logs are stored in.
    pub static ref INSHD_LOGS_DIR: PathBuf = {
        let mut path: PathBuf = INSHD_DIR.clone();
        path.push("logs");
        path
    };
}

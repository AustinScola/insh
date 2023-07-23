/*!
Contains the [`Program`] [`Bash`].
*/
use std::ffi::OsString;
use std::path::PathBuf;

use til::{Program, ProgramCleanup, ProgramSetup};

/// A Bash program.
pub struct Bash {
    /// The starting working directory.
    directory: PathBuf,
}

impl Bash {
    /// Return a new Bash program.
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
}

impl Program for Bash {
    fn setup(&self) -> ProgramSetup {
        ProgramSetup {
            clear_screen: true,
            cursor_home: true,
            cursor_visible: Some(true),
        }
    }

    fn cleanup(&self) -> ProgramCleanup {
        ProgramCleanup {
            hide_cursor: true,
            enable_raw_terminal: true,
        }
    }

    fn filename(&self) -> OsString {
        "bash".into()
    }

    fn cwd(&self) -> Option<PathBuf> {
        Some(self.directory.clone())
    }
}

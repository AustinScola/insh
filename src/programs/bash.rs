use crate::program::{Program, ProgramCleanup, ProgramSetup};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct Bash {
    directory: PathBuf,
}

impl Bash {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
}

impl Program for Bash {
    fn setup(&self) -> ProgramSetup {
        ProgramSetup {
            raw_terminal: Some(true),
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

    fn run(&self) -> Command {
        let mut command = Command::new("bash");
        command
            .current_dir(self.directory.clone())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit());
        command
    }
}

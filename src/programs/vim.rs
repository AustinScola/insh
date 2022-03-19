use crate::program::{Program, ProgramCleanup};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct Vim {
    file: PathBuf,
}

impl Vim {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }
}

impl Program for Vim {
    fn cleanup(&self) -> ProgramCleanup {
        ProgramCleanup {
            hide_cursor: true,
            ..Default::default()
        }
    }

    fn run(&self) -> Command {
        let mut command = Command::new("vim");
        command
            .arg(self.file.clone())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit());
        command
    }
}

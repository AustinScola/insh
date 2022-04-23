use crate::program::{Program, ProgramCleanup};
use std::process::{Command, Stdio};

use std::path::{Path, PathBuf};

pub struct Vim {
    args: Args,
}

impl Vim {
    pub fn new(args: Args) -> Self {
        Self { args }
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

        if let Some(path) = self.args.path() {
            command.arg(path.clone());
        }

        if let Some(line_number) = self.args.line_number() {
            command.arg(format!("+{}", line_number));
        }

        command.stdin(Stdio::inherit()).stdout(Stdio::inherit());

        command
    }
}

pub struct Args {
    path: Option<PathBuf>,
    line_number: Option<usize>,
}

impl Args {
    pub fn path(&self) -> &Option<PathBuf> {
        &self.path
    }

    pub fn line_number(&self) -> Option<usize> {
        self.line_number
    }
}

#[derive(Default)]
pub struct ArgsBuilder {
    path: Option<PathBuf>,
    line_number: Option<usize>,
}

impl ArgsBuilder {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn path(mut self, path: &Path) -> Self {
        self.path = Some(path.to_path_buf());
        self
    }

    pub fn line_number(mut self, line_number: usize) -> Self {
        self.line_number = Some(line_number);
        self
    }

    pub fn build(&self) -> Args {
        Args {
            path: self.path.clone(),
            line_number: self.line_number,
        }
    }
}

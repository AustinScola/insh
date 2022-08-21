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

        if let Some(line) = self.args.line() {
            command.arg(format!("+{}", line));
        }

        if let Some(column) = self.args.column() {
            if column > 1 {
                command.arg("-c");
                command.arg(format!("norm {}l", column - 1));
            }
        }

        command.stdin(Stdio::inherit()).stdout(Stdio::inherit());

        command
    }
}

pub struct Args {
    path: Option<PathBuf>,
    line: Option<usize>,
    column: Option<usize>,
}

impl Args {
    pub fn path(&self) -> &Option<PathBuf> {
        &self.path
    }

    pub fn line(&self) -> Option<usize> {
        self.line
    }

    pub fn column(&self) -> Option<usize> {
        self.column
    }
}

#[derive(Default)]
pub struct ArgsBuilder {
    path: Option<PathBuf>,
    line: Option<usize>,
    column: Option<usize>,
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

    pub fn line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    pub fn build(&self) -> Args {
        Args {
            path: self.path.clone(),
            line: self.line,
            column: self.column,
        }
    }
}

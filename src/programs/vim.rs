/*!
Contains the [`Program`] [`Vim`].
*/

use crate::program::{Program, ProgramCleanup};
use std::process::{Command, Stdio};

use std::path::{Path, PathBuf};

/// The `vim` program.
pub struct Vim {
    /// Arguments for running `vim`.
    args: Args,
}

impl Vim {
    /// Return a new `vim` program.
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

/// Arguments for running `vim`.
pub struct Args {
    /// The path to open.
    path: Option<PathBuf>,
    /// The starting line number.
    line: Option<usize>,
    /// The starting column number.
    column: Option<usize>,
}

impl Args {
    /// Return the path to open.
    pub fn path(&self) -> &Option<PathBuf> {
        &self.path
    }

    /// Return the starting line number.
    pub fn line(&self) -> Option<usize> {
        self.line
    }

    /// Return the starting column number.
    pub fn column(&self) -> Option<usize> {
        self.column
    }
}

/// A builder for `vim` [`Args`].
#[derive(Default)]
pub struct ArgsBuilder {
    /// The path to open.
    path: Option<PathBuf>,
    /// The starting line number.
    line: Option<usize>,
    /// The starting column number.
    column: Option<usize>,
}

impl ArgsBuilder {
    /// Return a new `vim` arguments builder.
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Set the path that `vim` should open.
    pub fn path(mut self, path: &Path) -> Self {
        self.path = Some(path.to_path_buf());
        self
    }

    /// Set the starting line number.
    pub fn line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the starting column number.
    pub fn column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    /// Return arguments for running `vim`.
    pub fn build(&self) -> Args {
        Args {
            path: self.path.clone(),
            line: self.line,
            column: self.column,
        }
    }
}

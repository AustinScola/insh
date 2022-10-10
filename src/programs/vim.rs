/*!
Contains the [`Program`] [`Vim`].
*/

use crate::program::{Program, ProgramCleanup};
use crate::ansi_escaped_text::{self, ANSIEscapedText, ANSIEscapeCode};

use std::process::{Command, Stdio, ChildStdout, Child};
use std::path::{Path, PathBuf};
use std::io::{self, BufReader, Write, Read};

use combine::Parser;
use combine::parser::combinator::AnyPartialState;
use combine::parser::combinator::any_partial_state;
use combine::stream::{PartialStream};


#[cfg(feature = "logging")]
use log::debug;


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

    fn run(&self) {
        let mut command = Command::new("vim");

        // Tell vim that it's output is not a terminal so that it doesn't output a warning message.
        command.arg("--not-a-term");

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

        command.stdin(Stdio::inherit()).stdout(Stdio::piped());

        let mut child: Child = command.spawn().unwrap();

        let command_stdout: ChildStdout = child.stdout.take().unwrap();
        let mut reader: BufReader<ChildStdout> = BufReader::new(command_stdout);
        let mut buffer: [u8; 1] = [0; 1];

        let mut stdout = io::stdout().lock();
        let mut parser = any_partial_state(ansi_escaped_text::parser());
        let mut parser_state = AnyPartialState::default();
        let mut length: usize;
        loop {
            length = {
                reader.read(&mut buffer).unwrap()
            };
            if length == 0 {
                break;
            }

            #[cfg(feature = "logging")]
            debug!("Got output {:?} from vim.", &buffer[..length]);

            // Parse the text so that we can strip out the ANSI escape codes for enabling and
            // disabling the alternative screen.
            let mut stream = PartialStream(&buffer[..]);
            let parsed_text = parser.parse_with_state(&mut stream, &mut parser_state);
            match parsed_text {
                Ok(ansi_escaped_text) => {
                    match ansi_escaped_text {
                        ANSIEscapedText::ANSIEscapeCode(ansi_escape_code) => {
                            match ansi_escape_code {
                                ANSIEscapeCode::EnableAlternativeScreen => {
                                    #[cfg(feature = "logging")]
                                    debug!("Stripping enable alternative screen ANSI escape code from vim's output.");
                                },
                                ANSIEscapeCode::DisableAlternativeScreen => {
                                    #[cfg(feature = "logging")]
                                    debug!("Stripping disable alternative screen ANSI escape code from vim's output.");
                                }
                            }
                        },
                        ANSIEscapedText::Character(character) => {
                            stdout.write(&[character]).unwrap();
                            stdout.flush().unwrap();
                        }
                    };
                },
                #[cfg(feature = "logging")]
                Err(error) => {
                    error!("Failed to parse ANSI escaped text: {:?}", error);
                },
                #[cfg(not(feature = "logging"))]
                Err(_) => {},
            };
        }

        let _ = child.wait();
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

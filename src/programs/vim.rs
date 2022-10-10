/*!
Contains the [`Program`] [`Vim`].
*/

use crate::ansi_escaped_text::{self, ANSIEscapeCode, ANSIEscapedText};
use crate::program::{Program, ProgramCleanup};

use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

use nom::{Err as ParseError, IResult as ParseResult};

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

    /// Run the vim program.
    ///
    /// One important thing this does is strip the ANSI escape codes for enabling and disabling the
    /// the alternative screen from the output of vim. This is accomplished by parsing the output
    /// using `nom`. While `nom` allows for "streaming" it does not actually save the partial state
    /// which means we are re-parsing a lot. The `combine` crate does supposedly allow for saving
    /// the partial state which means each byte is only passed to the parser once, but I was unable
    /// to get code using it to compile. I made an GitHub issue to try to get some help but have
    /// yet to hear back (https://github.com/Marwes/combine/issues/354).
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

        #[cfg(feature = "logging")]
        log::debug!("Starting vim...");
        let mut child: Child = command.spawn().unwrap();
        #[cfg(feature = "logging")]
        log::debug!("Started vim.");

        let command_stdout: ChildStdout = child.stdout.take().unwrap();
        let mut reader: BufReader<ChildStdout> = BufReader::new(command_stdout);

        // This buffer is used to read one byte at a time from the stdout.
        let mut read_buffer: [u8; 1] = [0; 1];

        // This buffer is used to pass bytes to the parser.
        let mut buffer: Vec<u8> = vec![];

        let mut stdout = io::stdout().lock();
        let mut should_read: bool = true;
        loop {
            if should_read {
                let length: usize = reader.read(&mut read_buffer).unwrap();
                if length == 0 {
                    break;
                }
                buffer.extend_from_slice(&read_buffer);
            }

            let buffer_clone = buffer.clone();
            let parse_result: ParseResult<&[u8], ANSIEscapedText> =
                ansi_escaped_text::parser(&buffer_clone);
            match parse_result {
                Err(ParseError::Incomplete(_needed)) => {}
                #[cfg(feature = "logging")]
                Err(error) => {
                    #[cfg(feature = "logging")]
                    log::error!("Error while parsing stdout of vim: {}", error)
                }
                #[cfg(not(feature = "logging"))]
                Err(_) => {}
                Ok((remaining, ansi_escaped_text)) => {
                    match ansi_escaped_text {
                        ANSIEscapedText::ANSIEscapeCode(ansi_escape_code) => match ansi_escape_code
                        {
                            ANSIEscapeCode::EnableAlternativeScreen => {
                                #[cfg(feature = "logging")]
                                log::debug!("Stripping enable alternative screen ANSI escape code from vim's output.");
                            }
                            ANSIEscapeCode::DisableAlternativeScreen => {
                                #[cfg(feature = "logging")]
                                log::debug!("Stripping disable alternative screen ANSI escape code from vim's output.");
                            }
                        },
                        ANSIEscapedText::Character(character) => {
                            stdout.write_all(&[character]).unwrap();
                            // TODO: Can we figure out a way to not flush after every character but
                            // only flush when vim does? Maybe based on the output?
                            stdout.flush().unwrap();
                        }
                    };

                    buffer.clear();
                    if !remaining.is_empty() {
                        should_read = false;
                        buffer.extend_from_slice(remaining);
                    } else {
                        should_read = true;
                    }
                }
            }
        }

        let _ = child.wait();

        #[cfg(feature = "logging")]
        log::debug!("Finished running vim.");
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

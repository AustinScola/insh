use crate::programs::{Vim, VimArgs, VimArgsBuilder};
use crate::system_effect::SystemEffect;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub struct Args {
    /// Starting directory to run in
    #[clap(short, long, display_order = 0)]
    directory: Option<PathBuf>,

    /// Unix socket to write logs to
    #[cfg(feature = "logging")]
    #[clap(long = "log-file", display_order = 1)]
    pub log_file_path: Option<PathBuf>,

    #[clap(subcommand)]
    command: Option<Command>,
}

impl Args {
    pub fn directory(&self) -> &Option<PathBuf> {
        &self.directory
    }

    #[cfg(feature = "logging")]
    pub fn log_file_path(&self) -> &Option<PathBuf> {
        &self.log_file_path
    }

    pub fn command(&self) -> &Option<Command> {
        &self.command
    }

    pub fn starting_effects(&self) -> Option<Vec<SystemEffect>> {
        match &self.command {
            Some(Command::Edit {
                browse,
                file_line_column,
            }) => {
                let mut vim_args_builder = VimArgsBuilder::new();
                if let Some(file_line_column) = file_line_column {
                    if let Some(file) = file_line_column.file() {
                        vim_args_builder = vim_args_builder.path(file);
                    }
                    if let Some(line) = file_line_column.line() {
                        vim_args_builder = vim_args_builder.line(line);
                    }
                    if let Some(column) = file_line_column.column() {
                        vim_args_builder = vim_args_builder.column(column);
                    }
                }
                let vim_args: VimArgs = vim_args_builder.build();
                let program = Box::new(Vim::new(vim_args));
                let run_vim = SystemEffect::RunProgram { program };
                let mut effects: Vec<SystemEffect> = vec![run_vim];

                if !browse {
                    effects.push(SystemEffect::Exit)
                }
                Some(effects)
            }
            _ => None,
        }
    }
}

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    /// Browse a directory
    #[clap(alias = "b", display_order = 1)]
    Browse,

    /// Find files by name
    #[clap(alias = "f", display_order = 2)]
    Find { phrase: Option<String> },

    /// Search file contents
    #[clap(alias = "s", display_order = 3)]
    Search { phrase: Option<String> },

    /// Edit a file
    ///
    /// Edit a file using the editor if a file is provided or just open the editor if no file is
    /// provided.
    #[clap(alias = "e", display_order = 4, value_parser)]
    Edit {
        /// Open the browser afterwards
        ///
        /// The directory is the directory the file is in or if the global `directory` argument is
        /// provided, then it is used.
        #[clap(short, long)]
        browse: bool,

        /// The file to edit
        ///
        /// Of the form "<file>", "<file>:<line>" or "<file>:<line>,<column>".
        #[clap(name = "FILE")]
        file_line_column: Option<FileLineColumn>,
    },
}

mod file_line_column {
    use super::file_line_column_parse_error::FileLineColumnParseError;

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::path::PathBuf;
    use std::str::FromStr;

    /// A file, line, and column number. The line and column numbers are 1-based.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct FileLineColumn {
        file: Option<PathBuf>,
        line: Option<usize>,
        column: Option<usize>,
    }

    impl FileLineColumn {
        pub fn new(file: Option<PathBuf>, line: Option<usize>, column: Option<usize>) -> Self {
            Self { file, line, column }
        }

        pub fn file(&self) -> &Option<PathBuf> {
            &self.file
        }

        pub fn line(&self) -> Option<usize> {
            self.line
        }

        pub fn column(&self) -> Option<usize> {
            self.column
        }
    }

    impl Display for FileLineColumn {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
            if let Some(file) = &self.file {
                write!(f, "{}", file.display())?;
            } else {
                return Ok(());
            }

            if let Some(line) = self.line {
                write!(f, ":{}", line)?;
            }

            if let Some(column) = self.column {
                write!(f, ",{}", column)?;
            }
            Ok(())
        }
    }

    impl FromStr for FileLineColumn {
        type Err = FileLineColumnParseError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            if s.is_empty() {
                return Ok(FileLineColumn::default());
            }
            let file_string: &str;
            let line_and_maybe_column_string: Option<&str>;
            match s.rsplit_once(':') {
                Some((file_, line_and_maybe_column_string_)) => {
                    file_string = file_;
                    line_and_maybe_column_string = Some(line_and_maybe_column_string_);
                }
                None => {
                    file_string = s;
                    line_and_maybe_column_string = None;
                }
            }

            let line_string: Option<&str>;
            let column_string: Option<&str>;
            match line_and_maybe_column_string {
                Some(line_and_maybe_column_) => match line_and_maybe_column_.rsplit_once(',') {
                    Some((line_, column_)) => {
                        line_string = Some(line_);
                        column_string = Some(column_);
                    }
                    None => {
                        line_string = Some(line_and_maybe_column_);
                        column_string = None;
                    }
                },
                None => {
                    line_string = None;
                    column_string = None;
                }
            }

            let mut bad_file: Option<String> = None;
            let mut bad_line: Option<String> = None;
            let mut bad_column: Option<String> = None;

            let mut file: Option<PathBuf> = None;
            let mut line: Option<usize> = None;
            let mut column: Option<usize> = None;

            match PathBuf::try_from(file_string) {
                Ok(file_) => {
                    file = Some(file_);
                }
                Err(_) => {
                    bad_file = Some(file_string.into());
                }
            };

            if let Some(line_string_) = line_string {
                match usize::from_str(line_string_) {
                    Ok(line_) => {
                        line = Some(line_);
                    }
                    Err(_) => {
                        bad_line = Some(line_string_.into());
                    }
                };
            }

            if let Some(column_string_) = column_string {
                match usize::from_str(column_string_) {
                    Ok(column_) => {
                        column = Some(column_);
                    }
                    Err(_) => {
                        bad_column = Some(column_string_.into());
                    }
                };
            }

            if bad_file.is_some() || bad_line.is_some() || bad_column.is_some() {
                Err(FileLineColumnParseError::new(
                    bad_file, bad_line, bad_column,
                ))
            } else {
                Ok(FileLineColumn::new(file, line, column))
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use test_case::test_case;

        #[test_case("", Ok(FileLineColumn::new(None, None, None)); "when the string is empty")]
        #[test_case("foo.py", Ok(FileLineColumn::new(Some("foo.py".into()), None, None)); "file")]
        #[test_case("foo.py:xx", Err(FileLineColumnParseError::from_bad_line("xx".into())); "file and bad line")]
        #[test_case("foo.py:42", Ok(FileLineColumn::new(Some("foo.py".into()), Some(42), None)); "file and line")]
        #[test_case("foo.py:42,xx", Err(FileLineColumnParseError::from_bad_column("xx".into())); "file, line, and bad column")]
        #[test_case("foo.py:42,7", Ok(FileLineColumn::new(Some("foo.py".into()), Some(42), Some(7))); "file, line, and column")]
        #[test_case("foo.py:xx,yy", Err(FileLineColumnParseError::new(None, Some("xx".into()), Some("yy".into()))); "file, bad line, and column")]
        fn test_from_str(
            string: &str,
            expected_result: Result<FileLineColumn, FileLineColumnParseError>,
        ) {
            let result: Result<FileLineColumn, FileLineColumnParseError> =
                FileLineColumn::from_str(string);

            assert_eq!(result, expected_result)
        }
    }
}
pub use file_line_column::FileLineColumn;

mod file_line_column_parse_error {
    use crate::string::{capitalize_first_letter, conjoin};

    use std::error::Error;
    use std::fmt::{Display, Error as FmtError, Formatter};

    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct FileLineColumnParseError {
        bad_file: Option<String>,
        bad_line: Option<String>,
        bad_column: Option<String>,
    }

    impl FileLineColumnParseError {
        /// Return a new parse error.
        pub fn new(
            bad_file: Option<String>,
            bad_line: Option<String>,
            bad_column: Option<String>,
        ) -> Self {
            Self {
                bad_file,
                bad_line,
                bad_column,
            }
        }

        /// Return a parse error from a string that cannot be parsed as a file.
        #[allow(dead_code)]
        pub fn from_bad_file(bad_file: String) -> Self {
            Self {
                bad_file: Some(bad_file),
                ..Default::default()
            }
        }

        /// Return a parse error from a string that cannot be parsed as a line number.
        #[allow(dead_code)]
        pub fn from_bad_line(bad_line: String) -> Self {
            Self {
                bad_line: Some(bad_line),
                ..Default::default()
            }
        }

        /// Return a parse error from a string that cannot be parsed as a column number.
        #[allow(dead_code)]
        pub fn from_bad_column(bad_column: String) -> Self {
            Self {
                bad_column: Some(bad_column),
                ..Default::default()
            }
        }
    }

    impl Display for FileLineColumnParseError {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
            if self.bad_file.is_none() && self.bad_line.is_none() && self.bad_column.is_none() {
                write!(
                    f,
                    "Something went wrong parsing the file, optional line, and or the optional column."
                )?;
                return Ok(());
            }

            let mut problems: Vec<String> = vec![];
            if let Some(bad_file) = &self.bad_file {
                problems.push(format!("could not parse \"{}\" as a file path", bad_file));
            }
            if let Some(bad_line) = &self.bad_line {
                problems.push(format!("could not parse \"{}\" as a line number", bad_line));
            }
            if let Some(bad_column) = &self.bad_column {
                problems.push(format!(
                    "could not parse \"{}\" as a column number",
                    bad_column
                ));
            }

            problems[0] = capitalize_first_letter(&problems[0]);
            let message: String = conjoin(problems, "and");
            write!(f, "{}.", message)
        }
    }

    impl Error for FileLineColumnParseError {}
}

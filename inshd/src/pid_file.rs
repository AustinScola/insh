//! The file which inshd writes its process id to.
use std::fs::File;
use std::io::Read;

use crate::paths::INSHD_PID_FILE;

/// The file which inshd writes its process id to.
pub struct PidFile {}

impl PidFile {
    /// Return the process id of inshd.
    pub fn read() -> Result<u64, ReadError> {
        let mut file = match File::open(&*INSHD_PID_FILE) {
            Ok(file) => file,
            Err(error) => match error.kind() {
                std::io::ErrorKind::NotFound => {
                    return Err(ReadError::PidFileNotFound);
                }
                _ => return Err(ReadError::ErrorOpeningPidFile(error)),
            },
        };

        let mut contents: String = String::new();
        if let Err(error) = file.read_to_string(&mut contents) {
            return Err(ReadError::ErrorReadingPidFile(error));
        }

        let contents: &str = contents.trim_end();

        let pid: u64 = match contents.parse::<u64>() {
            Ok(pid) => pid,
            Err(error) => {
                return Err(ReadError::FailedToParsePid(error));
            }
        };

        Ok(pid)
    }
}

mod read_error {
    //! An error getting the process id of inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;
    use std::num::ParseIntError;

    /// An error getting the process id of inshd.
    pub enum ReadError {
        /// The pid file does not exist.
        PidFileNotFound,
        /// There was an error opening the pid file.
        ErrorOpeningPidFile(IOError),
        /// There was an error reading the pid file.
        ErrorReadingPidFile(IOError),
        /// Failed to parse the pid.
        FailedToParsePid(ParseIntError),
    }

    impl Display for ReadError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::PidFileNotFound => {
                    write!(formatter, "PID file not found.")
                }
                Self::ErrorOpeningPidFile(error) => {
                    write!(formatter, "Error opening PID file: {}", error)
                }
                Self::ErrorReadingPidFile(error) => {
                    write!(formatter, "Error reading PID file: {}", error)
                }
                Self::FailedToParsePid(error) => {
                    write!(formatter, "Failed to parse PID: {}", error)
                }
            }
        }
    }
}
pub use read_error::ReadError;

//! Checks the status of inshd.
use std::fmt::{Display, Error as FmtError, Formatter};

use crate::pid_file::{PidFile, ReadError as PidFileReadError};

/// The status of inshd.
pub enum Status {
    /// Inshd is running.
    Running {
        /// The pid of inshd.
        pid: u64,
    },
    /// Inshd is not running.
    NotRunning,
}

impl Status {
    /// Return the status of inshd.
    pub fn get() -> Result<Status, StatusError> {
        let pid: u64 = match PidFile::read() {
            Ok(pid) => pid,
            Err(error) => match error {
                PidFileReadError::PidFileNotFound => {
                    return Ok(Status::NotRunning);
                }
                _ => {
                    return Err(StatusError::FailedToGetPid(error));
                }
            },
        };

        Ok(Status::Running { pid })
    }
}

impl Display for Status {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::Running { pid } => {
                write!(formatter, "Status: Running (PID: {})", pid)
            }
            Self::NotRunning => {
                write!(formatter, "Status: Not running")
            }
        }
    }
}

mod status_error {
    //! An error getting the status of inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};

    use crate::pid_file::ReadError as PidFileReadError;

    /// An error getting the status of inshd.
    pub enum StatusError {
        /// Failed to get the pid of inshd.
        FailedToGetPid(PidFileReadError),
    }

    impl Display for StatusError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToGetPid(error) => {
                    write!(formatter, "Failed to get PID of daemon: {}", error)
                }
            }
        }
    }
}
use status_error::StatusError;

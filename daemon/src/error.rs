//! An error daemonizing the process.

use std::fmt::{Display, Error as FmtError, Formatter};
use std::io::Error as IOError;

use nix::errno::Errno;

/// An error daemonizing the process.
#[derive(Debug)]
pub enum Error {
    /// Failed to fork the process.
    Fork(Errno),
    /// Failed to detach from the controlling terminal.
    SetSid(Errno),
    /// Failed to change the working directory.
    Chdir(Errno),
    /// Failed to write the PID file.
    PidFile(IOError),
    /// Failed to redirect one of the standard streams to `/dev/null`.
    Redirect(IOError),
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::Fork(errno) => write!(formatter, "Failed to fork the process: {}", errno),
            Self::SetSid(errno) => write!(
                formatter,
                "Failed to detach from the controlling terminal: {}",
                errno
            ),
            Self::Chdir(errno) => write!(
                formatter,
                "Failed to change the working directory: {}",
                errno
            ),
            Self::PidFile(error) => write!(formatter, "Failed to write the PID file: {}", error),
            Self::Redirect(error) => write!(
                formatter,
                "Failed to redirect the standard streams: {}",
                error
            ),
        }
    }
}

impl std::error::Error for Error {}

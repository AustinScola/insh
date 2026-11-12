//! An error daemonizing the process.

use std::ffi::NulError;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::io::Error as IOError;

use nix::errno::Errno;

/// An error daemonizing the process.
#[derive(Debug)]
pub enum Error {
    /// The program or one of its arguments has a nul byte in it.
    Argument(NulError),
    /// Failed to fork the process.
    Fork(Errno),
    /// Failed to detach from the controlling terminal.
    SetSid(Errno),
    /// Failed to change the working directory.
    Chdir(Errno),
    /// Failed to redirect one of the standard streams to `/dev/null`.
    Redirect(IOError),
    /// Failed to run the program.
    Exec(Errno),
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::Argument(error) => {
                write!(
                    formatter,
                    "Failed to pass the program its arguments: {}",
                    error
                )
            }
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
            Self::Redirect(error) => write!(
                formatter,
                "Failed to redirect the standard streams: {}",
                error
            ),
            Self::Exec(errno) => write!(formatter, "Failed to run the program: {}", errno),
        }
    }
}

impl std::error::Error for Error {}

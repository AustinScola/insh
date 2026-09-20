//! An error streaming a reply.

use std::fmt::{Display, Error as FmtError, Formatter};
use std::io::Error as IoError;

/// An error streaming a reply.
#[derive(Debug)]
pub enum ChatError {
    /// The request to the inference engine could not be made.
    RequestFailed {
        /// The error which was encountered.
        error: String,
    },
    /// The inference engine responded with an error status.
    Status {
        /// The status code which was responded with.
        code: u16,
        /// The body which was responded with.
        body: String,
    },
    /// The body of the response could not be read.
    ReadFailed {
        /// The error which was encountered.
        error: IoError,
    },
    /// An event of the response could not be understood.
    ParseFailed {
        /// The error which was encountered.
        error: String,
    },
    /// The inference engine reported an error part way through the reply.
    Reported {
        /// What the inference engine said.
        message: String,
    },
}

impl Display for ChatError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::RequestFailed { error } => {
                write!(formatter, "Failed to request a reply: {}", error)
            }
            Self::Status { code, body } => {
                write!(
                    formatter,
                    "The inference engine responded with {}: {}",
                    code, body
                )
            }
            Self::ReadFailed { error } => {
                write!(formatter, "Failed to read the reply: {}", error)
            }
            Self::ParseFailed { error } => {
                write!(formatter, "Failed to parse the reply: {}", error)
            }
            Self::Reported { message } => {
                write!(formatter, "The inference engine reported: {}", message)
            }
        }
    }
}

//! An error searching or reading the web.

use std::fmt::{Display, Error as FmtError, Formatter};

/// An error searching or reading the web.
#[derive(Debug)]
pub enum WebError {
    /// The request to the provider could not be made.
    RequestFailed {
        /// The error which was encountered.
        error: String,
    },
    /// The provider responded with an error status.
    Status {
        /// The status code which was responded with.
        code: u16,
        /// The body which was responded with.
        body: String,
    },
    /// The response could not be understood.
    ParseFailed {
        /// The error which was encountered.
        error: String,
    },
    /// The provider could not read the page.
    Unreadable {
        /// Why, as the provider says it.
        reason: String,
    },
}

impl Display for WebError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::RequestFailed { error } => {
                write!(formatter, "Failed to reach the web provider: {}", error)
            }
            Self::Status { code, body } => {
                write!(
                    formatter,
                    "The web provider responded with {}: {}",
                    code, body
                )
            }
            Self::ParseFailed { error } => {
                write!(formatter, "Failed to parse the response: {}", error)
            }
            Self::Unreadable { reason } => {
                write!(formatter, "Failed to read the page: {}", reason)
            }
        }
    }
}

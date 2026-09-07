//! Writes requests to inshd.
use std::io::Write;
use std::os::unix::net::UnixStream;

use crate::LENGTH_SIZE;

use insh_api::Request;

use typed_builder::TypedBuilder;

/// Writes requests to inshd.
#[derive(TypedBuilder)]
pub struct RequestWriter {
    /// The socket which inshd is connected over.
    socket: UnixStream,
}

impl RequestWriter {
    /// Send a request.
    pub fn send(&mut self, request: &Request) -> Result<(), SendError> {
        let bytes: Vec<u8> = postcard::to_stdvec(request).map_err(SendError::FailedToSerialize)?;

        let length: u64 = bytes.len().try_into().unwrap();
        let length: [u8; LENGTH_SIZE] = length.to_be_bytes();

        self.socket
            .write_all(&length)
            .map_err(SendError::FailedToWrite)?;
        self.socket
            .write_all(&bytes)
            .map_err(SendError::FailedToWrite)?;

        return Ok(());
    }
}

mod send_error {
    //! An error sending a request to inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;

    use postcard::Error as PostcardError;

    /// An error sending a request to inshd.
    pub enum SendError {
        /// Failed to serialize the request.
        FailedToSerialize(PostcardError),
        /// Failed to write the request to the socket.
        FailedToWrite(IOError),
    }

    impl Display for SendError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToSerialize(error) => {
                    write!(formatter, "Failed to serialize a request: {}", error)
                }
                Self::FailedToWrite(error) => {
                    write!(formatter, "Failed to write a request: {}", error)
                }
            }
        }
    }
}
pub use send_error::SendError;

//! Reads responses from inshd.
use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;

use crate::LENGTH_SIZE;

use insh_api::Response;

use typed_builder::TypedBuilder;

/// Reads responses from inshd.
#[derive(TypedBuilder)]
pub struct ResponseReader {
    /// The socket which inshd is connected over.
    socket: UnixStream,
    /// A buffer which responses are read into.
    #[builder(default)]
    buffer: Vec<u8>,
}

impl ResponseReader {
    /// Return the next response from inshd.
    pub fn receive(&mut self) -> Result<Response, ReceiveError> {
        let mut length_buffer: [u8; LENGTH_SIZE] = [0; LENGTH_SIZE];
        Self::read_exact(&mut self.socket, &mut length_buffer)?;
        let length: usize = u64::from_be_bytes(length_buffer).try_into().unwrap();

        if self.buffer.len() < length {
            self.buffer.resize(length, 0);
        }
        Self::read_exact(&mut self.socket, &mut self.buffer[..length])?;

        let response: Response = postcard::from_bytes(&self.buffer[..length])
            .map_err(ReceiveError::FailedToDeserialize)?;

        return Ok(response);
    }

    /// Read from a socket until a buffer is filled.
    fn read_exact(socket: &mut UnixStream, buffer: &mut [u8]) -> Result<(), ReceiveError> {
        return socket.read_exact(buffer).map_err(|error| {
            // The end of the socket is reached when inshd disconnects, and also when the reader is
            // deliberately shut down from another thread.
            if error.kind() == ErrorKind::UnexpectedEof {
                ReceiveError::Disconnected
            } else {
                ReceiveError::FailedToRead(error)
            }
        });
    }
}

mod receive_error {
    //! An error receiving a response from inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;

    use postcard::Error as PostcardError;

    /// An error receiving a response from inshd.
    pub enum ReceiveError {
        /// Failed to read a response from the socket.
        FailedToRead(IOError),
        /// Failed to deserialize a response.
        FailedToDeserialize(PostcardError),
        /// Inshd disconnected.
        Disconnected,
    }

    impl Display for ReceiveError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToRead(error) => {
                    write!(formatter, "Failed to read a response: {}", error)
                }
                Self::FailedToDeserialize(error) => {
                    write!(formatter, "Failed to deserialize a response: {}", error)
                }
                Self::Disconnected => {
                    write!(formatter, "Disconnected from inshd.")
                }
            }
        }
    }
}
pub use receive_error::ReceiveError;

//! Connects to inshd and exchanges requests and responses with it.
use std::os::unix::net::UnixStream;

use crate::request_writer::{RequestWriter, SendError};
use crate::response_reader::{ReceiveError, ResponseReader};

use common::paths::INSHD_SOCKET;
use insh_api::{Request, Response};

use typed_builder::TypedBuilder;

/// A connection to inshd.
///
/// Sending and receiving both block, so this is for callers which have nothing else to do while
/// they wait. Insh drives the two directions on their own threads instead.
#[derive(TypedBuilder)]
pub struct InshdClient {
    /// Writes requests to inshd.
    writer: RequestWriter,
    /// Reads responses from inshd.
    reader: ResponseReader,
}

impl InshdClient {
    /// Connect to inshd.
    pub fn connect() -> Result<Self, ConnectError> {
        let socket: UnixStream =
            UnixStream::connect(&*INSHD_SOCKET).map_err(ConnectError::FailedToConnect)?;
        let clone: UnixStream = socket.try_clone().map_err(ConnectError::FailedToConnect)?;

        return Ok(Self::builder()
            .writer(RequestWriter::builder().socket(socket).build())
            .reader(ResponseReader::builder().socket(clone).build())
            .build());
    }

    /// Send a request to inshd.
    pub fn send(&mut self, request: &Request) -> Result<(), SendError> {
        return self.writer.send(request);
    }

    /// Return the next response from inshd.
    pub fn receive(&mut self) -> Result<Response, ReceiveError> {
        return self.reader.receive();
    }
}

mod connect_error {
    //! An error connecting to inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;

    /// An error connecting to inshd.
    pub enum ConnectError {
        /// Failed to connect to the socket.
        FailedToConnect(IOError),
    }

    impl Display for ConnectError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToConnect(error) => {
                    write!(
                        formatter,
                        "Failed to connect to inshd (is it running?): {}",
                        error
                    )
                }
            }
        }
    }
}
pub use connect_error::ConnectError;

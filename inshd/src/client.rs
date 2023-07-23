//! Information about a client.
use std::io::Error as IOError;
use std::os::unix::net::UnixStream;

use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Information about a client.
#[derive(TypedBuilder)]
pub struct Client {
    /// The UUID of the client.
    #[builder(default = Uuid::new_v4())]
    pub uuid: Uuid,
    /// The unix stream of the client.
    pub stream: UnixStream,
}

impl Client {
    /// Return the client UUID.
    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    /// Return the stream.
    pub fn stream(&mut self) -> &mut UnixStream {
        &mut self.stream
    }

    /// Try returning a clone of the client information.
    pub fn try_clone(&self) -> Result<Self, IOError> {
        Ok(Self {
            uuid: self.uuid,
            stream: self.stream.try_clone()?,
        })
    }
}

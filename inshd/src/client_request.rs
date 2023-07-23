//! Information about a request from a client.
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Information about a request from a client.
#[derive(TypedBuilder)]
pub struct ClientRequest {
    /// The client UUID.
    client_uuid: Uuid,
    /// The request UUID.
    request_uuid: Uuid,
}

impl ClientRequest {
    /// Return the client UUID.
    pub fn client_uuid(&self) -> &Uuid {
        &self.client_uuid
    }

    /// Return the request UUID.
    pub fn request_uuid(&self) -> &Uuid {
        &self.request_uuid
    }
}

//! Information about a client that has disconnected.
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Information about a client that has disconnected.
#[derive(TypedBuilder, Clone)]
pub struct DisconnectedClient {
    /// The UUID of the client.
    pub client_uuid: Uuid,
    /// The total number of requets the client made.
    pub num_requests: usize,
}

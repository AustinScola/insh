//! A subscription of a client to the logs of the daemon.
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// A subscription of a client to the logs of the daemon.
#[derive(TypedBuilder)]
pub struct LogSubscription {
    /// The UUID of the client which subscribed.
    client_uuid: Uuid,
    /// The UUID of the request to respond to.
    request_uuid: Uuid,
}

impl LogSubscription {
    /// Return the UUID of the client which subscribed.
    pub fn client_uuid(&self) -> &Uuid {
        &self.client_uuid
    }

    /// Return the UUID of the request to respond to.
    pub fn request_uuid(&self) -> &Uuid {
        &self.request_uuid
    }
}

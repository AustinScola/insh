//! A request along with the context it was made in.
use insh_api::Request;

use typed_builder::TypedBuilder;
use uuid::Uuid;

/// A request along with the context it was made in.
#[derive(TypedBuilder)]
pub struct ContextedRequest {
    /// The UUID of the client which made the request.
    client_uuid: Uuid,
    /// The request.
    request: Request,
}

impl ContextedRequest {
    /// Return the UUID of the client which made the request.
    pub fn client_uuid(&self) -> &Uuid {
        &self.client_uuid
    }

    /// Return the request.
    pub fn request(&self) -> &Request {
        &self.request
    }
}

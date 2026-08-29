//! A response along with the context it should be sent in.
use insh_api::Response;

use typed_builder::TypedBuilder;

/// A response along with the context it should be sent in.
#[derive(TypedBuilder)]
pub struct ContextedResponse {
    /// The response.
    response: Response,
    /// Whether or not handling the response should be logged.
    ///
    /// Responses which carry log records are not logged. If they were, then sending one would emit
    /// log records, which would be sent as more responses, which would emit more log records, and
    /// so on forever.
    #[builder(default = true)]
    log: bool,
}

impl ContextedResponse {
    /// Return the response.
    pub fn response(&self) -> &Response {
        &self.response
    }

    /// Return whether or not handling the response should be logged.
    pub fn log(&self) -> bool {
        self.log
    }
}

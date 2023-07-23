//! Information about a request handler dying.
use typed_builder::TypedBuilder;

/// Information about a request handler dying.
#[derive(TypedBuilder)]
pub struct RequestHandlerDied {
    /// The request handler number
    pub number: usize,
}

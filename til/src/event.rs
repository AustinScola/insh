//! Component events.

use term::TermEvent;

/// A component event.
pub enum Event<Response> {
    /// A terminal event.
    TermEvent(TermEvent),
    /// A response.
    Response(Response),
}

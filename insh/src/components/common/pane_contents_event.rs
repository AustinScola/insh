//! Contains the [`PaneContentsEvent`] enum.

use insh_api::Response;
use term::TermEvent;

/// An event for one of the components which a pane shows.
pub enum PaneContentsEvent {
    /// Whether what is typed goes to the pane has changed.
    SetFocus {
        /// Whether it does.
        focused: bool,
    },
    /// A response.
    Response(Response),
    /// A terminal event.
    TermEvent(TermEvent),
}

impl From<til::Event<Response>> for PaneContentsEvent {
    fn from(event: til::Event<Response>) -> Self {
        match event {
            til::Event::Response(response) => Self::Response(response),
            til::Event::TermEvent(term_event) => Self::TermEvent(term_event),
        }
    }
}

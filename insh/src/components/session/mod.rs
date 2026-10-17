//! The panes and windows of a session, which the daemon owns and this shows.

mod borders;
mod copying;
mod pane;
mod prompt;
mod session;
mod terminal;
mod tree;

pub use session::{Effect, Event, Props, Session};

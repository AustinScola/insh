//! Contains the [`Hover`] enum.

/// How much of the row the mouse is over is grayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover {
    /// The whole of it, which is what a bar is.
    Row,
    /// Only what is written in it, which is what a row of a list is.
    Text,
}

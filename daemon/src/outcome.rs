//! The outcome of daemonizing.

/// Which side of the fork the calling code is running on.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The original process. The daemon has been started in the background.
    Parent,
    /// The daemon process.
    Child,
}

//! Used as a sentinel value for threads to stop.

/// Used as a sentinel value for threads to stop.
pub struct Stop {}

impl Stop {
    /// Return a new stop sentinel.
    pub fn new() -> Self {
        Self {}
    }
}

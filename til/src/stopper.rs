//! Stops something.

/// Stops something.
pub trait Stopper {
    /// Stop it.
    fn stop(&mut self) {}
}

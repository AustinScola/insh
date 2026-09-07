//! Handles responses.

use crossbeam::channel::Sender;

/// Handles responses.
pub trait ResponseHandler<Response>: Send {
    /// Handle responses.
    fn run(&mut self, response_tx: Sender<Response>);
}

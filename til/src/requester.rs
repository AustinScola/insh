//! Makes requests.

use crossbeam::channel::Receiver;

/// Makes requests.
pub trait Requester<Request>: Send {
    /// Make requests.
    fn run(&mut self, request_rx: Receiver<Request>);
}

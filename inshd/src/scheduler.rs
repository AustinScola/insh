//! Schedules requests.
use crate::contexted_request::ContextedRequest;
use crate::stop::Stop;

use crossbeam::channel::{Receiver, Sender};
use crossbeam::select;
use typed_builder::TypedBuilder;

/// Schedules requests.
#[derive(TypedBuilder)]
pub struct Scheduler {
    /// The number of request handlers.
    num_request_handlers: usize,
    /// Channels for sending requests to each request handler.
    contexted_requests_txs: Vec<Sender<ContextedRequest>>,
    /// Incoming requests from client handlers.
    contexted_requests_rx: Receiver<ContextedRequest>,
    /// A receiver for a stop sentinel.
    stop: Receiver<Stop>,
}

impl Scheduler {
    /// Run the scheduler.
    pub fn run(&mut self) {
        log::info!("Scheduler running.");

        // Round-robin for now :)
        let mut current_request_handler: usize = 0;
        loop {
            select! {
                recv(self.stop) -> _stop => {
                    log::debug!("Recieved stop.");
                    break;
                }
                recv(self.contexted_requests_rx) -> request => {
                    let request: ContextedRequest = match request {
                        Ok(request) => request,
                        Err(_) => {
                            log::warn!("Error receiving incoming request.");
                            break;
                        }
                    };

                    log::debug!(
                        "Scheduling request with request handler {}.",
                        current_request_handler
                    );
                    let contexted_requests_tx: &Sender<ContextedRequest> = &self.contexted_requests_txs[current_request_handler];
                    contexted_requests_tx.send(request).unwrap();
                    current_request_handler = (current_request_handler + 1) % self.num_request_handlers;
                }
            }
        }

        log::info!("Scheduler stopping...");
    }
}

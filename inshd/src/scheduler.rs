//! Schedules requests.
use crate::stop::Stop;
use insh_api::Request;

use crossbeam::channel::{Receiver, Sender};
use crossbeam::select;
use typed_builder::TypedBuilder;

/// Schedules requests.
#[derive(TypedBuilder)]
pub struct Scheduler {
    /// The number of request handlers.
    num_request_handlers: usize,
    /// Channels for sending requests to each request handler.
    requests_txs: Vec<Sender<Request>>,
    /// Incoming requests from client handlers.
    incoming_requests_rx: Receiver<Request>,
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
                recv(self.incoming_requests_rx) -> request => {
                    let request: Request = match request {
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
                    let requests_tx: &Sender<Request> = &self.requests_txs[current_request_handler];
                    requests_tx.send(request).unwrap();
                    current_request_handler = (current_request_handler + 1) % self.num_request_handlers;
                }
            }
        }

        log::info!("Scheduler stopping...");
    }
}

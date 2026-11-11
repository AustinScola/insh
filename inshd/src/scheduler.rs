//! Schedules requests.
use crate::contexted_request::ContextedRequest;
use crate::stop::Stop;

use insh_api::RequestParams;

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
    /// A sender of the requests which are about panes, windows and sessions.
    multiplexer_requests_tx: Sender<ContextedRequest>,
    /// A receiver for a stop sentinel.
    stop: Receiver<Stop>,
}

impl Scheduler {
    /// Return whether the request is one which the multiplexer answers.
    ///
    /// NOTE: These are routed from here rather than from a request handler because the handlers
    /// are a pool which requests are shared out around. Two of them handling what was typed would
    /// race, and the characters would reach the program in whichever order the two threads
    /// happened to run in. There is only one scheduler, and it takes requests in the order they
    /// arrived, so sending them on from here is what keeps them in that order.
    fn multiplexes(params: &RequestParams) -> bool {
        return matches!(
            params,
            RequestParams::Attach(_)
                | RequestParams::Detach(_)
                | RequestParams::WritePane(_)
                | RequestParams::SplitPane(_)
                | RequestParams::SelectPane(_)
                | RequestParams::KillPane(_)
                | RequestParams::RunInPane(_)
                | RequestParams::DragPane(_)
                | RequestParams::ZoomPane(_)
                | RequestParams::SyncPanes(_)
                | RequestParams::RenameSession(_)
                | RequestParams::RenameWindow(_)
                | RequestParams::SetDir(_)
                | RequestParams::ScrollPane(_)
                | RequestParams::CopyPane(_)
                | RequestParams::SearchPane(_)
                | RequestParams::SwapPane(_)
                | RequestParams::NewWindow(_)
                | RequestParams::SelectWindow(_)
                | RequestParams::NewSession(_)
                | RequestParams::ListSessions(_)
                | RequestParams::ResizeClient(_)
        );
    }

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

                    if Self::multiplexes(request.request().params()) {
                        if let Err(error) = self.multiplexer_requests_tx.send(request) {
                            log::error!("Failed to hand a request to the multiplexer: {}", error);
                        }
                        continue;
                    }

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

#[cfg(test)]
mod tests {
    use super::*;

    use insh_api::SearchPaneRequestParams;
    use rend::Location;
    use uuid::Uuid;

    #[test]
    fn test_looking_for_something_in_a_pane_goes_to_the_multiplexer() {
        let params = RequestParams::SearchPane(
            SearchPaneRequestParams::builder()
                .pane(Uuid::from_u128(1))
                .phrase(String::from("x"))
                .from(Location::default())
                .down(true)
                .build(),
        );

        assert!(Scheduler::multiplexes(&params));
    }
}

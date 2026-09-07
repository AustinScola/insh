//! Sends requests to inshd.

use insh_api::Request;
use inshd_client::RequestWriter;
use til::Requester;

use crossbeam::channel::Receiver;
use typed_builder::TypedBuilder;

/// Sends requests to inshd.
#[derive(TypedBuilder)]
pub struct InshdRequester {
    /// The request writer.
    writer: RequestWriter,
}

impl Requester<Request> for InshdRequester {
    fn run(&mut self, request_rx: Receiver<Request>) {
        #[cfg(feature = "logging")]
        log::info!("Requester running.");

        loop {
            // Get a request from the channel.
            #[cfg(feature = "logging")]
            log::debug!("Waiting for a request...");
            let request = match request_rx.recv() {
                Ok(request) => request,
                Err(_) => {
                    #[cfg(feature = "logging")]
                    log::debug!("Stopping...");
                    break;
                }
            };
            #[cfg(feature = "logging")]
            log::debug!("Received request {}.", request.uuid());

            // Send the request to inshd.
            #[cfg(feature = "logging")]
            log::debug!("Sending the request...");
            if let Err(error) = self.writer.send(&request) {
                // NOTE: This panics rather than just logging because `log` is only compiled in
                // under the logging feature. Returning here would lose the error in a normal
                // build, and the only sign of it would be til panicking on the request channel the
                // next time something asks inshd for anything.
                panic!("{}", error);
            }
            #[cfg(feature = "logging")]
            log::debug!("Sent the request.");
        }

        #[cfg(feature = "logging")]
        log::info!("Requester stopping...");
    }
}

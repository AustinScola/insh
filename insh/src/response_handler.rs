use insh_api::Response;
use inshd_client::{ReceiveError, ResponseReader};
use til::{ResponseHandler, Stopper};

use std::net::Shutdown;
use std::os::unix::net::UnixStream;

use crossbeam::channel::Sender;
use typed_builder::TypedBuilder;
#[cfg(feature = "logging")]
use uuid::Uuid;

#[derive(TypedBuilder)]
pub struct InshdResponseHandler {
    reader: ResponseReader,
}

impl ResponseHandler<Response> for InshdResponseHandler {
    fn run(&mut self, response_tx: Sender<Response>) {
        #[cfg(feature = "logging")]
        log::info!("Response handler running.");

        loop {
            #[cfg(feature = "logging")]
            log::debug!("Waiting for a response...");

            let response: Response = match self.reader.receive() {
                Ok(response) => response,
                #[allow(unused_variables)]
                Err(error) => {
                    match error {
                        // NOTE: We can get here if either inshd disconnects or when til calls
                        // the response handler stopper which shutsdown the socket.
                        ReceiveError::Disconnected => {
                            #[cfg(feature = "logging")]
                            log::warn!("Disconnected from inshd.");
                        }
                        _ => {
                            #[cfg(feature = "logging")]
                            log::error!("{}", error);
                        }
                    }
                    break;
                }
            };
            #[cfg(feature = "logging")]
            {
                let response_uuid: Uuid = response.uuid().clone();
                log::debug!("Received response {:?}.", response_uuid);
            }

            // Send the response to TIL.
            if response_tx.send(response).is_err() {
                #[cfg(feature = "logging")]
                log::info!("Responses channel closed.");
                break;
            }
        }

        #[cfg(feature = "logging")]
        log::info!("Response handler stopping...")
    }
}

#[derive(TypedBuilder)]
pub struct InshdResponseHandlerStopper {
    socket: UnixStream,
}

impl Stopper for InshdResponseHandlerStopper {
    fn stop(&mut self) {
        self.socket.shutdown(Shutdown::Read).unwrap();
    }
}

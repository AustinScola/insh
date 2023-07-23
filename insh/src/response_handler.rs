use insh_api::Response;
use til::{ResponseHandler, Stopper};

use std::io::{ErrorKind as IOErrorKind, Read};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

use crossbeam::channel::Sender;
use typed_builder::TypedBuilder;
#[cfg(feature = "logging")]
use uuid::Uuid;

#[derive(TypedBuilder)]
pub struct InshdResponseHandler {
    socket: UnixStream,
}

impl ResponseHandler<Response> for InshdResponseHandler {
    fn run(&mut self, response_tx: Sender<Response>) {
        #[cfg(feature = "logging")]
        log::info!("Response handler running.");

        let mut length_buffer: [u8; 8] = [0; 8];
        let mut response_buffer: Vec<u8> = vec![];

        loop {
            #[cfg(feature = "logging")]
            log::debug!("Waiting for a response...");

            // Read the length of the response.
            if let Err(error) = self.socket.read_exact(&mut length_buffer) {
                match error.kind() {
                    IOErrorKind::UnexpectedEof => {
                        // NOTE: We can get here if either inshd disconnects or when til calls
                        // the response handler stopper which shutsdown the socket.
                        #[cfg(feature = "logging")]
                        log::warn!("Disconnected from inshd.");
                        break;
                    }
                    _ => {
                        #[cfg(feature = "logging")]
                        log::error!("Encountered an error reading the request length: {}", error);
                        break;
                    }
                }
            }
            let length: u64 = u64::from_be_bytes(length_buffer);
            #[cfg(feature = "logging")]
            log::debug!("The response is {} bytes long.", length);

            // Reserve more space in the response buffer if necessary.
            let length: usize = length.try_into().unwrap();
            #[cfg(feature = "logging")]
            log::debug!("Checking the capacity of the response buffer...");
            let capacity: usize = response_buffer.capacity();
            #[cfg(feature = "logging")]
            log::debug!("The response buffer has a capacity of {}.", capacity);
            if capacity < length {
                let reserve: usize = length - capacity;
                #[cfg(feature = "logging")]
                log::debug!("Reserving {} more bytes in the response buffer.", reserve);
                response_buffer.reserve_exact(reserve);
                response_buffer.resize(length, 0);
            } else {
                #[cfg(feature = "logging")]
                log::debug!("The response buffer has enough capacity to read the response.");
            }

            // Read the response.
            #[cfg(feature = "logging")]
            log::debug!("Reading the response...");
            if let Err(error) = self.socket.read_exact(&mut response_buffer[..length]) {
                match error.kind() {
                    IOErrorKind::UnexpectedEof => {
                        // NOTE: We can get here if either inshd disconnects or when til calls
                        // the response handler stopper which shutsdown the socket.
                        #[cfg(feature = "logging")]
                        log::warn!("Disconnected from inshd.");
                        break;
                    }
                    _ => {
                        #[cfg(feature = "logging")]
                        log::error!(
                            "Encountered an error reading the response buffer: {}",
                            error
                        );
                        break;
                    }
                }
            }
            #[cfg(feature = "logging")]
            log::debug!("Read the response.");

            // Deserialize the response.
            let response: Response = bincode::deserialize(&response_buffer[..length]).unwrap();
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

use insh_api::Request;
use til::Requester;

use std::io::Write;
use std::os::unix::net::UnixStream;

use crossbeam::channel::Receiver;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct InshdRequester {
    socket: UnixStream,
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

            // Serialize the request.
            #[cfg(feature = "logging")]
            log::debug!("Serializing the request...");
            let bytes: Vec<u8> = match bincode::serialize(&request) {
                Ok(bytes) => bytes,
                #[allow(unused_variables)]
                Err(error) => {
                    #[cfg(feature = "logging")]
                    log::error!("Failed to serialize a request: {}", error);
                    return;
                }
            };
            #[cfg(feature = "logging")]
            log::debug!("Serialized the request.");

            // Determine how many bytes long the request is.
            let length: u64 = bytes.len().try_into().unwrap();
            #[cfg(feature = "logging")]
            log::debug!("The request is {} bytes long", length);
            let length: [u8; 8] = length.to_be_bytes();

            // Write the length of the request to the socket.
            #[cfg(feature = "logging")]
            log::debug!("Writing length to socket...");
            let _ = self.socket.write(&length).unwrap();
            #[cfg(feature = "logging")]
            log::debug!("Wrote length to socket...");

            // Write the serialzed request to the socket.
            #[cfg(feature = "logging")]
            log::debug!("Writing request to socket...");
            let _ = self.socket.write(&bytes).unwrap();
            #[cfg(feature = "logging")]
            log::debug!("Wrote request to socket.");
        }

        #[cfg(feature = "logging")]
        log::info!("Requester stopping...");
    }
}

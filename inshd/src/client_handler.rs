//! Handles a client.
use crate::client::Client;
use crate::client_request::ClientRequest;
use crate::disconnected_client::DisconnectedClient;

use insh_api::Request;

use std::io::{ErrorKind as IOErrorKind, Read};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;

use crossbeam::channel::Sender;
use nix::sys::select::select;
use nix::sys::select::FdSet;
use os_pipe::PipeReader;
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Handles a client.
#[derive(TypedBuilder)]
pub struct ClientHandler {
    /// Information about the client.
    client: Client,
    /// A sender for requests the from the client.
    requests: Sender<Request>,
    /// A sender of information about requests from the client.
    client_requests_tx: Sender<ClientRequest>,
    /// Senders of information about the client disconnecting.
    disconnected_clients_txs: Vec<Sender<DisconnectedClient>>,
    /// The read side of a pipe for a stop sentinel.
    stop_rx: PipeReader,
}

impl ClientHandler {
    /// Run the client handler.
    pub fn run(&mut self) {
        let client_uuid: Uuid = *self.client.uuid();
        log::info!("Client handler running for client {}.", client_uuid);

        let mut num_requests: usize = 0;

        let mut length_buffer: [u8; 8] = [0; 8];
        let mut request_buffer: Vec<u8> = vec![];

        let stream: &mut UnixStream = self.client.stream();
        let stop_rx_fd: RawFd = self.stop_rx.as_raw_fd();

        loop {
            let nfds = None;
            let mut read_fds = FdSet::new();
            read_fds.insert(stream.as_raw_fd());
            read_fds.insert(stop_rx_fd);
            let write_fds = None;
            let error_fds = None;
            let timeout = None;
            select(nfds, &mut read_fds, write_fds, error_fds, timeout).unwrap();

            if read_fds.contains(stop_rx_fd) {
                break;
            }

            // Get the length of the request.
            if let Err(error) = stream.read_exact(&mut length_buffer) {
                match error.kind() {
                    IOErrorKind::UnexpectedEof => {
                        log::info!("Client {} disconnected.", client_uuid);
                        break;
                    }
                    _ => {
                        log::error!("Encountered an error reading the request length: {}", error);
                        break;
                    }
                }
            }
            let length: u64 = u64::from_be_bytes(length_buffer);
            log::debug!("The request is {} bytes long.", length);

            // Reserve more space in the request buffer if necessary.
            let length: usize = length.try_into().unwrap();
            log::debug!("Checking the capacity of the request buffer...");
            let capacity: usize = request_buffer.capacity();
            log::debug!("The request buffer has a capacity of {}.", capacity);
            if capacity < length {
                let reserve: usize = length - capacity;
                log::debug!("Reserving {} more bytes in the request buffer.", reserve);
                request_buffer.reserve_exact(reserve);
                request_buffer.resize(length, 0);
            } else {
                log::debug!("The request buffer has enough capacity to read the request.");
            }

            // Read the request.
            log::debug!("Reading the request...");
            if let Err(error) = stream.read_exact(&mut request_buffer[..length]) {
                match error.kind() {
                    IOErrorKind::UnexpectedEof => {
                        log::info!("Client {} disconnected.", client_uuid);
                        break;
                    }
                    _ => {
                        log::error!("Encountered an error reading the request buffer: {}", error);
                        break;
                    }
                }
            }
            log::debug!("Read the request.");

            // Deserialize the request.
            let request: Request = bincode::deserialize(&request_buffer[..length]).unwrap();
            let request_uuid: Uuid = *request.uuid();
            log::debug!("Received request {:?}.", request_uuid);

            // Send the request to the scheduler.
            self.requests.send(request).unwrap();

            num_requests += 1;

            // Inform the response handler that the request is for this client.
            let client_request: ClientRequest = ClientRequest::builder()
                .client_uuid(client_uuid)
                .request_uuid(request_uuid)
                .build();
            self.client_requests_tx.send(client_request).unwrap();
        }

        log::info!("Client handler stopping for client {}...", client_uuid);
        let disconnected_client = DisconnectedClient::builder()
            .client_uuid(client_uuid)
            .num_requests(num_requests)
            .build();
        for disconnected_clients_tx in &self.disconnected_clients_txs {
            disconnected_clients_tx
                .send(disconnected_client.clone())
                .unwrap();
        }
    }
}

//! Handles incoming connections on the socket.
use crate::client::Client;
use crate::client_handler::ClientHandler;
use crate::client_handler_handle::ClientHandlerHandle;
use crate::client_request::ClientRequest;
use crate::disconnected_client::DisconnectedClient;
use insh_api::Request;

use std::io::Result as IOResult;
use std::os::fd::AsRawFd;
use std::os::fd::RawFd;
use std::os::unix::net::Incoming;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::thread;
use std::thread::JoinHandle;

use crossbeam::channel::Sender;
use nix::sys::select::select;
use nix::sys::select::FdSet;
use os_pipe::PipeReader;
use typed_builder::TypedBuilder;

/// Handles incoming connections on the socket.
#[derive(TypedBuilder)]
pub struct ConnHandler {
    /// A unix socket listener.
    listener: UnixListener,
    /// A sender of information about a client.
    new_clients_tx: Sender<Client>,
    /// A sender of incoming requests (from clients).
    incoming_requests_tx: Sender<Request>,
    /// A Sender of client requests.
    client_requests_tx: Sender<ClientRequest>,
    /// Senders of disconnected client uuids.
    disconnected_clients_txs: Vec<Sender<DisconnectedClient>>,
    /// A sender of client handler thread handles.
    client_handler_handles_tx: Sender<ClientHandlerHandle>,
    /// A receiver of a stop sentinel.
    stop_rx: PipeReader,
}

impl ConnHandler {
    /// Run the connection handler.
    pub fn run(&mut self) {
        log::info!("Accepting connections...");
        let mut client_num: usize = 0;

        let listener_fd: RawFd = self.listener.as_raw_fd();
        let stop_rx_fd = self.stop_rx.as_raw_fd();
        loop {
            let nfds = None;
            let mut readfds = FdSet::new();
            readfds.insert(listener_fd);
            readfds.insert(stop_rx_fd);
            let writefds = None;
            let errorfds = None;
            let timeout = None;
            select(nfds, &mut readfds, writefds, errorfds, timeout).unwrap();

            if readfds.contains(stop_rx_fd) {
                // Don't bother reading from the pipe.
                break;
            }

            if readfds.contains(listener_fd) {
                let mut incoming: Incoming = self.listener.incoming();
                let stream: Option<IOResult<UnixStream>> = incoming.next();
                let stream: IOResult<UnixStream> = match stream {
                    Some(stream) => stream,
                    None => {
                        log::warn!("No incoming connection?");
                        continue;
                    }
                };
                let stream: UnixStream = match stream {
                    Ok(stream) => stream,
                    Err(error) => {
                        log::error!("Error with new connection: {}", error);
                        continue;
                    }
                };

                log::info!("Accepted a new connection.");

                let client: Client = Client::builder().stream(stream).build();
                log::info!("New client {}.", client.uuid());
                let requests: Sender<Request> = self.incoming_requests_tx.clone();
                let (stop_rx, stop_tx) = os_pipe::pipe().unwrap();
                let mut client_handler: ClientHandler = ClientHandler::builder()
                    .client(client.try_clone().unwrap())
                    .requests(requests)
                    .client_requests_tx(self.client_requests_tx.clone())
                    .disconnected_clients_txs(self.disconnected_clients_txs.clone())
                    .stop_rx(stop_rx)
                    .build();
                let name: String = format!("client-handler-{}", client_num).to_string();
                let handle: JoinHandle<()> = thread::Builder::new()
                    .name(name)
                    .spawn(move || client_handler.run())
                    .unwrap();

                // Send the client handler handle to the client handler monitor thread.
                let client_handler_handle: ClientHandlerHandle = ClientHandlerHandle::builder()
                    .client(*client.uuid())
                    .handle(handle)
                    .stop_tx(stop_tx)
                    .build();
                self.client_handler_handles_tx
                    .send(client_handler_handle)
                    .unwrap();

                // Inform the response handler thread of the new client.
                self.new_clients_tx.send(client).unwrap();
                client_num += 1;
            }
        }

        log::info!("Connection handler stopping...");
    }
}

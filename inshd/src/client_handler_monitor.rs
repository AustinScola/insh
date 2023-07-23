//! Monitors client handler threads.
use crate::client_handler_handle::ClientHandlerHandle;
use crate::disconnected_client::DisconnectedClient;
use crate::stop::Stop;

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::thread::JoinHandle;

use crossbeam::channel::Receiver;
use crossbeam::select;
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Monitors client handler threads.
#[derive(TypedBuilder)]
pub struct ClientHandlerMonitor {
    /// A receiver of handles to client handlers.
    client_handler_handles_rx: Receiver<ClientHandlerHandle>,
    /// A receiver of the uuids of disconnected client.
    disconnected_clients_rx: Receiver<DisconnectedClient>,
    /// A receiver of a stop sentinel.
    stop_rx: Receiver<Stop>,
}

impl ClientHandlerMonitor {
    /// Run the client handler monitor.
    pub fn run(&mut self) {
        log::info!("Client handler monitor running...");

        let mut client_uuid_to_handle: HashMap<Uuid, ClientHandlerHandle> = HashMap::new();
        let mut pending_disconnected_clients: HashSet<Uuid> = HashSet::new();

        loop {
            select! {
                recv(self.stop_rx) -> _stop => {
                    break;
                },
                recv(self.client_handler_handles_rx) -> client_handler_handle => {
                    let client_handler_handle: ClientHandlerHandle = match client_handler_handle {
                        Ok(client_handler_handle) => client_handler_handle,
                        Err(error) => {
                            log::error!("Error receiving client handler handle: {}", error);
                            continue
                        }
                    };

                    if pending_disconnected_clients.remove(&client_handler_handle.client) {
                        ClientHandlerMonitor::join_client_handler(&client_handler_handle.client, client_handler_handle.handle);
                        continue;
                    }

                    client_uuid_to_handle.insert(client_handler_handle.client, client_handler_handle);
                }
                recv(self.disconnected_clients_rx) -> disconnected_client => {
                    let disconnected_client: DisconnectedClient = match disconnected_client {
                        Ok(disconnected_client) => disconnected_client,
                        Err(error) => {
                            log::error!("Error receiving disconnected client: {}", error);
                            continue
                        }
                    };

                    match client_uuid_to_handle.remove(&disconnected_client.client_uuid) {
                        Some(handle) => {
                            ClientHandlerMonitor::join_client_handler(&disconnected_client.client_uuid, handle.handle);
                        },
                        None => {
                            pending_disconnected_clients.insert(disconnected_client.client_uuid);
                        }
                    }
                }
            }
        }

        // If there are any pending disconnected clients, then we need to wait to receive their
        // handles.
        if !pending_disconnected_clients.is_empty() {
            log::info!(
                "There are {} pending disconnected clients.",
                pending_disconnected_clients.len()
            );
        }
        while !pending_disconnected_clients.is_empty() {
            let client_handler_handle: ClientHandlerHandle =
                self.client_handler_handles_rx.recv().unwrap();
            ClientHandlerMonitor::join_client_handler(
                &client_handler_handle.client,
                client_handler_handle.handle,
            );
            pending_disconnected_clients.remove(&client_handler_handle.client);
        }

        // Join all the client handlers.
        for (client, mut handle) in client_uuid_to_handle.into_iter() {
            let _ = handle.stop_tx.write(&[1; 1]).unwrap();
            ClientHandlerMonitor::join_client_handler(&client, handle.handle);
        }

        log::info!("Client handler monitor stopping...");
    }

    /// Join a client handler thread.
    fn join_client_handler(disconnected_client: &Uuid, handle: JoinHandle<()>) {
        log::info!(
            "Waiting for client handler for client {} to stop...",
            disconnected_client
        );
        let _ = handle.join();
        log::info!("Client handler for client {} stopped.", disconnected_client);
    }
}

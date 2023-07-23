//! Handles sending responses to clients.
use crate::client::Client;
use crate::client_request::ClientRequest;
use crate::disconnected_client::DisconnectedClient;
use crate::stop::Stop;

use insh_api::Response;

use std::collections::HashMap;
use std::io::Write;
use std::os::unix::net::UnixStream;

use crossbeam::channel::{select, Receiver};
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Handles sending responses to clients.
#[derive(TypedBuilder)]
pub struct ResponseHandler {
    /// Responses to requests generated by request handlers.
    responses_rx: Receiver<Response>,

    /// Used to receive updates about requests made by clients.
    client_requests_rx: Receiver<ClientRequest>,

    /// Used to receive new clients.
    new_clients_rx: Receiver<Client>,
    /// Used to receive updates about clients disconnecting.
    disconnected_clients_rx: Receiver<DisconnectedClient>,
    /// A receiver for a stop sentinel.
    stop_rx: Receiver<Stop>,

    /// A map from request/response UUID to the client UUID.
    #[builder(setter(skip), default)]
    request_to_client: HashMap<Uuid, Uuid>,
    /// A map from client UUID to unix stream.
    #[builder(setter(skip), default)]
    client_streams: HashMap<Uuid, UnixStream>,
    /// A map from client UUID to the number of responses that have been handled for the client.
    #[builder(setter(skip), default)]
    client_to_num_handled_responses: HashMap<Uuid, usize>,
    /// A map from client UUID to the total number of requests the client made before
    /// disconnecting.
    #[builder(setter(skip), default)]
    client_to_num_total_requests: HashMap<Uuid, usize>,
}

impl ResponseHandler {
    /// Run the response handler.
    pub fn run(&mut self) {
        log::info!("Response handler running.");

        loop {
            select! {
                recv(self.stop_rx) -> _stop => {
                    break;
                }
                recv(self.new_clients_rx) -> new_client => {
                    let new_client: Client = match new_client {
                        Ok(new_client) => new_client,
                        Err(error) => {
                            log::error!("Error receiving info about a new client: {}", error);
                            continue;
                        }
                    };

                    self.handle_new_client(new_client);
                }
                recv(self.client_requests_rx) -> client_request => {
                    let client_request: ClientRequest = match client_request {
                        Ok(client_request) => client_request,
                        Err(error) => {
                            log::error!("Error receiving client request: {}", error);
                            continue;
                        }
                    };

                    self.handle_client_request(client_request);
                }
                recv(self.responses_rx) -> response => {
                    let response: Response = match response {
                        Ok(response) => response,
                        Err(error) => {
                            log::error!("Error receiving response: {}", error);
                            continue;
                        },
                    };

                    let response_uuid: &Uuid = response.uuid();
                    log::info!("Handling response {}.", response_uuid);

                    // Determine which client the response should be sent to.
                    log::debug!(
                        "Determining which client response {} is for...",
                        response_uuid
                    );
                    let client_uuid: Uuid = match self.request_to_client.get(response_uuid) {
                        Some(client_uuid) => *client_uuid,
                        None => {
                            log::warn!("Have not received info about which client made request {} yet.", response_uuid);
                            loop {
                                let client_request: ClientRequest = self.client_requests_rx.recv().unwrap();
                                let request_uuid: Uuid = *client_request.request_uuid();
                                let client_uuid: Uuid = *client_request.client_uuid();
                                self.handle_client_request(client_request);
                                if request_uuid == *response_uuid {
                                    break client_uuid;
                                }
                            }
                        },
                    };
                    log::debug!(
                        "The response {} is for client {}.",
                        response_uuid,
                        client_uuid
                    );

                    // If this is the last response then remove the request from the map.
                    if response.last() {
                        self.request_to_client.remove(response_uuid);
                    }

                    // Get the client stream.
                    log::debug!("Gettings stream for client {}...", client_uuid);
                    let stream: &mut UnixStream = match self.client_streams.get_mut(&client_uuid) {
                        Some(stream) => stream,
                        None => {
                            log::warn!("Have not received client stream for client {} yet.", client_uuid);
                            loop {
                                let new_client: Client = self.new_clients_rx.recv().unwrap();
                                let found: bool = new_client.uuid == client_uuid;
                                self.handle_new_client(new_client);
                                if found {
                                    break self.client_streams.get_mut(&client_uuid).unwrap();
                                }
                            }
                        },
                    };
                    log::debug!("Successfully got the stream for client {}.", client_uuid);

                    // Increment the number of handled responses for the client.
                    let mut handled_responses: usize = *self.client_to_num_handled_responses.get(&client_uuid).unwrap();
                    handled_responses += 1;
                    self.client_to_num_handled_responses.insert(client_uuid, handled_responses);

                    // Serialize the response.
                    // TODO: Re-use the respones buffer.
                    log::debug!("Serializing the response...");
                    let response: Vec<u8> = bincode::serialize(&response).unwrap();
                    log::debug!("Serialized the response.");

                    // Send the length.
                    // TODO: Re-use the length buffer.
                    log::debug!("Determining the length of the serialized response...");
                    let length: u64 = response.len().try_into().unwrap();
                    log::debug!("The serialized response is {} bytes.", length);

                    log::debug!("Sending the length...");
                    let length_buffer: [u8; 8] = length.to_be_bytes();
                    if let Err(error) = stream.write(&length_buffer) {
                        log::error!("Failed to write length: {}", error);
                        self.maybe_cleanup_client(&client_uuid);
                        continue
                    };
                    log::debug!("Sent the length.");

                    // Send the response.
                    log::debug!("Sending the response...");
                    if let Err(error) = stream.write(&response) {
                        log::error!("Failed to write response: {}", error);
                        self.maybe_cleanup_client(&client_uuid);
                        continue
                    }
                    log::debug!("Sent the response.");

                    // If the client disconnected, and all responses have now been handled, then
                    // remove the client from the state.
                    self.maybe_cleanup_client(&client_uuid);

                    log::info!("Done handling response {}.", response_uuid);
                }
                recv(self.disconnected_clients_rx) -> disconnected_client => {
                    let disconnected_client: DisconnectedClient = match disconnected_client {
                        Ok(disconnected_client) => disconnected_client,
                        Err(error) => {
                            log::error!("Error receiving info about a disconnected client: {}", error);
                            continue;
                        }
                    };

                    let client_uuid: &Uuid = &disconnected_client.client_uuid;

                    let handled_responses: usize = match self.client_to_num_handled_responses.get(client_uuid) {
                        Some(handled_responses) => *handled_responses,
                        None => {
                            log::warn!("Have not received initial client info about the disconnected client yet.");
                            loop {
                                let new_client: Client = self.new_clients_rx.recv().unwrap();
                                let found: bool = new_client.uuid == *client_uuid;
                                self.handle_new_client(new_client);
                                if found {
                                    break 0;
                                }
                            }
                        }
                    };

                    // If all of the responses for the client have already been handled, then remove
                    // information about the client from the state.
                    if handled_responses == disconnected_client.num_requests {
                        self.cleanup_client(client_uuid);
                        continue;
                    }

                    // If not all the responses for the client have been handled, then save the
                    // number of total requests the client made. When all the responses have been
                    // handled, information about the client will be removed from the state.
                    self.client_to_num_total_requests.insert(*client_uuid, disconnected_client.num_requests);
                }
            }
        }

        log::info!("Response handler stopping...");
    }

    /// Handle a new client.
    fn handle_new_client(&mut self, client: Client) {
        let Client {
            uuid: client_uuid,
            stream,
        } = client;
        self.client_to_num_handled_responses.insert(client_uuid, 0);
        self.client_streams.insert(client_uuid, stream);
    }

    /// Remove a client from the state.
    fn cleanup_client(&mut self, client_uuid: &Uuid) {
        self.client_to_num_handled_responses.remove(client_uuid);
        self.client_streams.remove(client_uuid);
    }

    /// Cleanup the client if all its responses have been handled.
    fn maybe_cleanup_client(&mut self, client_uuid: &Uuid) {
        let handled_responses = self
            .client_to_num_handled_responses
            .get(client_uuid)
            .unwrap();
        if let Some(total_requests) = self.client_to_num_total_requests.get(client_uuid) {
            if total_requests == handled_responses {
                self.cleanup_client(client_uuid);
            }
        }
    }

    /// Handle information about a request made by a client.
    fn handle_client_request(&mut self, client_request: ClientRequest) {
        let request_uuid: &Uuid = client_request.request_uuid();
        let client_uuid: Uuid = *client_request.client_uuid();
        self.request_to_client.insert(*request_uuid, client_uuid);
    }
}

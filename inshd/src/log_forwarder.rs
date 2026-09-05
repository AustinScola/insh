//! Forwards log records to clients which have subscribed to them.
use crate::contexted_response::ContextedResponse;
use crate::disconnected_client::DisconnectedClient;
use crate::log_subscription::LogSubscription;
use crate::stop::Stop;

use insh_api::{LogRecord, Response, ResponseParams, StreamLogsResponseParams};

use std::collections::HashMap;

use crossbeam::channel::{select, Receiver, Sender};
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Forwards log records to clients which have subscribed to them.
#[derive(TypedBuilder)]
pub struct LogForwarder {
    /// A receiver of log records.
    records_rx: Receiver<LogRecord>,
    /// A receiver of subscriptions to the logs.
    subscriptions_rx: Receiver<LogSubscription>,
    /// A receiver of information about clients disconnecting.
    disconnected_clients_rx: Receiver<DisconnectedClient>,
    /// A sender of responses.
    contexted_responses_tx: Sender<ContextedResponse>,
    /// A receiver of a stop sentinel.
    stop_rx: Receiver<Stop>,

    /// A map from the UUID of a subscribed client to the UUID of the request the log records are
    /// sent as responses to.
    #[builder(setter(skip), default)]
    subscriptions: HashMap<Uuid, Uuid>,
}

impl LogForwarder {
    /// Run the log forwarder.
    pub fn run(&mut self) {
        log::info!("Log forwarder running.");

        loop {
            select! {
                recv(self.stop_rx) -> _stop => {
                    break;
                }
                recv(self.subscriptions_rx) -> subscription => {
                    match subscription {
                        Ok(subscription) => self.handle_subscription(subscription),
                        Err(error) => {
                            log::error!("Error receiving a log subscription: {}", error);
                        }
                    }
                }
                recv(self.disconnected_clients_rx) -> disconnected_client => {
                    match disconnected_client {
                        Ok(disconnected_client) => {
                            self.handle_disconnected_client(&disconnected_client)
                        }
                        Err(error) => {
                            log::error!("Error receiving info about a disconnected client: {}", error);
                        }
                    }
                }
                recv(self.records_rx) -> record => {
                    if let Ok(record) = record {
                        self.handle_record(record);
                    }
                }
            }
        }

        log::info!("Log forwarder stopping...");
    }

    /// Handle a subscription to the logs.
    fn handle_subscription(&mut self, subscription: LogSubscription) {
        log::info!(
            "Client {} subscribed to the logs with request {}.",
            subscription.client_uuid(),
            subscription.request_uuid()
        );
        self.subscriptions
            .insert(*subscription.client_uuid(), *subscription.request_uuid());
    }

    /// Handle a client disconnecting.
    fn handle_disconnected_client(&mut self, disconnected_client: &DisconnectedClient) {
        let request_uuid: Uuid = match self.subscriptions.remove(&disconnected_client.client_uuid) {
            Some(request_uuid) => request_uuid,
            None => {
                return;
            }
        };

        log::info!(
            "Client {} unsubscribed from the logs.",
            disconnected_client.client_uuid
        );

        // Send the last response for the request so that the response handler stops considering
        // the request to be one which more responses could be sent for.
        self.send(request_uuid, vec![], true);
    }

    /// Handle a log record.
    fn handle_record(&mut self, record: LogRecord) {
        // NOTE: Nothing in here may log. Log records emitted here would be forwarded, which would
        // emit more log records, and so on forever.
        for request_uuid in self.subscriptions.values().copied().collect::<Vec<Uuid>>() {
            self.send(request_uuid, vec![record.clone()], false);
        }
    }

    /// Send log records as a response to a request to stream the logs.
    fn send(&self, request_uuid: Uuid, records: Vec<LogRecord>, last: bool) {
        let response: Response = Response::builder()
            .uuid(request_uuid)
            .last(last)
            .params(ResponseParams::StreamLogs(
                StreamLogsResponseParams::builder().records(records).build(),
            ))
            .build();

        // NOTE: The responses are sent with logging turned off. If they were logged, then sending
        // one would emit log records, which would be sent as more responses, and so on forever.
        let response: ContextedResponse = ContextedResponse::builder()
            .response(response)
            .log(false)
            .build();

        let _ = self.contexted_responses_tx.send(response);
    }
}

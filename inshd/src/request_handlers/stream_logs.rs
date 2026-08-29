//! Handles requests to stream the logs of the daemon.
use crate::log_subscription::LogSubscription;

use insh_api::{ResponseParamsAndLast, StreamLogsRequestParams};

use crossbeam::channel::Sender;
use uuid::Uuid;

/// Handles a request to stream the logs of the daemon.
pub struct StreamLogs {}

impl StreamLogs {
    /// Subscribe the client to the logs.
    ///
    /// All of the responses are sent by the log forwarder, including the last one, which it sends
    /// when the client disconnects.
    pub fn run(
        _params: &StreamLogsRequestParams,
        client_uuid: Uuid,
        request_uuid: Uuid,
        subscriptions_tx: &Sender<LogSubscription>,
    ) -> StreamLogs {
        let subscription: LogSubscription = LogSubscription::builder()
            .client_uuid(client_uuid)
            .request_uuid(request_uuid)
            .build();
        if let Err(error) = subscriptions_tx.send(subscription) {
            log::error!("Error sending a log subscription: {}", error);
        }

        StreamLogs {}
    }
}

impl Iterator for StreamLogs {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        return None;
    }
}

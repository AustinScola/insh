//! Streams the logs of inshd.
use crate::logging::{Color, ColoredLogRecord};

use insh_api::{
    Request, RequestParams, Response, ResponseParams, StreamLogsRequestParams,
    StreamLogsResponseParams,
};
use inshd_client::InshdClient;

/// Streams the logs of inshd.
pub struct Logs {}

impl Logs {
    /// Stream the logs of inshd.
    ///
    /// Only the log records which are emitted from this point onwards are streamed.
    pub fn run(color: Color) -> Result<(), LogsError> {
        let color: bool = color.color_stdout();

        let mut client: InshdClient = match InshdClient::connect() {
            Ok(client) => client,
            Err(error) => {
                return Err(LogsError::FailedToConnect(error));
            }
        };

        // Send a request to stream the logs.
        let request: Request = Request::builder()
            .params(RequestParams::StreamLogs(
                StreamLogsRequestParams::builder().build(),
            ))
            .build();
        if let Err(error) = client.send(&request) {
            return Err(LogsError::FailedToSendRequest(error));
        }

        // Print the log records as they are received.
        loop {
            let response: Response = match client.receive() {
                Ok(response) => response,
                Err(error) => {
                    return Err(LogsError::FailedToReceiveResponse(error));
                }
            };

            let params: &StreamLogsResponseParams = match response.params() {
                ResponseParams::StreamLogs(params) => params,
                _ => {
                    continue;
                }
            };

            for record in params.records() {
                if color {
                    println!("{}", ColoredLogRecord::new(record));
                } else {
                    println!("{}", record);
                }
            }
        }
    }
}

mod logs_error {
    //! An error streaming the logs of inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};

    use inshd_client::{ConnectError, ReceiveError, SendError};

    /// An error streaming the logs of inshd.
    #[allow(clippy::enum_variant_names)]
    pub enum LogsError {
        /// Failed to connect to inshd.
        FailedToConnect(ConnectError),
        /// Failed to send the request to stream the logs.
        FailedToSendRequest(SendError),
        /// Failed to receive a response.
        FailedToReceiveResponse(ReceiveError),
    }

    impl Display for LogsError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToConnect(error) => {
                    write!(formatter, "{}", error)
                }
                Self::FailedToSendRequest(error) => {
                    write!(formatter, "Failed to request the logs: {}", error)
                }
                Self::FailedToReceiveResponse(error) => {
                    write!(formatter, "{}", error)
                }
            }
        }
    }
}
use logs_error::LogsError;

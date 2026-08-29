//! Handles requests from clients.
use crossbeam::channel::{select, Receiver, Sender};
use typed_builder::TypedBuilder;
use uuid::Uuid;

use insh_api::{Request, RequestParams, Response, ResponseParamsAndLast};

use crate::config::Config;
use crate::contexted_request::ContextedRequest;
use crate::contexted_response::ContextedResponse;
use crate::log_subscription::LogSubscription;
use crate::request_handlers::{
    CreateFile, FindFiles, GetFiles, SearchPhrase, StreamLogs, SuggestSearchPhrase,
};
use crate::stop::Stop;

/// Handles requests from clients.
#[derive(TypedBuilder)]
pub struct RequestHandler {
    /// The request handler number.
    #[allow(dead_code)]
    number: usize,
    /// A receiver for requests.
    contexted_requests_rx: Receiver<ContextedRequest>,
    /// A sender for responses.
    contexted_responses_tx: Sender<ContextedResponse>,
    /// A sender of log streaming subscriptions.
    log_subscriptions_tx: Sender<LogSubscription>,
    /// A receiver for a stop sentinel.
    stop_rx: Receiver<Stop>,
    /// The configuration for inshd.
    config: Config,
}

impl RequestHandler {
    /// Run the request handler.
    pub fn run(&mut self) {
        log::info!("Request handler running.");

        loop {
            select! {
                recv(self.stop_rx) -> _stop => {
                    break;
                }
                recv(self.contexted_requests_rx) -> contexted_request => {
                    let contexted_request: ContextedRequest = contexted_request.unwrap();
                    let client_uuid: Uuid = *contexted_request.client_uuid();
                    let request: &Request = contexted_request.request();
                    log::info!("Handling request {}.", request.uuid());

                    let response_params_and_last_iter: Box<dyn Iterator<Item = ResponseParamsAndLast>> = match request.params() {
                        RequestParams::GetFiles(params) => Box::new(GetFiles::new(params)),
                        RequestParams::FindFiles(params) => Box::new(FindFiles::run(params)),
                        RequestParams::CreateFile(params) => Box::new(CreateFile::new(params)),
                        RequestParams::SearchPhrase(params) => {
                            Box::new(SearchPhrase::run(params, self.config.clone()))
                        }
                        RequestParams::SuggestSearchPhrase(params) => {
                            Box::new(SuggestSearchPhrase::new(params))
                        }
                        RequestParams::StreamLogs(params) => {
                            Box::new(StreamLogs::run(params, client_uuid, *request.uuid(), &self.log_subscriptions_tx))
                        }
                    };

                    // Whether the responses for the request are dispatched or not.
                    let dispatches: bool = matches!(request.params(), RequestParams::StreamLogs(_));

                    let mut sent_last: bool = false;
                    let mut send_error: bool = false;
                    for response_params_and_last in response_params_and_last_iter {
                        let response = Response::builder()
                            .uuid(*request.uuid())
                            .last(response_params_and_last.last)
                            .params(response_params_and_last.response_params)
                            .build();
                        let response = ContextedResponse::builder().response(response).build();

                        if response_params_and_last.last {
                            if sent_last {
                                log::error!("Multiple last responses.");
                                break;
                            }
                            sent_last = true;
                        }

                        if let Err(error) = self.contexted_responses_tx.send(response) {
                            log::error!("Error sending response: {}", error);
                            send_error = true;
                            break;
                        }
                    }
                    if !dispatches && !sent_last && !send_error {
                        log::warn!("Never received last response.");
                    }

                    log::info!("Done handling request {}.", request.uuid());
                }
            }
        }

        log::info!("Request handler stopping...");
    }
}

/// Context for a request.
#[derive(TypedBuilder)]
pub struct Context {}

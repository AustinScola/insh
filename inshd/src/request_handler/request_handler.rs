//! Handles requests from clients.
use crossbeam::channel::{select, Receiver, Sender};
use typed_builder::TypedBuilder;

use insh_api::{Request, RequestParams, Response, ResponseParamsAndLast};

use super::ai_chat::AIChat;
use super::create_file::CreateFile;
use super::find_files::FindFiles;
use super::get_files::GetFiles;
use crate::stop::Stop;
use config::Config;

/// Handles requests from clients.
#[derive(TypedBuilder)]
pub struct RequestHandler {
    /// The request handler number.
    #[allow(dead_code)]
    number: usize,
    /// Configuration.
    config: Config,
    /// A receiver for requests.
    requests: Receiver<Request>,
    /// A sender for responses.
    responses: Sender<Response>,
    /// A receiver for a stop sentinel.
    stop_rx: Receiver<Stop>,
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
                recv(self.requests) -> request => {
                    let request: Request = request.unwrap();
                    log::info!("Handling request {}.", request.uuid());

                    let response_params_and_last_iter: Box<dyn Iterator<Item = ResponseParamsAndLast>> = match request.params() {
                        RequestParams::GetFiles(params) => Box::new(GetFiles::new(params)),
                        RequestParams::FindFiles(params) => Box::new(FindFiles::run(params)),
                        RequestParams::CreateFile(params) => Box::new(CreateFile::new(params)),
                        RequestParams::AIChat(params) => Box::new(AIChat::new(params, self.config.clone())),
                    };

                    let mut sent_last: bool = false;
                    let mut send_error: bool = false;
                    for response_params_and_last in response_params_and_last_iter {
                        let response = Response::builder()
                            .uuid(*request.uuid())
                            .last(response_params_and_last.last)
                            .params(response_params_and_last.response_params)
                            .build();

                        if response_params_and_last.last {
                            if sent_last {
                                log::error!("Multiple last responses.");
                                break;
                            }
                            sent_last = true;
                        }

                        if let Err(error) = self.responses.send(response) {
                            log::error!("Error sending response: {}", error);
                            send_error = true;
                            break;
                        }
                    }
                    if !sent_last && !send_error {
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

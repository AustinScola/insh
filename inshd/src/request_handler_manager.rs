//! Manages the request handler threads.
use std::thread;
use std::thread::JoinHandle;

use crate::request_handler::RequestHandler;
use crate::request_handler_died::RequestHandlerDied;
use crate::stop::Stop;
use insh_api::{Request, Response};

use crossbeam::channel::{self, select, Receiver, Sender};
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
/// Manages the request handler threads.
pub struct RequestHandlerManager {
    /// The number of request handlers.
    num_request_handlers: usize,
    /// A receiver of request handler dying information.
    died_rx: Receiver<RequestHandlerDied>,
    /// Receivers of requests for each request handler.
    requests_rxs: Vec<Receiver<Request>>,
    /// A senders of responses.
    responses_tx: Sender<Response>,
    /// A receiver of a stop sentinel.
    stop_rx: Receiver<Stop>,
}

impl RequestHandlerManager {
    /// Run the request handler manager.
    pub fn run(&mut self) {
        log::info!("Request handler manager running.");

        let mut request_handler_handles: Vec<JoinHandle<()>> =
            Vec::with_capacity(self.num_request_handlers);
        let mut request_handler_stop_txs: Vec<Sender<Stop>> =
            Vec::with_capacity(self.num_request_handlers);
        let mut request_handler_stop_rxs: Vec<Receiver<Stop>> =
            Vec::with_capacity(self.num_request_handlers);

        // Start the request handlers.
        for request_handler_num in 0..self.num_request_handlers {
            let requests_rx = self.requests_rxs[request_handler_num].clone();

            let (request_handler_stop_tx, request_handler_stop_rx): (Sender<Stop>, Receiver<Stop>) =
                channel::unbounded();
            request_handler_stop_txs.push(request_handler_stop_tx);
            request_handler_stop_rxs.push(request_handler_stop_rx.clone());

            // Create and spawn the request handler.
            let mut request_handler = RequestHandler::builder()
                .number(request_handler_num)
                .requests(requests_rx)
                .responses(self.responses_tx.clone())
                .stop_rx(request_handler_stop_rx)
                .build();
            let name: String = format!("request-handler-{}", request_handler_num).to_string();
            let request_handler_handle: JoinHandle<()> = thread::Builder::new()
                .name(name)
                .spawn(move || request_handler.run())
                .unwrap();
            request_handler_handles.push(request_handler_handle);
        }

        loop {
            select! {
                recv(self.stop_rx) -> _stop => {
                    break;
                }
                recv(self.died_rx) -> request_handler_died => {
                    let request_handler_died: RequestHandlerDied = match request_handler_died {
                        Ok(request_handler_died) => request_handler_died,
                        Err(_) => break,
                    };

                    let number: usize = request_handler_died.number;
                    log::info!("Restarting request handler {}...", number);

                    let mut request_handler = RequestHandler::builder()
                        .number(number)
                        .requests(self.requests_rxs[number].clone())
                        .responses(self.responses_tx.clone())
                        .stop_rx(request_handler_stop_rxs[number].clone())
                        .build();
                    let name: String = format!("request-handler-{}", number).to_string();
                    let request_handler_handle: JoinHandle<()> = thread::Builder::new()
                        .name(name)
                        .spawn(move || request_handler.run())
                        .unwrap();
                    request_handler_handles[number] = request_handler_handle;
                }
            }
        }

        // Stop the request handlers.
        log::info!("Stopping request handlers...");
        for request_handler_stop_tx in request_handler_stop_txs {
            request_handler_stop_tx.send(Stop::new()).unwrap();
        }
        for request_handler_handle in request_handler_handles {
            let _ = request_handler_handle.join();
        }
        log::info!("Stopped request handlers.");

        log::info!("Request handler manager stopping...");
    }
}

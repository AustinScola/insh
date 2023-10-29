/*!
A graphical, interactive, terminal environment.
*/
#![deny(warnings)]
#![deny(missing_docs)]
#![allow(clippy::module_inception)]
#![allow(clippy::needless_return)]
#![allow(clippy::while_let_loop)]
#![allow(clippy::single_match)]
#![allow(clippy::infallible_destructuring_match)]

#[macro_use]
extern crate lazy_static;

mod ansi_escaped_text;
mod args;
mod auto_completer;
mod auto_completers;
mod clipboard;
mod color;
mod components;
mod config;
mod current_dir;
mod data;
#[cfg(feature = "logging")]
mod logging;
mod phrase_searcher;
mod programs;
mod requester;
mod response_handler;
mod stateful;
mod string;

use std::os::unix::net::UnixStream;
use std::process::exit;

use clap::Parser;
#[cfg(feature = "logging")]
use flexi_logger::LoggerHandle;
use uuid::Uuid;

use common::paths::INSHD_SOCKET;
use insh_api::{GetFilesRequestParams, Request, RequestParams, Response};
use term::TermEvent;
use til::{App, AppRunOptions, Component, Requester, ResponseHandler, Stopper, SystemEffect};

use crate::args::Args;
use crate::components::{Insh, InshProps};
use crate::config::Config;
#[cfg(feature = "logging")]
use crate::logging::{configure_logging, ConfigureLoggingResult};
use crate::requester::InshdRequester;
use crate::response_handler::{InshdResponseHandler, InshdResponseHandlerStopper};
use crate::stateful::Stateful;

fn main() {
    let args: Args = Args::parse();

    #[cfg(feature = "logging")]
    let _logger_handle: LoggerHandle;
    #[cfg(feature = "logging")]
    if let Some(log_file_path) = args.log_file_path() {
        let configure_logging_result: ConfigureLoggingResult =
            configure_logging(log_file_path.to_path_buf(), args.log_specification());
        _logger_handle = match configure_logging_result {
            Ok(_logger_handle) => _logger_handle,
            Err(error) => {
                println!("{}", error);
                exit(1);
            }
        }
    }

    // Determine the starting effects.
    let mut starting_effects: Option<Vec<SystemEffect<Request>>> = args.starting_effects();
    let pending_browser_request: Option<Uuid> = if args.browse() {
        let request = Request::builder()
            .params(RequestParams::GetFiles(
                GetFilesRequestParams::builder()
                    .dir(args.dir().clone().unwrap_or_else(current_dir::current_dir))
                    .build(),
            ))
            .build();

        let request_uuid: Uuid = *request.uuid();
        let effect = SystemEffect::Request(request);
        if let Some(ref mut starting_effects) = starting_effects {
            starting_effects.push(effect);
        } else {
            starting_effects = Some(vec![effect]);
        }

        Some(request_uuid)
    } else {
        None
    };
    #[cfg(feature = "logging")]
    log::info!("{:?}", pending_browser_request);

    // Determine the starting term events.
    let starting_term_events: Option<Vec<TermEvent>> = args.starting_term_events();

    let config: Config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            println!("{}", error);
            exit(1);
        }
    };

    let mut app: App = App::builder().build();

    let insh_props: InshProps = InshProps::builder()
        .dir(args.dir().clone())
        .start(args.command().clone().into())
        .pending_browser_request(pending_browser_request)
        .config(config)
        .build();
    let root = Insh::new(insh_props);

    // Connect to the Unix socket.
    let socket = match UnixStream::connect(&*INSHD_SOCKET) {
        Ok(socket) => socket,
        Err(error) => {
            println!("Failed to connect to the inshd socket: {}", error);
            exit(1);
        }
    };

    // Create a requester for sending requests to the unix stream socket.
    let requester: Box<dyn Requester<Request>> = Box::new(
        InshdRequester::builder()
            .socket(socket.try_clone().unwrap())
            .build(),
    );

    // Create a respones handler for receiving responses from the unix stream socket.
    let response_handler: Box<dyn ResponseHandler<Response>> = Box::new(
        InshdResponseHandler::builder()
            .socket(socket.try_clone().unwrap())
            .build(),
    );
    let response_handler_stopper: Box<dyn Stopper> = Box::new(
        InshdResponseHandlerStopper::builder()
            .socket(socket)
            .build(),
    );

    let run_options: AppRunOptions<InshProps, Request, Response> = AppRunOptions::builder()
        .root(Box::new(root))
        .starting_effects(starting_effects)
        .starting_term_events(starting_term_events)
        .requester(requester)
        .response_handler(response_handler)
        .response_handler_stopper(response_handler_stopper)
        .build();
    app.run(run_options);
}

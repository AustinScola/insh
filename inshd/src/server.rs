//! The inshd server.
use crate::client::Client;
use crate::client_handler_handle::ClientHandlerHandle;
use crate::client_handler_monitor::ClientHandlerMonitor;
use crate::client_request::ClientRequest;
use crate::conn_handler::ConnHandler;
use crate::disconnected_client::DisconnectedClient;
use crate::request_handler_died::RequestHandlerDied;
use crate::request_handler_manager::RequestHandlerManager;
use crate::response_handler::ResponseHandler;
use crate::scheduler::Scheduler;
use crate::signal_handler::SignalHandler;
use crate::stop::Stop;
use crate::INSHD_PID_FILE;
use common::paths::INSHD_SOCKET;
use insh_api::{Request, Response};

use std::fs::remove_file;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::panic;
use std::panic::PanicHookInfo;
use std::process::exit;
use std::thread;
use std::thread::JoinHandle;

use crossbeam::channel::{self, Receiver, Sender};
use crossbeam::sync::{Parker, Unparker};

/// The inshd server.
pub struct Server {}

impl Server {
    /// Return a new server.
    pub fn new() -> Self {
        Self {}
    }

    /// Run the server.
    // NOTE: We do a lot of extra cloning of channels in here in order to ensure that they do not
    // get dropped.
    #[allow(clippy::redundant_clone)]
    pub fn run(&self, options: RunOptions) -> Result<(), RunError> {
        log::info!("Running...");
        let RunOptions {
            num_request_handlers,
        } = options;

        let (died_tx, died_rx): (Sender<RequestHandlerDied>, Receiver<RequestHandlerDied>) =
            channel::unbounded();

        // Set up a panic hook.
        Server::set_panic_hook(died_tx.clone());

        // Create a unix socket for clients to connect to.
        log::debug!("Creating a unix socket {:?}...", &*INSHD_SOCKET);
        let listener = match UnixListener::bind(&*INSHD_SOCKET) {
            Ok(listener) => listener,
            Err(error) => {
                log::error!("Failed to create the unix domain socket.");
                return Err(RunError::CreateSocketError(error));
            }
        };
        log::debug!("Created the unix socket.");

        // Create and spawn a thread for handling termination signals.
        let main_parker: Parker = Parker::new();
        let main_unparker: Unparker = main_parker.unparker().clone();
        let mut signal_handler = SignalHandler::builder()
            .main_unparker(main_unparker)
            .build();
        let signal_handler_handle: JoinHandle<_> = thread::Builder::new()
            .name("term-singals-handler".to_string())
            .spawn(move || signal_handler.run())
            .unwrap();

        // Crate and spawn a response handler thread.
        let (responses_tx, responses_rx): (Sender<Response>, Receiver<Response>) =
            channel::unbounded();
        let (new_clients_tx, new_clients_rx): (Sender<Client>, Receiver<Client>) =
            channel::unbounded();
        let (client_requests_tx, client_requests_rx): (
            Sender<ClientRequest>,
            Receiver<ClientRequest>,
        ) = channel::unbounded();
        let (disconnected_clients_tx, disconnected_clients_rx): (
            Sender<DisconnectedClient>,
            Receiver<DisconnectedClient>,
        ) = channel::unbounded();
        let mut disconnected_clients_txs: Vec<Sender<DisconnectedClient>> =
            vec![disconnected_clients_tx.clone()];
        let (response_handler_stop_tx, response_handler_stop_rx): (Sender<Stop>, Receiver<Stop>) =
            channel::unbounded();
        let mut response_handler = ResponseHandler::builder()
            .responses_rx(responses_rx)
            .new_clients_rx(new_clients_rx)
            .client_requests_rx(client_requests_rx)
            .disconnected_clients_rx(disconnected_clients_rx.clone())
            .stop_rx(response_handler_stop_rx)
            .build();
        let response_handler_handle: JoinHandle<()> = thread::Builder::new()
            .name("response-handler".to_string())
            .spawn(move || response_handler.run())
            .unwrap();

        // Create and spawn a request handler manager thread. The request handler manager starts
        // the request handler threads, restarts them if they die, and stops when it is time.
        let mut requests_rxs: Vec<Receiver<Request>> = Vec::with_capacity(num_request_handlers);
        let mut requests_txs: Vec<Sender<Request>> = Vec::with_capacity(num_request_handlers);
        for _ in 0..num_request_handlers {
            // Create the channels.
            let (requests_tx, requests_rx): (Sender<Request>, Receiver<Request>) =
                channel::unbounded();
            requests_rxs.push(requests_rx.clone());
            requests_txs.push(requests_tx);
        }
        let (request_handler_manager_stop_tx, request_handler_manager_stop_rx): (
            Sender<Stop>,
            Receiver<Stop>,
        ) = channel::unbounded();
        let mut request_handler_manager = RequestHandlerManager::builder()
            .num_request_handlers(num_request_handlers)
            .died_rx(died_rx)
            .requests_rxs(requests_rxs)
            .responses_tx(responses_tx.clone())
            .stop_rx(request_handler_manager_stop_rx)
            .build();
        let request_handler_manager_handle: JoinHandle<()> = thread::Builder::new()
            .name("request-handler-monitor".to_string())
            .spawn(move || request_handler_manager.run())
            .unwrap();

        // Create and spawn a scheduler to schedule the execution of requests with request handlers.
        let (incoming_requests_tx, incoming_requests_rx): (Sender<Request>, Receiver<Request>) =
            channel::unbounded();
        let (scheduler_stop_tx, scheduler_stop_rx): (Sender<Stop>, Receiver<Stop>) =
            channel::unbounded();
        let mut scheduler: Scheduler = Scheduler::builder()
            .num_request_handlers(num_request_handlers)
            .requests_txs(requests_txs.clone())
            .incoming_requests_rx(incoming_requests_rx)
            .stop(scheduler_stop_rx)
            .build();
        let scheduler_handle: JoinHandle<_> = thread::Builder::new()
            .name("scheduler".to_string())
            .spawn(move || scheduler.run())
            .unwrap();

        // Create and spawn a thread to handle the client handlers. Specifically, this enables
        // joining the client handlers on termination.
        let (disconnected_clients_tx, disconnected_clients_rx): (
            Sender<DisconnectedClient>,
            Receiver<DisconnectedClient>,
        ) = channel::unbounded();
        disconnected_clients_txs.push(disconnected_clients_tx.clone());
        let (client_handler_handles_tx, client_handler_handles_rx): (
            Sender<ClientHandlerHandle>,
            Receiver<ClientHandlerHandle>,
        ) = channel::unbounded();
        let (client_handler_monitor_stop_tx, client_handler_monitor_stop_rx): (
            Sender<Stop>,
            Receiver<Stop>,
        ) = channel::unbounded();
        let mut client_handler_monitor: ClientHandlerMonitor = ClientHandlerMonitor::builder()
            .disconnected_clients_rx(disconnected_clients_rx)
            .client_handler_handles_rx(client_handler_handles_rx)
            .stop_rx(client_handler_monitor_stop_rx)
            .build();
        let client_handler_monitor_handle: JoinHandle<_> = thread::Builder::new()
            .name("client-handler-monitor".to_string())
            .spawn(move || client_handler_monitor.run())
            .unwrap();

        // Create and spawn a thread to handle clients connecting to the socket.
        let (conn_handler_stop_rx, mut conn_handler_stop_tx) = os_pipe::pipe().unwrap();
        let mut conn_handler: ConnHandler = ConnHandler::builder()
            .listener(listener)
            .new_clients_tx(new_clients_tx.clone())
            .incoming_requests_tx(incoming_requests_tx.clone())
            .client_requests_tx(client_requests_tx.clone())
            .disconnected_clients_txs(disconnected_clients_txs)
            .client_handler_handles_tx(client_handler_handles_tx.clone())
            .stop_rx(conn_handler_stop_rx)
            .build();
        let conn_handler_handle: JoinHandle<_> = thread::Builder::new()
            .name("conn-handler".to_string())
            .spawn(move || conn_handler.run())
            .unwrap();

        // Wait until signaled to stop.
        main_parker.park();
        log::info!("Stopping...");

        log::info!("Stopping all threads...");

        let _ = conn_handler_stop_tx.write(&[1; 1]).unwrap();
        let _ = conn_handler_handle.join();
        log::info!("Connection handler stopped.");

        client_handler_monitor_stop_tx.send(Stop::new()).unwrap();
        let _ = client_handler_monitor_handle.join();
        log::info!("Client handler monitor stopped.");

        scheduler_stop_tx.send(Stop::new()).unwrap();
        let _ = scheduler_handle.join();
        log::info!("Scheduler stopped.");

        request_handler_manager_stop_tx.send(Stop::new()).unwrap();
        let _ = request_handler_manager_handle.join();
        log::info!("Request handler manager stopped.");

        let _ = signal_handler_handle.join();
        log::info!("Signal handler stopped.");

        response_handler_stop_tx.send(Stop::new()).unwrap();
        let _ = response_handler_handle.join();
        log::info!("Response handler stopped.");

        log::info!("All threads stopped.");

        Server::cleanup();
        Ok(())
    }

    /// Set the panic hook.
    fn set_panic_hook(died_tx: Sender<RequestHandlerDied>) {
        panic::set_hook(Box::new(move |panic_info: &PanicHookInfo| {
            let thread_handle = thread::current();
            let thread_name: &str = match thread_handle.name() {
                Some(thread_name) => thread_name,
                None => {
                    log::error!("Unnamed thread panicked: {}", panic_info);

                    Server::cleanup();
                    exit(1);
                }
            };

            log::error!("Thread {} panicked: {}", thread_name, panic_info);

            if let Some(rest) = thread_name.strip_prefix("request-handler-") {
                if let Ok(number) = rest.parse::<usize>() {
                    let request_handler_died = RequestHandlerDied::builder().number(number).build();
                    died_tx.send(request_handler_died).unwrap();
                }
            }
        }));
    }

    /// Perform cleanup.
    pub fn cleanup() {
        // Try to remove the socket file.
        log::debug!("Removing the socket...");
        match remove_file(&*INSHD_SOCKET) {
            Ok(_) => {
                log::debug!("Removed the socket...");
            }
            Err(error) => {
                log::warn!("Failed to removed the socket: {}", error);
            }
        }

        // Try to remove the pid file.
        log::debug!("Removing the pid file...");
        match remove_file(&*INSHD_PID_FILE) {
            Ok(_) => {
                log::debug!("Removed the pid file.");
            }
            Err(error) => {
                log::warn!("Failed to removed the pid file: {}", error);
            }
        }
    }
}

mod run_options {
    //! Options for running inshd.

    use typed_builder::TypedBuilder;

    /// The number of request handlers.
    const DEFAULT_NUM_REQUEST_HANDLERS: usize = 8;

    /// Options for running inshd.
    #[derive(TypedBuilder)]
    pub struct RunOptions {
        /// The number of request handlers.
        #[builder(default = DEFAULT_NUM_REQUEST_HANDLERS)]
        pub num_request_handlers: usize,
    }

    impl Default for RunOptions {
        fn default() -> Self {
            Self {
                num_request_handlers: DEFAULT_NUM_REQUEST_HANDLERS,
            }
        }
    }
}
pub use run_options::RunOptions;

mod run_error {
    //! An error running inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;

    /// An error running inshd.
    pub enum RunError {
        /// An error creating the unix socket.
        CreateSocketError(IOError),
    }

    impl Display for RunError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::CreateSocketError(error) => {
                    write!(formatter, "Failed to create the unix socket: {}.", error)
                }
            }
        }
    }
}
pub use run_error::RunError;

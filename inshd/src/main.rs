/*!
The insh daemon.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_return)]

mod args;
mod cache;
mod client;
mod client_handler;
mod client_handler_handle;
mod client_handler_monitor;
mod client_request;
mod commands;
mod config;
mod conn_handler;
mod contexted_request;
mod contexted_response;
mod disconnected_client;
mod file_finder;
mod file_searcher;
mod log_forwarder;
mod log_subscription;
mod logging;
mod paths;
mod pid_file;
mod pid_waiter;
mod request_handler;
mod request_handler_died;
mod request_handler_manager;
mod request_handlers;
mod response_handler;
mod scheduler;
mod server;
mod signal_handler;
mod stop;

use crate::args::{Args, Command, DatabaseCommand};
use crate::commands::{
    DatabaseShell, DatabaseShellOptions, Logs, Restart, RestartOptions, Start, StartOptions,
    Status, Stop, StopOptions,
};
use crate::logging::{configure_logging, ConfiguredLogging};

use clap::Parser;

#[macro_use]
extern crate lazy_static;

/// The main entry point.
fn main() {
    let args: Args = Args::parse();

    // Configure a basic stdout logger. The logger configured for the inshd process can be more
    // sophisticated, but for commands like start, stop, etc. we just want logging to go to stdout.
    let ConfiguredLogging {
        mut logger_handle,
        records_rx,
    } = configure_logging(&args.log_options());

    let exit_code: i32 = match args.command() {
        Command::Start(start_args) => {
            let mut options: StartOptions =
                StartOptions::new(&mut logger_handle, records_rx, start_args);
            if Start::run(&mut options).is_err() {
                1
            } else {
                0
            }
        }
        Command::Stop(stop_args) => {
            let options: StopOptions = StopOptions::new(stop_args);
            if Stop::run(&options).is_err() {
                1
            } else {
                0
            }
        }
        Command::Restart(restart_args) => {
            let mut options: RestartOptions =
                RestartOptions::new(&mut logger_handle, records_rx, restart_args);
            if Restart::run(&mut options).is_err() {
                1
            } else {
                0
            }
        }
        Command::Status => match Status::get() {
            Ok(status) => {
                log::info!("{}", status);
                0
            }
            Err(error) => {
                log::error!("{}", error);
                1
            }
        },
        Command::Logs => match Logs::run(args.color()) {
            Ok(_) => 0,
            Err(error) => {
                log::error!("{}", error);
                1
            }
        },
        Command::Database(database_args) => match &database_args.command {
            DatabaseCommand::Shell(shell_args) => {
                let options: DatabaseShellOptions = DatabaseShellOptions::new(shell_args);
                match DatabaseShell::run(&options) {
                    Ok(exit_code) => exit_code,
                    Err(error) => {
                        log::error!("{}", error);
                        1
                    }
                }
            }
        },
    };

    // NOTE: We should just be able to return a `std::process::ExitCode`, but the daemon process
    // does not exit if we do that?
    std::process::exit(exit_code);
}

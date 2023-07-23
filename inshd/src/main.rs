/*!
The insh daemon.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_return)]

mod args;
mod client;
mod client_handler;
mod client_handler_handle;
mod client_handler_monitor;
mod client_request;
mod conn_handler;
mod disconnected_client;
mod file_finder;
mod logging;
mod paths;
mod request_handler;
mod request_handler_died;
mod request_handler_manager;
mod response_handler;
mod scheduler;
mod server;
mod signal_handler;
mod stop;

use std::fs::File;
use std::io::Read;
use std::process::ExitCode;

use crate::args::{Args, Command, StopArgs};
use crate::logging::configure_logging;
use crate::paths::INSHD_PID_FILE;
use crate::server::{RunOptions, Server};

use clap::Parser;
use daemonize::Daemonize;
use flexi_logger::LoggerHandle;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

#[macro_use]
extern crate lazy_static;

/// The main entry point.
fn main() -> ExitCode {
    let args: Args = Args::parse();

    let _logger_handle: LoggerHandle = configure_logging(args.log_options());

    match args.command() {
        Command::Start => {
            if let Err(error) = start() {
                log::error!("{}", error);
                return ExitCode::FAILURE;
            }
        }
        Command::Stop(args) => match stop(args) {
            Ok(summary) => {
                log::info!("{}", summary);
            }
            Err(error) => {
                log::error!("{}", error);
                return ExitCode::FAILURE;
            }
        },
        Command::Status => match status() {
            Ok(status) => {
                log::info!("{}", status);
            }
            Err(error) => {
                log::error!("{}", error);
                return ExitCode::FAILURE;
            }
        },
    };

    ExitCode::SUCCESS
}

/// Start inshd.
fn start() -> Result<(), StartError> {
    log::info!("Starting inshd...");

    // Daemonize the process.
    log::info!("Daemonizing...");
    let daemonize = Daemonize::new()
        .pid_file(&*INSHD_PID_FILE)
        .chown_pid_file(true);
    if let Err(error) = daemonize.start() {
        return Err(StartError::FailedToDaemonize(error));
    }
    log::info!("Daemonized.");

    let server = Server::new();
    let run_options: RunOptions = RunOptions::default();
    if let Err(error) = server.run(run_options) {
        return Err(StartError::FailedToRunServer(error));
    }

    log::info!("Inshd stopped.");

    Ok(())
}

mod start_error {
    //! A failure to start inshd.

    use crate::server::RunError;
    use std::fmt::{Display, Error as FmtError, Formatter};

    use daemonize::Error as DaemonizeError;

    /// A failure to start inshd.
    pub enum StartError {
        /// A failure to daemonize the inshd server.
        FailedToDaemonize(DaemonizeError),
        /// A failure to start the inshd server.
        FailedToRunServer(RunError),
    }

    impl Display for StartError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToDaemonize(error) => {
                    write!(formatter, "Failed to daemonize the process: {}.", error)
                }
                Self::FailedToRunServer(error) => {
                    write!(formatter, "Failed to run the server: {}.", error)
                }
            }
        }
    }
}
use start_error::StartError;

/// Stop inshd.
fn stop(args: &StopArgs) -> Result<StopSummary, StopError> {
    let pid: u64 = match _get_inshd_pid() {
        Ok(pid) => pid,
        Err(error) => match error {
            GetPidError::PidFileNotFound => {
                return Err(StopError::NotRunning);
            }
            _ => {
                return Err(StopError::FailedToGetPid(error));
            }
        },
    };

    let pid: Pid = Pid::from_raw(pid.try_into().unwrap());

    let signal: Signal = match args.force {
        true => signal::SIGKILL,
        false => signal::SIGTERM,
    };

    // Send inshd a signal to stop.
    if let Err(error) = signal::kill(pid, signal) {
        return Err(StopError::ErrorSendingSignal(error));
    };

    // NOTE: We could use kill with a signal of 0 to check if the process with the pid is still
    // alive but it is not safe from race conditions (another process w/ the same pid could be
    // started).

    Ok(StopSummary::builder().signal(signal).build())
}

mod stop_summary {
    //! A summary of stopping inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};

    use nix::sys::signal::Signal;
    use typed_builder::TypedBuilder;

    /// A summary of stopping inshd.
    #[derive(TypedBuilder)]
    pub struct StopSummary {
        /// The signal send to inshd.
        signal: Signal,
    }

    impl Display for StopSummary {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            write!(formatter, "Sent {} to inshd.", self.signal.as_str())
        }
    }
}
use stop_summary::StopSummary;

mod stop_error {
    //! An error stopping inshd.
    use crate::get_pid_error::GetPidError;

    use std::fmt::{Display, Error as FmtError, Formatter};

    use nix::errno::Errno;

    /// An error stopping inshd.
    pub enum StopError {
        /// Inshd is not running.
        NotRunning,
        /// A failure to get the pid of inshd.
        FailedToGetPid(GetPidError),
        /// An error sending a signal to inshd.
        ErrorSendingSignal(Errno),
    }

    impl Display for StopError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::NotRunning => {
                    write!(formatter, "Inshd is not running.")
                }
                Self::FailedToGetPid(get_pid_error) => {
                    write!(formatter, "{}", get_pid_error)
                }
                Self::ErrorSendingSignal(errno) => {
                    write!(formatter, "Failed to send signal: {}", errno)
                }
            }
        }
    }
}
use stop_error::StopError;

/// Return the status of inshd.
fn status() -> Result<Status, StatusError> {
    let pid: u64 = match _get_inshd_pid() {
        Ok(pid) => pid,
        Err(error) => match error {
            GetPidError::PidFileNotFound => {
                return Ok(Status::NotRunning);
            }
            _ => {
                return Err(StatusError::FailedToGetPid(error));
            }
        },
    };

    Ok(Status::Running { pid })
}

mod status {
    //! The status of inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};

    /// The status of inshd.
    pub enum Status {
        /// Inshd is running.
        Running {
            /// The pid of inshd.
            pid: u64,
        },
        /// Inshd is not running.
        NotRunning,
    }

    impl Display for Status {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::Running { pid } => {
                    write!(formatter, "Status: Running (PID: {})", pid)
                }
                Self::NotRunning => {
                    write!(formatter, "Status: Not running")
                }
            }
        }
    }
}
use status::Status;

mod status_error {
    //! An error getting the status of inshd.

    use crate::get_pid_error::GetPidError;

    use std::fmt::{Display, Error as FmtError, Formatter};

    /// An error getting the status of inshd.
    pub enum StatusError {
        /// Failed to get the pid of inshd.
        FailedToGetPid(GetPidError),
    }

    impl Display for StatusError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToGetPid(error) => {
                    write!(formatter, "Failed to get PID of daemon: {}", error)
                }
            }
        }
    }
}
use status_error::StatusError;

/// Return the result of getting the pid of inshd.
fn _get_inshd_pid() -> Result<u64, GetPidError> {
    let mut file = match File::open(&*INSHD_PID_FILE) {
        Ok(file) => file,
        Err(error) => match error.kind() {
            std::io::ErrorKind::NotFound => {
                return Err(GetPidError::PidFileNotFound);
            }
            _ => return Err(GetPidError::ErrorOpeningPidFile(error)),
        },
    };

    let mut contents: String = String::new();
    if let Err(error) = file.read_to_string(&mut contents) {
        return Err(GetPidError::ErrorReadingPidFile(error));
    }

    let contents: &str = contents.trim_end();

    let pid: u64 = match contents.parse::<u64>() {
        Ok(pid) => pid,
        Err(error) => {
            return Err(GetPidError::FailedToParsePid(error));
        }
    };

    Ok(pid)
}

mod get_pid_error {
    //! An error getting the prcocess id of inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;
    use std::num::ParseIntError;

    /// An error getting the prcocess id of inshd.
    pub enum GetPidError {
        /// The pid file does not exist.
        PidFileNotFound,
        /// There was an error opening the pid file.
        ErrorOpeningPidFile(IOError),
        /// There was an error reading the pid file.
        ErrorReadingPidFile(IOError),
        /// Failed to parse the pid.
        FailedToParsePid(ParseIntError),
    }

    impl Display for GetPidError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::PidFileNotFound => {
                    write!(formatter, "PID file not found.")
                }
                Self::ErrorOpeningPidFile(error) => {
                    write!(formatter, "Error opening PID file: {}", error)
                }
                Self::ErrorReadingPidFile(error) => {
                    write!(formatter, "Error reading PID file: {}", error)
                }
                Self::FailedToParsePid(error) => {
                    write!(formatter, "Failed to parse PID: {}", error)
                }
            }
        }
    }
}
use get_pid_error::GetPidError;

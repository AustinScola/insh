//! Starts inshd.
use crate::config::Config;
use crate::paths::INSHD_PID_FILE;
use crate::server::{RunOptions, Server};

use daemon::{Daemon, Outcome as DaemonOutcome};
use flexi_logger::Duplicate as LogDuplicate;

/// Starts inshd.
pub struct Start {}

impl Start {
    /// Start inshd.
    pub fn run(options: &mut StartOptions) -> Result<(), StartError> {
        log::info!("Starting inshd...");

        if options.force {
            Server::cleanup();
        }

        // Daemonize the process.
        log::info!("Daemonizing...");
        let daemon = Daemon::new().pid_file(&*INSHD_PID_FILE);
        match daemon.execute() {
            Ok(DaemonOutcome::Parent) => {
                log::info!("Daemonized inshd.");

                log::info!("Started inshd.");
                return Ok(());
            }
            Ok(DaemonOutcome::Child) => {
                let _ = options
                    .logger_handle
                    .adapt_duplication_to_stdout(LogDuplicate::None);
            }
            Err(error) => {
                let error = StartError::FailedToDaemonize(error);
                log::error!("{}", error);
                return Err(error);
            }
        }

        let config: Config = match Config::load() {
            Ok(config) => config,
            Err(error) => {
                let error = StartError::FailedToLoadConfig(error);
                log::error!("{}", error);
                return Err(error);
            }
        };

        let server = Server::new();
        let run_options: RunOptions = RunOptions::builder()
            .config(config)
            .log_records_rx(options.log_records_rx.clone())
            .build();
        if let Err(error) = server.run(run_options) {
            let error = StartError::FailedToRunServer(error);
            log::error!("{}", error);
            return Err(error);
        }

        log::info!("Inshd stopped.");

        Ok(())
    }
}

mod start_options {
    //! Options for starting inshd.

    use crate::args::StartArgs;

    use insh_api::LogRecord;

    use crossbeam::channel::Receiver;
    use flexi_logger::LoggerHandle;

    /// Options for starting inshd.
    pub struct StartOptions<'a> {
        /// Start even if already running.
        pub force: bool,
        /// The basic logger handle.
        pub logger_handle: &'a mut LoggerHandle,
        /// A receiver of the log records which have been emitted.
        pub log_records_rx: Receiver<LogRecord>,
    }

    impl<'a> StartOptions<'a> {
        /// Return new start options.
        pub fn new(
            logger_handle: &'a mut LoggerHandle,
            log_records_rx: Receiver<LogRecord>,
            start_args: &StartArgs,
        ) -> Self {
            StartOptions {
                force: start_args.force,
                logger_handle,
                log_records_rx,
            }
        }
    }
}
pub use start_options::StartOptions;

mod start_error {
    //! A failure to start inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};

    use crate::config::LoadError as ConfigLoadError;
    use crate::server::RunError;

    use daemon::Error as DaemonError;

    /// A failure to start inshd.
    #[allow(clippy::enum_variant_names)]
    pub enum StartError {
        /// A failure to daemonize the inshd server.
        FailedToDaemonize(DaemonError),
        /// A failure to load the configuration.
        FailedToLoadConfig(ConfigLoadError),
        /// A failure to start the inshd server.
        FailedToRunServer(RunError),
    }

    impl Display for StartError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToDaemonize(error) => {
                    write!(formatter, "Failed to daemonize the process: {}.", error)
                }
                Self::FailedToLoadConfig(error) => {
                    write!(formatter, "Failed to load the configuration: {}.", error)
                }
                Self::FailedToRunServer(error) => {
                    write!(formatter, "Failed to run the server: {}.", error)
                }
            }
        }
    }
}
pub use start_error::StartError;

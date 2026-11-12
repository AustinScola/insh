//! Starts inshd.
use std::env::current_exe;
use std::path::PathBuf;

use crate::config::Config;
use crate::pid_file::PidFile;
use crate::server::{RunOptions, Server};

use daemon::Daemon;

/// Starts inshd.
pub struct Start {}

impl Start {
    /// Start inshd.
    pub fn run(options: &StartOptions) -> Result<(), StartError> {
        log::info!("Starting inshd...");

        if options.force {
            Server::cleanup();
        }

        if options.foreground {
            return Self::serve(options);
        }

        Self::daemonize(options)
    }

    /// Start a daemon which runs inshd again in the foreground.
    fn daemonize(options: &StartOptions) -> Result<(), StartError> {
        let program: PathBuf = match current_exe() {
            Ok(program) => program,
            Err(error) => {
                let error = StartError::FailedToFindProgram(error);
                log::error!("{}", error);
                return Err(error);
            }
        };

        log::info!("Daemonizing...");
        let daemon: Daemon = Daemon::builder()
            .program(program)
            .args(options.daemon_args.clone())
            .build();
        if let Err(error) = daemon.execute() {
            let error = StartError::FailedToDaemonize(error);
            log::error!("{}", error);
            return Err(error);
        }

        log::info!("Daemonized inshd.");
        log::info!("Started inshd.");
        Ok(())
    }

    /// Serve clients until stopped.
    fn serve(options: &StartOptions) -> Result<(), StartError> {
        // NOTE: This is written here rather than by the daemon so that inshd can be stopped and
        // checked on however it was started, and the daemon has the same process id as this.
        if let Err(error) = PidFile::write() {
            let error = StartError::FailedToWritePidFile(error);
            log::error!("{}", error);
            return Err(error);
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

    use std::ffi::OsString;

    use crate::args::StartArgs;

    use insh_api::LogRecord;

    use crossbeam::channel::Receiver;

    /// Options for starting inshd.
    pub struct StartOptions {
        /// Start even if already running.
        pub force: bool,
        /// Run in the foreground rather than as a daemon.
        pub foreground: bool,
        /// The arguments which the daemon runs inshd with.
        pub daemon_args: Vec<OsString>,
        /// A receiver of the log records which have been emitted.
        pub log_records_rx: Receiver<LogRecord>,
    }

    impl StartOptions {
        /// Return new start options.
        pub fn new(
            start_args: &StartArgs,
            daemon_args: Vec<OsString>,
            log_records_rx: Receiver<LogRecord>,
        ) -> Self {
            StartOptions {
                force: start_args.force,
                foreground: start_args.foreground,
                daemon_args,
                log_records_rx,
            }
        }
    }
}
pub use start_options::StartOptions;

mod start_error {
    //! A failure to start inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;

    use crate::config::LoadError as ConfigLoadError;
    use crate::server::RunError;

    use daemon::Error as DaemonError;

    /// A failure to start inshd.
    #[allow(clippy::enum_variant_names)]
    pub enum StartError {
        /// A failure to find the inshd executable for the daemon to run.
        FailedToFindProgram(IOError),
        /// A failure to daemonize the inshd server.
        FailedToDaemonize(DaemonError),
        /// A failure to write the pid file.
        FailedToWritePidFile(IOError),
        /// A failure to load the configuration.
        FailedToLoadConfig(ConfigLoadError),
        /// A failure to run the inshd server.
        FailedToRunServer(RunError),
    }

    impl Display for StartError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToFindProgram(error) => {
                    write!(formatter, "Failed to find the inshd executable: {}.", error)
                }
                Self::FailedToDaemonize(error) => {
                    write!(formatter, "Failed to daemonize the process: {}.", error)
                }
                Self::FailedToWritePidFile(error) => {
                    write!(formatter, "Failed to write the pid file: {}.", error)
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

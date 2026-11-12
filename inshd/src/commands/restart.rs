//! Restarts inshd.
use crate::commands::start::Start;
use crate::commands::stop::{Stop, StopError};

/// Restarts inshd.
pub struct Restart {}

impl Restart {
    /// Restart inshd.
    pub fn run(options: &RestartOptions) -> Result<(), RestartError> {
        if let Err(error) = Stop::run(&options.stop_options) {
            match error {
                StopError::NotRunning => {}
                _ => {
                    return Err(RestartError::FailedToStop(error));
                }
            }
        }

        if let Err(error) = Start::run(&options.start_options) {
            return Err(RestartError::FailedToStart(error));
        }

        Ok(())
    }
}

mod restart_options {
    //! Options for restarting inshd.

    use std::ffi::OsString;

    use crate::args::RestartArgs;
    use crate::commands::start::StartOptions;
    use crate::commands::stop::StopOptions;

    use insh_api::LogRecord;

    use crossbeam::channel::Receiver;

    /// Options for restarting inshd.
    pub struct RestartOptions {
        /// Options for starting inshd.
        pub start_options: StartOptions,
        /// Options for stopping inshd.
        pub stop_options: StopOptions,
    }

    impl RestartOptions {
        /// Return new restart options.
        pub fn new(
            restart_args: &RestartArgs,
            daemon_args: Vec<OsString>,
            log_records_rx: Receiver<LogRecord>,
        ) -> Self {
            Self {
                start_options: StartOptions {
                    force: restart_args.force,
                    foreground: false,
                    daemon_args,
                    log_records_rx,
                },
                stop_options: StopOptions {
                    force: restart_args.force,
                    timeout: restart_args.timeout,
                },
            }
        }
    }
}
pub use restart_options::RestartOptions;

mod restart_error {
    //! An error restarting inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};

    use crate::commands::start::StartError;
    use crate::commands::stop::StopError;

    /// An error restarting inshd.
    pub enum RestartError {
        /// An error stopping inshd.
        FailedToStop(StopError),
        /// An error starting inshd.
        FailedToStart(StartError),
    }

    impl Display for RestartError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToStop(error) => {
                    write!(formatter, "Failed to stop inshd: {}", error)
                }
                Self::FailedToStart(error) => {
                    write!(formatter, "Failed to start inshd: {}", error)
                }
            }
        }
    }
}
use restart_error::RestartError;

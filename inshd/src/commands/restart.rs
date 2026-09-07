//! Restarts inshd.
use crate::commands::start::Start;
use crate::commands::stop::{Stop, StopError};

/// Restarts inshd.
pub struct Restart {}

impl Restart {
    /// Restart inshd.
    pub fn run(options: &mut RestartOptions) -> Result<(), RestartError> {
        if let Err(error) = Stop::run(&options.stop_options) {
            match error {
                StopError::NotRunning => {}
                _ => {
                    return Err(RestartError::FailedToStop(error));
                }
            }
        }

        if let Err(error) = Start::run(&mut options.start_options) {
            return Err(RestartError::FailedToStart(error));
        }

        Ok(())
    }
}

mod restart_options {
    //! Options for restarting inshd.

    use crate::args::RestartArgs;
    use crate::commands::start::StartOptions;
    use crate::commands::stop::StopOptions;

    use insh_api::LogRecord;

    use crossbeam::channel::Receiver;
    use flexi_logger::LoggerHandle;

    /// Options for restarting inshd.
    pub struct RestartOptions<'a> {
        /// Options for starting inshd.
        pub start_options: StartOptions<'a>,
        /// Options for stopping inshd.
        pub stop_options: StopOptions,
    }

    impl<'a> RestartOptions<'a> {
        /// Return new restart options.
        pub fn new(
            logger_handle: &'a mut LoggerHandle,
            log_records_rx: Receiver<LogRecord>,
            restart_args: &RestartArgs,
        ) -> Self {
            Self {
                start_options: StartOptions {
                    force: restart_args.force,
                    logger_handle,
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

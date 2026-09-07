//! Stops inshd.
use std::io::Write;
use std::thread::{self, JoinHandle};

use crate::pid_file::{PidFile, ReadError as PidFileReadError};
use crate::pid_waiter::{PidWaitResult, PidWaiter};

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

/// Stops inshd.
pub struct Stop {}

impl Stop {
    /// Stop inshd.
    pub fn run(options: &StopOptions) -> Result<StopSummary, StopError> {
        log::info!("Stopping inshd...");

        let pid: u64 = match PidFile::read() {
            Ok(pid) => pid,
            Err(error) => match error {
                PidFileReadError::PidFileNotFound => {
                    let error = StopError::NotRunning;
                    log::error!("{}", error);
                    return Err(error);
                }
                _ => {
                    let error = StopError::FailedToGetPid(error);
                    log::error!("{}", error);
                    return Err(error);
                }
            },
        };

        let pid: Pid = Pid::from_raw(pid.try_into().unwrap());

        let signal: Signal = match options.force {
            true => signal::SIGKILL,
            false => signal::SIGTERM,
        };

        // Start a thread to wait for the process with the PID to terminate.
        let (pid_waiter_stop_rx, mut pid_waiter_stop_tx) = os_pipe::pipe().unwrap();
        let mut pid_waiter = PidWaiter::builder()
            .pid(pid)
            .timeout(options.timeout)
            .stop_rx(pid_waiter_stop_rx)
            .build();
        let pid_waiter_handle: JoinHandle<PidWaitResult> = thread::Builder::new()
            .name(String::from("pid-waiter"))
            .spawn(move || pid_waiter.run())
            .unwrap();

        // TODO: Wait for the pid waiter to be waiting.

        // Send inshd a signal to stop.
        log::debug!("Sending {} to inshd (pid {})...", signal, pid);
        if let Err(errno) = signal::kill(pid, signal) {
            let _ = pid_waiter_stop_tx.write(&[1; 1]).unwrap();
            let _ = pid_waiter_handle.join();
            let error = StopError::ErrorSendingSignal { pid, signal, errno };
            log::error!("{}", error);
            return Err(error);
        };
        log::debug!("Sent {} to inshd (pid {}).", signal, pid);

        let pid_wait_result: PidWaitResult = pid_waiter_handle.join().unwrap();
        match pid_wait_result {
            Ok(pid_wait_success) => {
                let summary = StopSummary::builder()
                    .signal(signal)
                    .waited(pid_wait_success.waited)
                    .build();
                log::info!("{}", summary);
                return Ok(summary);
            }
            Err(pid_wait_error) => {
                let error = StopError::WaitError(pid_wait_error);
                log::error!("{}", error);
                return Err(error);
            }
        }
    }
}

mod stop_options {
    //! Options for stopping inshd.

    use std::time::Duration;

    use crate::args::StopArgs;

    /// Options for stopping inshd.
    pub struct StopOptions {
        /// Force stop (with SIGKILL).
        pub force: bool,
        /// How long to wait for the inshd main process to stop.
        pub timeout: Duration,
    }

    impl StopOptions {
        /// Return new stop options.
        pub fn new(stop_args: &StopArgs) -> Self {
            Self {
                force: stop_args.force,
                timeout: stop_args.timeout,
            }
        }
    }
}
pub use stop_options::StopOptions;

mod stop_summary {
    //! A summary of stopping inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::time::Duration;

    use nix::sys::signal::Signal;
    use typed_builder::TypedBuilder;

    /// A summary of stopping inshd.
    #[derive(TypedBuilder)]
    pub struct StopSummary {
        /// The signal send to inshd.
        signal: Signal,
        /// The amount of time spent waiting for the inshd to stop.
        waited: Duration,
    }

    impl Display for StopSummary {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            write!(
                formatter,
                "Sent {} to inshd and it stopped after {:.2} secs.",
                self.signal.as_str(),
                self.waited.as_secs_f32()
            )
        }
    }
}
use stop_summary::StopSummary;

mod stop_error {
    //! An error stopping inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};

    use crate::pid_file::ReadError as PidFileReadError;
    use crate::pid_waiter::PidWaitError;

    use nix::errno::Errno;
    use nix::sys::signal::Signal;
    use nix::unistd::Pid;

    /// An error stopping inshd.
    pub enum StopError {
        /// Inshd is not running.
        NotRunning,
        /// A failure to get the pid of inshd.
        FailedToGetPid(PidFileReadError),
        /// An error sending a signal to inshd.
        ErrorSendingSignal {
            /// The signal sent to the main inshd process.
            signal: Signal,
            /// The pid of inshd.
            pid: Pid,
            /// The errno from sending the signal.
            errno: Errno,
        },
        /// An error waiting for inshd to stop.
        WaitError(PidWaitError),
    }

    impl Display for StopError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::NotRunning => {
                    write!(formatter, "Inshd is not running.")
                }
                Self::FailedToGetPid(read_error) => {
                    write!(formatter, "{}", read_error)
                }
                Self::ErrorSendingSignal { signal, pid, errno } => {
                    write!(
                        formatter,
                        "Error send signal {} to inshd (pid {}): {}",
                        signal, pid, errno
                    )
                }
                Self::WaitError(pid_wait_error) => match pid_wait_error {
                    PidWaitError::Timeout => {
                        write!(formatter, "Timed out waiting for inshd to stop.")
                    }
                    _ => {
                        write!(
                            formatter,
                            "Error waiting for inshd to stop: {}",
                            pid_wait_error
                        )
                    }
                },
            }
        }
    }
}
pub use stop_error::StopError;

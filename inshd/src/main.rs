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

use crate::args::{Args, Command};
use crate::logging::configure_logging;
use crate::paths::INSHD_PID_FILE;
use crate::server::{RunOptions, Server};

use std::fs::File;
use std::io::{Read, Write};
use std::thread::{self, JoinHandle};

use clap::Parser;
use daemonize::{Daemonize, Outcome as DaemonizeOutcome};
use flexi_logger::{Duplicate as LogDuplicate, LoggerHandle};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

#[macro_use]
extern crate lazy_static;

/// The main entry point.
fn main() {
    let args: Args = Args::parse();

    // Configure a basic stdout logger. The logger configured for the inshd process can be more
    // sophistiacted, but for commands like start, stop, etc. we just want logging to go to stdout.
    let mut logger_handle: LoggerHandle = configure_logging(&args.log_options());

    let exit_code: i32 = match args.command() {
        Command::Start(start_args) => {
            let mut options: StartOptions = StartOptions::new(&mut logger_handle, start_args);
            if start(&mut options).is_err() {
                1
            } else {
                0
            }
        }
        Command::Stop(stop_args) => {
            let options: StopOptions = StopOptions::new(stop_args);
            if stop(&options).is_err() {
                1
            } else {
                0
            }
        }
        Command::Restart(restart_args) => {
            let mut options: RestartOptions = RestartOptions::new(&mut logger_handle, restart_args);
            if restart(&mut options).is_err() {
                1
            } else {
                0
            }
        }
        Command::Status => match status() {
            Ok(status) => {
                log::info!("{}", status);
                0
            }
            Err(error) => {
                log::error!("{}", error);
                1
            }
        },
    };

    // NOTE: We should just be able to return a `std::process::ExitCode`, but the daemon process
    // does not exit if we do that?
    std::process::exit(exit_code);
}

/// Start inshd.
fn start(options: &mut StartOptions) -> Result<(), StartError> {
    log::info!("Starting inshd...");

    if options.force {
        Server::cleanup();
    }

    // Daemonize the process.
    log::info!("Daemonizing...");
    let daemonize = Daemonize::new()
        .pid_file(&*INSHD_PID_FILE)
        .chown_pid_file(true);
    match daemonize.execute() {
        DaemonizeOutcome::Parent(result) => {
            if let Err(error) = result {
                let error = StartError::FailedToDaemonize(error);
                log::error!("{}", error);
                return Err(error);
            }
            log::info!("Daemonized inshd.");

            log::info!("Started inshd.");
            return Ok(());
        }
        DaemonizeOutcome::Child(result) => {
            let _ = options
                .logger_handle
                .adapt_duplication_to_stdout(LogDuplicate::None);

            if let Err(error) = result {
                let error = StartError::FailedToDaemonize(error);
                log::error!("{}", error);
                return Err(error);
            }
        }
    }

    let server = Server::new();
    let run_options: RunOptions = RunOptions::default();
    if let Err(error) = server.run(run_options) {
        let error = StartError::FailedToRunServer(error);
        log::error!("{}", error);
        return Err(error);
    }

    log::info!("Inshd stopped.");

    Ok(())
}

mod start_options {
    //! Options for starting inshd.

    use crate::args::StartArgs;

    use flexi_logger::LoggerHandle;

    /// Options for starting inshd.
    pub struct StartOptions<'a> {
        /// Start even if already running.
        pub force: bool,
        /// The basic logger handle.
        pub logger_handle: &'a mut LoggerHandle,
    }

    impl<'a> StartOptions<'a> {
        /// Return new start options.
        pub fn new(logger_handle: &'a mut LoggerHandle, start_args: &StartArgs) -> Self {
            StartOptions {
                force: start_args.force,
                logger_handle,
            }
        }
    }
}
pub use start_options::StartOptions;

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
fn stop(options: &StopOptions) -> Result<StopSummary, StopError> {
    log::info!("Stopping inshd...");

    let pid: u64 = match _get_inshd_pid() {
        Ok(pid) => pid,
        Err(error) => match error {
            GetPidError::PidFileNotFound => {
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

mod stop_options {
    //! Options for stopping inshd.

    use crate::args::StopArgs;
    use std::time::Duration;

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

mod pid_waiter {
    //! Waits for a process with a pid to terminate.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;
    #[cfg(target_os = "linux")]
    use std::os::fd::AsRawFd;
    use std::time::{Duration, Instant};

    use nix::errno::Errno;
    #[cfg(target_os = "linux")]
    use nix::libc::{syscall, SYS_pidfd_open};
    #[cfg(target_os = "linux")]
    use nix::sys::select::{select, FdSet};
    #[cfg(target_os = "macos")]
    use nix::sys::signal::kill;
    #[cfg(target_os = "linux")]
    use nix::sys::time::TimeVal;
    use nix::unistd::Pid;
    use os_pipe::PipeReader;
    use typed_builder::TypedBuilder;

    /// Waits for a process with a pid to terminate.
    #[derive(TypedBuilder)]
    pub struct PidWaiter {
        /// The process id of the process to wait for.
        pid: Pid,
        /// A timeout for waiting.
        timeout: Duration,
        /// A receiver of a sentinel value to stop waiting.
        stop_rx: PipeReader,
    }

    impl PidWaiter {
        /// Run the pid waiter.
        pub fn run(&mut self) -> PidWaitResult {
            let start = Instant::now();

            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            {
                compile_error!("Only Linux and MacOS are supported.");
            }

            #[cfg(target_os = "linux")]
            {
                // Obtain a file descriptor for the pid.
                log::debug!("Getting fd for pid {}...", self.pid);
                let pid_fd: i32;
                unsafe {
                    let flags = 0;
                    let result: i64 = syscall(SYS_pidfd_open, self.pid, flags);
                    if result == -1 {
                        let error = IOError::last_os_error();
                        return Err(PidWaitError::FailedToGetPidFd(error));
                    }
                    pid_fd = result.try_into().unwrap();
                }
                log::debug!("Got fd {} for pid {}.", pid_fd, self.pid);

                let mut readfds = FdSet::new();
                let stop_fd: i32 = self.stop_rx.as_raw_fd();
                readfds.insert(stop_fd);
                readfds.insert(pid_fd);

                let timeout_secs: u64 = self.timeout.as_secs();
                let timeout_secs: i64 = timeout_secs.try_into().unwrap_or(i64::MAX);
                let mut time_val = TimeVal::new(timeout_secs, 0);

                select(None, &mut readfds, None, None, &mut time_val).unwrap();

                if readfds.contains(stop_fd) {
                    return Err(PidWaitError::Stopped);
                }

                if readfds.contains(pid_fd) {
                    let waited: Duration = start.elapsed();
                    return Ok(PidWaitSuccess::builder().waited(waited).build());
                }

                return Err(PidWaitError::Timeout);
            }

            #[cfg(target_os = "macos")]
            {
                let signal = None;

                loop {
                    match kill(self.pid, signal) {
                        Ok(_) => {}
                        Err(error) => {
                            match error {
                                Errno::ESRCH => {
                                    // There is no process or process group corresponding to the pid.
                                    let waited: Duration = start.elapsed();
                                    return Ok(PidWaitSuccess::builder().waited(waited).build());
                                }
                                _ => return Err(PidWaitError::ErrorWaitingOnPid(error)),
                            }
                        }
                    };

                    let elapsed = start.elapsed();
                    if elapsed >= self.timeout {
                        return Err(PidWaitError::Timeout);
                    }
                }
            }
        }
    }

    /// The result of successfully waiting for a process to terminate.
    #[derive(TypedBuilder)]
    pub struct PidWaitSuccess {
        /// The amount of spent waiting for the process to terminate.
        pub waited: Duration,
    }

    /// An error waiting for a process with a given pid to terminate.
    #[allow(dead_code)]
    pub enum PidWaitError {
        /// The pid waiter was stopped.
        Stopped,
        /// An error getting a file descriptor for the PID.
        FailedToGetPidFd(IOError),
        /// An error waiting for a process to terminate.
        ErrorWaitingOnPid(Errno),
        /// The timeout was exceeded.
        Timeout,
    }

    impl Display for PidWaitError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::Stopped => {
                    write!(formatter, "Stopped before process terminated.")
                }
                Self::FailedToGetPidFd(io_error) => {
                    write!(formatter, "Failed to get pid fd for process: {}.", io_error)
                }
                Self::ErrorWaitingOnPid(errno) => {
                    write!(
                        formatter,
                        "Encountered an error while waiting on the process: {}",
                        errno
                    )
                }
                Self::Timeout => {
                    write!(formatter, "Timed out waiting for process to terminate.")
                }
            }
        }
    }

    /// The result of waiting for a process with a given pid to terminate.
    pub type PidWaitResult = Result<PidWaitSuccess, PidWaitError>;
}
use pid_waiter::{PidWaitResult, PidWaiter};

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
    use crate::get_pid_error::GetPidError;
    use crate::pid_waiter::PidWaitError;

    use std::fmt::{Display, Error as FmtError, Formatter};

    use nix::errno::Errno;
    use nix::sys::signal::Signal;
    use nix::unistd::Pid;

    /// An error stopping inshd.
    pub enum StopError {
        /// Inshd is not running.
        NotRunning,
        /// A failure to get the pid of inshd.
        FailedToGetPid(GetPidError),
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
                Self::FailedToGetPid(get_pid_error) => {
                    write!(formatter, "{}", get_pid_error)
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
use stop_error::StopError;

/// Restart inshd.
fn restart(options: &mut RestartOptions) -> Result<(), RestartError> {
    if let Err(error) = stop(&options.stop_options) {
        match error {
            StopError::NotRunning => {}
            _ => {
                return Err(RestartError::FailedToStop(error));
            }
        }
    }

    if let Err(error) = start(&mut options.start_options) {
        return Err(RestartError::FailedToStart(error));
    }

    Ok(())
}

mod restart_options {
    //! Options for restarting inshd.

    use super::{StartOptions, StopOptions};
    use crate::args::RestartArgs;

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
        pub fn new(logger_handle: &'a mut LoggerHandle, restart_args: &RestartArgs) -> Self {
            Self {
                start_options: StartOptions {
                    force: restart_args.force,
                    logger_handle,
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
    use super::{StartError, StopError};

    use std::fmt::{Display, Error as FmtError, Formatter};

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

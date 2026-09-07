//! Waits for a process with a pid to terminate.

use std::fmt::{Display, Error as FmtError, Formatter};
use std::io::Error as IOError;
#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, BorrowedFd};
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
#[allow(dead_code)]
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
            let stop_fd: BorrowedFd = unsafe { BorrowedFd::borrow_raw(self.stop_rx.as_raw_fd()) };
            let pid_fd: BorrowedFd = unsafe { BorrowedFd::borrow_raw(pid_fd) };
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

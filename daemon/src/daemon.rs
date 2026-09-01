//! Turns the current process into a daemon.

use crate::{Error, Outcome};

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};

use nix::sys::stat::{umask, Mode};
use nix::unistd::{chdir, dup2_stderr, dup2_stdin, dup2_stdout, fork, getpid, setsid, ForkResult};

/// The file that the standard streams of the daemon are redirected to.
const NULL: &str = "/dev/null";

/// The umask that the daemon runs with.
const UMASK: Mode = Mode::from_bits_truncate(0o027);

/// Turns the current process into a daemon.
pub struct Daemon {
    /// The file to write the pid of the daemon to.
    pid_file: Option<PathBuf>,
    /// The working directory of the daemon.
    working_directory: PathBuf,
}

impl Default for Daemon {
    fn default() -> Self {
        return Self {
            pid_file: None,
            working_directory: PathBuf::from("/"),
        };
    }
}

impl Daemon {
    /// Return a new daemonizer.
    pub fn new() -> Self {
        return Self::default();
    }

    /// Write the pid of the daemon to the given file.
    pub fn pid_file<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.pid_file = Some(path.as_ref().to_path_buf());
        return self;
    }

    /// Run the daemon from the given working directory.
    pub fn working_directory<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.working_directory = path.as_ref().to_path_buf();
        return self;
    }

    /// Daemonize the current process.
    ///
    /// This double forks so that the daemon is orphaned and cannot reacquire a controlling
    /// terminal. The original process gets [`Outcome::Parent`] and the daemon gets
    /// [`Outcome::Child`]. The intermediate process exits without returning.
    ///
    /// Call this before spawning any threads. Only the calling thread survives a fork, so any
    /// thread that is already running is gone in the daemon, and any lock it happened to be
    /// holding stays locked forever.
    pub fn execute(self) -> Result<Outcome, Error> {
        // Fork so that the daemon is not the process group leader (a prerequisite of `setsid`).
        match unsafe { fork() }.map_err(Error::Fork)? {
            ForkResult::Parent { .. } => {
                return Ok(Outcome::Parent);
            }
            ForkResult::Child => {}
        }

        // Detach from the controlling terminal.
        setsid().map_err(Error::SetSid)?;

        // Fork again so that the daemon cannot reacquire a controlling terminal.
        match unsafe { fork() }.map_err(Error::Fork)? {
            ForkResult::Parent { .. } => {
                // NOTE: Exit without unwinding so that nothing this process shares with the daemon
                // (buffers, locks, destructors) is torn down twice.
                unsafe { nix::libc::_exit(0) };
            }
            ForkResult::Child => {}
        }

        umask(UMASK);
        chdir(self.working_directory.as_path()).map_err(Error::Chdir)?;

        if let Some(pid_file) = &self.pid_file {
            let mut file: File = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(pid_file)
                .map_err(Error::PidFile)?;
            writeln!(file, "{}", getpid()).map_err(Error::PidFile)?;
        }

        // Redirect the standard streams to `/dev/null`.
        let null_read: File = File::open(NULL).map_err(Error::Redirect)?;
        let null_write: File = OpenOptions::new()
            .write(true)
            .open(NULL)
            .map_err(Error::Redirect)?;
        dup2_stdin(null_read.as_fd()).map_err(|errno| Error::Redirect(errno.into()))?;
        dup2_stdout(null_write.as_fd()).map_err(|errno| Error::Redirect(errno.into()))?;
        dup2_stderr(null_write.as_fd()).map_err(|errno| Error::Redirect(errno.into()))?;

        return Ok(Outcome::Child);
    }
}

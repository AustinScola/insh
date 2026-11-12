//! Turns the current process into a daemon.

use std::ffi::{CString, OsString};
use std::fs::{File, OpenOptions};
use std::os::fd::AsFd;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use crate::Error;

use nix::sys::stat::{umask, Mode};
use nix::unistd::{chdir, dup2_stderr, dup2_stdin, dup2_stdout, execv, fork, setsid, ForkResult};
use typed_builder::TypedBuilder;

/// The file that the standard streams of the daemon are redirected to.
const NULL: &str = "/dev/null";

/// The umask that the daemon runs with.
const UMASK: Mode = Mode::from_bits_truncate(0o027);

/// Turns the current process into a daemon.
#[derive(TypedBuilder)]
pub struct Daemon {
    /// The program which the daemon runs.
    #[builder(setter(into))]
    program: PathBuf,
    /// The arguments which the program is run with, not counting the program itself.
    #[builder(default)]
    args: Vec<OsString>,
    /// The working directory of the daemon.
    #[builder(default = PathBuf::from("/"), setter(into))]
    working_directory: PathBuf,
}

impl Daemon {
    /// Daemonize the current process.
    ///
    /// This double forks so that the daemon is orphaned and cannot reacquire a controlling
    /// terminal, and then the daemon replaces itself with the program. Only the original process
    /// returns, unless the daemon fails to run the program, in which case it returns the error.
    ///
    /// The program has to be run rather than the daemon carrying on with the code which forked it,
    /// because a process which has forked without running a program is not allowed to use much of
    /// macOS (anything which reaches the Objective-C runtime, which resolving a hostname does), and
    /// macOS aborts it when it tries.
    ///
    /// Call this before spawning any threads. Only the calling thread survives a fork, so any lock
    /// which another thread happens to be holding stays locked until the program is run.
    pub fn execute(self) -> Result<(), Error> {
        // NOTE: These are made before forking so that as little as possible happens between the
        // fork and running the program.
        let program: CString =
            CString::new(self.program.as_os_str().as_bytes()).map_err(Error::Argument)?;
        let mut argv: Vec<CString> = vec![program.clone()];
        for arg in &self.args {
            argv.push(CString::new(arg.as_bytes()).map_err(Error::Argument)?);
        }

        // Fork so that the daemon is not the process group leader (a prerequisite of `setsid`).
        match unsafe { fork() }.map_err(Error::Fork)? {
            ForkResult::Parent { .. } => {
                return Ok(());
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

        // Redirect the standard streams to `/dev/null`.
        let null_read: File = File::open(NULL).map_err(Error::Redirect)?;
        let null_write: File = OpenOptions::new()
            .write(true)
            .open(NULL)
            .map_err(Error::Redirect)?;
        dup2_stdin(null_read.as_fd()).map_err(|errno| Error::Redirect(errno.into()))?;
        dup2_stdout(null_write.as_fd()).map_err(|errno| Error::Redirect(errno.into()))?;
        dup2_stderr(null_write.as_fd()).map_err(|errno| Error::Redirect(errno.into()))?;

        // The session, working directory, umask, process id and standard streams all carry over.
        let Err(errno) = execv(&program, &argv);
        return Err(Error::Exec(errno));
    }
}

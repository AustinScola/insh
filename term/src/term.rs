//! The terminal.

use std::ffi::c_int;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::fs::File;
use std::io::{self, Error as IOError, Read, Stdin};
use std::os::fd::AsRawFd;
use std::os::fd::{AsFd, BorrowedFd, IntoRawFd, RawFd};
use std::time::Duration;

use crate::event::{KeyEvent, ParsedTermEvent, TermEvent, TermEventParseError};

use size::Size;

use libc::{ioctl, winsize as WindowSize, TIOCGWINSZ};
use nix::errno::Errno;
use nix::libc;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use nix::sys::signal::{signal, SigHandler, Signal};
use nix::unistd::{pipe, read, write};
use nix::Result as NixResult;
use termios::*;
use typed_builder::TypedBuilder;

// TODO: Make sure we close these?
/// The read end of the resize pipe.
static mut RESIZED_RX: Option<RawFd> = None;
/// The write end of the resize pipe.
static mut RESIZED_TX: Option<RawFd> = None;

/// The terminal.
#[derive(TypedBuilder)]
pub struct Term {
    /// The standard input.
    #[builder(setter(skip), default=io::stdin())]
    stdin: Stdin,
    /// The bytes read from the terminal but not parsed yet.
    #[builder(setter(skip), default)]
    bytes: Vec<u8>,
    /// The attributes of the terminal.
    #[builder(setter(skip), default=Termios::from_fd(io::stdin().as_raw_fd()).unwrap())]
    termios: Termios,
    /// The saved attributes of the terminal.
    #[builder(setter(skip), default)]
    saved_termios: Option<Termios>,
    /// The read end of the pipe which the handler for the signal that the terminal was resized
    /// writes to, so that a resize can be waited for alongside input.
    #[builder(setter(skip), default=Term::handle_resizes())]
    resized_rx: RawFd,
    /// How long to wait for the rest of an escape sequence before deciding that the bytes which
    /// have been read are all that the terminal is going to send. A press of the escape key is
    /// indistinguishable from the start of a sequence until this runs out.
    #[builder(default=Term::DEFAULT_ESCAPE_TIMEOUT)]
    escape_timeout: Duration,
}

impl Term {
    /// Return a new terminal.
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// How long to wait for the rest of an escape sequence by default.
    pub const DEFAULT_ESCAPE_TIMEOUT: Duration = Duration::from_millis(50);

    /// How many bytes to read from the terminal at a time. Pasted text is the only input which is
    /// ever anywhere near this long, and it does not matter if it takes more than one read.
    const READ_SIZE: usize = 4096;

    /// Set up the pipe and the signal handler which report that the terminal was resized, and
    /// return the end of the pipe which is read from.
    fn handle_resizes() -> RawFd {
        // Create a pipe for the SIGWINCH signal handler to communicate with the rest of the code.
        let (resized_rx, resized_tx) = pipe().unwrap();
        let (resized_rx, resized_tx): (RawFd, RawFd) =
            (resized_rx.into_raw_fd(), resized_tx.into_raw_fd());
        unsafe {
            (RESIZED_RX, RESIZED_TX) = (Some(resized_rx), Some(resized_tx));
        }

        // Set up handling of window size changes.
        unsafe {
            let handler = SigHandler::Handler(_handle_sigwinch);
            if let Err(_error) = signal(Signal::SIGWINCH, handler) {
                todo!();
            }
        }

        resized_rx
    }

    /// Return the next event from the terminal.
    pub fn read(&mut self) -> Result<TermEvent, ReadError> {
        loop {
            // Try to parse an event out of the bytes which have already been read, and work out
            // how long it is worth waiting for more of them if there is not one yet.
            let timeout: PollTimeout = match ParsedTermEvent::try_from(&self.bytes[..]) {
                Ok(ParsedTermEvent { event, len }) => {
                    self.bytes.drain(..len);
                    return Ok(event);
                }
                Err(TermEventParseError::Unrecognized(len)) => {
                    self.bytes.drain(..len);
                    continue;
                }
                // Block indefinitely, because the terminal has committed to sending the rest of
                // the pasted text however long it takes to arrive.
                Err(TermEventParseError::NeedRestOfPaste) => PollTimeout::NONE,
                // Block indefinitely, because nothing has been read which could be an event yet.
                Err(TermEventParseError::Need(_)) if self.bytes.is_empty() => PollTimeout::NONE,
                Err(TermEventParseError::Need(_)) => {
                    PollTimeout::try_from(self.escape_timeout).unwrap_or(PollTimeout::MAX)
                }
            };

            let stdin_pollfd = PollFd::new(self.stdin.as_fd(), PollFlags::POLLIN);
            let resized_rx_pollfd =
                unsafe { PollFd::new(BorrowedFd::borrow_raw(self.resized_rx), PollFlags::POLLIN) };
            let mut pollfds: [PollFd; 2] = [stdin_pollfd, resized_rx_pollfd];

            let result: NixResult<c_int> = poll(&mut pollfds, timeout);
            match result {
                Err(Errno::EINTR) => {
                    continue;
                }
                Err(errno) => {
                    return Err(ReadError::PollError(errno));
                }
                // Nothing more is coming, so the bytes which have been read are not the start of a
                // sequence after all and the first one is a key of its own.
                Ok(0) => match self.bytes.first().copied() {
                    Some(byte) => {
                        self.bytes.remove(0);
                        return Ok(TermEvent::KeyEvent(KeyEvent::from(byte)));
                    }
                    // NOTE: There is only ever a timeout to run out when bytes are waiting, so
                    // there is always one to take, but polling again is the harmless thing to do
                    // rather than counting on that from thirty lines away.
                    None => {
                        continue;
                    }
                },
                Ok(_) => {}
            }

            let [stdin_events, resized_rx_events] = pollfds;

            if let Some(stdin_events) = stdin_events.revents() {
                if stdin_events.contains(PollFlags::POLLIN) {
                    let mut buffer: [u8; Self::READ_SIZE] = [0; Self::READ_SIZE];
                    match self.stdin.read(&mut buffer) {
                        Ok(read) => {
                            self.bytes.extend_from_slice(&buffer[..read]);
                        }
                        Err(error) => {
                            return Err(ReadError::IOError(error));
                        }
                    }
                    continue;
                }
            }

            if let Some(resized_rx_events) = resized_rx_events.revents() {
                let mut buffer: [u8; 1] = [0; 1];
                unsafe {
                    read(BorrowedFd::borrow_raw(self.resized_rx), &mut buffer[..]).unwrap();
                }
                if resized_rx_events.contains(PollFlags::POLLIN) {
                    let size: Size = match Term::size() {
                        Ok(size) => size,
                        Err(error) => {
                            return Err(ReadError::SizeError(error));
                        }
                    };
                    return Ok(TermEvent::Resize(size));
                }
            }
        }
    }

    /// Set the minimum number of bytes to read (VMIN).
    pub fn set_read_min(&mut self, min: u8) -> Result<(), SetReadMinError> {
        self.termios.c_cc[VMIN] = min;

        if let Err(error) = termios::tcsetattr(self.stdin.as_raw_fd(), TCSAFLUSH, &self.termios) {
            return Err(SetReadMinError::FailedToSetAttrs(error));
        }
        Ok(())
    }

    /// Set the read timeout (VTIME) measured in 1/10ths of a second (100ms).
    pub fn set_read_timeout(&mut self, timeout: u8) -> Result<(), SetReadTimeoutError> {
        self.termios.c_cc[VTIME] = timeout;

        if let Err(error) = termios::tcsetattr(self.stdin.as_raw_fd(), TCSAFLUSH, &self.termios) {
            return Err(SetReadTimeoutError::FailedToSetAttrs(error));
        }
        Ok(())
    }

    /// Remember the attributes which the terminal has now.
    pub fn save_attrs(&mut self) -> Result<(), SaveAttrsError> {
        let mut termios: Termios = Termios::from_fd(self.stdin.as_raw_fd()).unwrap();
        if let Err(error) = termios::tcgetattr(self.stdin.as_raw_fd(), &mut termios) {
            return Err(SaveAttrsError::FailedToGetAttrs(error));
        }
        self.saved_termios = Some(termios);
        Ok(())
    }

    /// Return the attributes which were saved, or `None` if they have not been.
    pub fn saved_attrs(&self) -> Option<SavedAttrs> {
        self.saved_termios.map(|termios| SavedAttrs {
            termios,
            fd: self.stdin.as_raw_fd(),
        })
    }

    /// Put the attributes which were saved back.
    pub fn restore_attrs(&mut self) -> Result<(), RestoreAttrsError> {
        let saved_attrs: SavedAttrs = match self.saved_attrs() {
            Some(saved_attrs) => saved_attrs,
            None => {
                return Err(RestoreAttrsError::AttrsNotSavedError);
            }
        };

        saved_attrs.restore()?;

        self.termios = saved_attrs.termios;
        Ok(())
    }

    /// Put the terminal into raw mode.
    // This implementation is based on https://viewsourcecode.org/snaptoken/kilo/02.enteringRawMode.html
    pub fn enable_raw(&mut self) -> Result<(), EnableRawError> {
        if let Err(error) = termios::tcgetattr(self.stdin.as_raw_fd(), &mut self.termios) {
            return Err(EnableRawError::FailedToGetAttrs(error));
        }

        self.termios.c_iflag &= !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
        self.termios.c_oflag &= !(OPOST);
        self.termios.c_cflag |= CS8;
        self.termios.c_lflag &= !(ECHO | ICANON | ISIG | IEXTEN);

        self.termios.c_cc[VMIN] = 0; // Min number of bytes before read() will return
        self.termios.c_cc[VTIME] = 0; // Max time to wait before read() returns (measured in 1/10ths of a second)

        if let Err(error) = termios::tcsetattr(self.stdin.as_raw_fd(), TCSAFLUSH, &self.termios) {
            return Err(EnableRawError::FailedToSetAttrs(error));
        }

        return Ok(());
    }

    /// Return the TOSTOP attribute.
    pub fn get_tostop_attr(&self) -> bool {
        (self.termios.c_cflag & TOSTOP) != 0
    }

    /// Return the size of the terminal.
    pub fn size() -> Result<Size, SizeError> {
        let file: File = File::open("/dev/tty").unwrap();
        let fd = file.as_raw_fd();

        let mut size: WindowSize = WindowSize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        unsafe {
            let result: i32 = ioctl(fd, TIOCGWINSZ, &mut size);
            if result == -1 {
                return Err(SizeError::IOError(io::Error::last_os_error()));
            }
        }
        return Ok(Size {
            rows: size.ws_row.into(),
            columns: size.ws_col.into(),
        });
    }
}

impl Default for Term {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle the signal that the terminal was resized.
extern "C" fn _handle_sigwinch(_signal: libc::c_int) {
    unsafe {
        if let Some(resized_tx_) = RESIZED_TX {
            // NOTE: There is probably a race condition here where Term could get dropped and close the fds?
            let buffer: [u8; 1] = [1; 1];
            write(BorrowedFd::borrow_raw(resized_tx_), &buffer[..]).unwrap();
        }
    }
}

/// The attributes which a terminal had before it was taken over, kept so that they can be put back
/// from somewhere which does not own the [`Term`] they came from. A panic handler is the one which
/// needs that, because it runs when the terminal is no longer reachable through anything else.
#[derive(Debug, Clone, Copy)]
pub struct SavedAttrs {
    /// The attributes.
    termios: Termios,
    /// The file descriptor of the terminal they are for.
    fd: RawFd,
}

impl SavedAttrs {
    /// Put the attributes back.
    pub fn restore(&self) -> Result<(), RestoreAttrsError> {
        if let Err(error) = termios::tcsetattr(self.fd, TCSAFLUSH, &self.termios) {
            return Err(RestoreAttrsError::FailedToSetAttrs(error));
        }
        Ok(())
    }
}

/// A terminal read error.
#[derive(Debug)]
pub enum ReadError {
    /// Reading from the terminal failed.
    IOError(IOError),
    /// The size of the resized terminal could not be got.
    SizeError(SizeError),
    /// Waiting for input or a resize failed.
    PollError(Errno),
}

impl Display for ReadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::IOError(error) => write!(
                formatter,
                "Encountered IO error while attempting to read terminal event: {}",
                error
            ),
            Self::SizeError(error) => write!(
                formatter,
                "Failed to get the size of the terminal: {}",
                error
            ),
            Self::PollError(error) => write!(formatter, "Polling failed: {}", error),
        }
    }
}

/// A read minimum error.
#[derive(Debug)]
pub enum SetReadMinError {
    /// The attributes of the terminal could not be set.
    FailedToSetAttrs(IOError),
}

/// A read timeout error.
#[derive(Debug)]
pub enum SetReadTimeoutError {
    /// The attributes of the terminal could not be set.
    FailedToSetAttrs(IOError),
}

/// An attribute saving error.
#[derive(Debug)]
pub enum SaveAttrsError {
    /// The attributes of the terminal could not be got.
    FailedToGetAttrs(IOError),
}

/// An attribute restoring error.
#[derive(Debug)]
pub enum RestoreAttrsError {
    /// There are no saved attributes to put back.
    AttrsNotSavedError,
    /// The attributes of the terminal could not be set.
    FailedToSetAttrs(IOError),
}

/// A raw mode error.
#[derive(Debug)]
pub enum EnableRawError {
    /// The attributes of the terminal could not be set.
    FailedToSetAttrs(IOError),
    /// The attributes of the terminal could not be got.
    FailedToGetAttrs(IOError),
}

/// A terminal size error.
#[derive(Debug)]
pub enum SizeError {
    /// Asking the terminal how big it is failed.
    IOError(IOError),
}

impl Display for SizeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::IOError(error) => write!(
                formatter,
                "Encountered IO error while attempting to get the size of the terminal: {}",
                error
            ),
        }
    }
}

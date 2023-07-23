use crate::event::TermEvent;
use size::Size;

use std::ffi::c_int;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::fs::File;
use std::io::{self, Error as IOError, ErrorKind as IOErrorKind, Read, Stdin};
use std::os::fd::AsRawFd;
use std::os::fd::RawFd;

use libc::{ioctl, winsize as WindowSize, TIOCGWINSZ};
use nix::errno::Errno;
use nix::libc;
use nix::poll::{poll, PollFd, PollFlags};
use nix::sys::signal::{signal, SigHandler, Signal};
use nix::unistd::{pipe, read, write};
use nix::Result as NixResult;

use termios::*;

// TODO: Make sure we close these?
static mut RESIZED_RX: Option<RawFd> = None;
static mut RESIZED_TX: Option<RawFd> = None;

pub struct Term {
    stdin: Stdin,
    buffer: [u8; 1],
    termios: Termios,
    saved_termios: Option<Termios>,
}

impl Term {
    pub fn new() -> Self {
        let stdin: Stdin = io::stdin();
        let termios: Termios = Termios::from_fd(stdin.as_raw_fd()).unwrap();

        // Create a pipe for the SIGWINCH signal handler to communicate with the rest of the code.
        let (resized_rx, resized_tx): (RawFd, RawFd) = pipe().unwrap();
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

        Self {
            stdin,
            buffer: [0; 1],
            termios,
            saved_termios: None,
        }
    }

    pub fn read(&mut self) -> Result<TermEvent, ReadError> {
        loop {
            let timeout = -1; //  block indefinitely
            let stdin_pollfd = PollFd::new(self.stdin.as_raw_fd(), PollFlags::POLLIN);
            let resized_rx_pollfd = unsafe { PollFd::new(RESIZED_RX.unwrap(), PollFlags::POLLIN) };
            let mut pollfds: [PollFd; 2] = [stdin_pollfd, resized_rx_pollfd];
            let result: NixResult<c_int> = poll(&mut pollfds, timeout);
            match result {
                Err(Errno::EINTR) => {
                    continue;
                }
                Err(errno) => {
                    return Err(ReadError::PollError(errno));
                }
                _ => {}
            }

            let [stdin_events, resized_rx_events] = pollfds;

            let stdin_events: Option<PollFlags> = stdin_events.revents();
            if let Some(stdin_events) = stdin_events {
                if stdin_events.contains(PollFlags::POLLIN) {
                    if let Err(error) = self.stdin.read_exact(&mut self.buffer) {
                        return Err(ReadError::IOError(error));
                    }
                    return Ok(TermEvent::try_from(&self.buffer[..]).unwrap());
                }
            }

            let resized_rx_events: Option<PollFlags> = resized_rx_events.revents();
            if let Some(resized_rx_events) = resized_rx_events {
                let mut buffer: [u8; 1] = [0; 1];
                unsafe {
                    read(RESIZED_RX.unwrap(), &mut buffer[..]).unwrap();
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

            unreachable!();
        }
    }

    pub fn try_read(&mut self) -> Result<Option<TermEvent>, ReadError> {
        if let Err(error) = self.stdin.read_exact(&mut self.buffer) {
            if error.kind() == IOErrorKind::Interrupted {
                return Ok(None);
            }
            return Err(ReadError::IOError(error));
        }
        return Ok(Some(TermEvent::try_from(&self.buffer[..]).unwrap()));
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

    pub fn save_attrs(&mut self) -> Result<(), SaveAttrsError> {
        let mut termios: Termios = Termios::from_fd(self.stdin.as_raw_fd()).unwrap();
        if let Err(error) = termios::tcgetattr(self.stdin.as_raw_fd(), &mut termios) {
            return Err(SaveAttrsError::FailedToGetAttrs(error));
        }
        self.saved_termios = Some(termios);
        Ok(())
    }

    pub fn restore_attrs(&mut self) -> Result<(), RestoreAttrsError> {
        let saved_termios: Termios = match self.saved_termios {
            Some(saved_termios) => saved_termios,
            None => {
                return Err(RestoreAttrsError::AttrsNotSavedError);
            }
        };

        if let Err(error) = termios::tcsetattr(self.stdin.as_raw_fd(), TCSAFLUSH, &saved_termios) {
            return Err(RestoreAttrsError::FailedToSetAttrs(error));
        }

        self.termios = saved_termios;
        Ok(())
    }

    // This implementation is based on https://viewsourcecode.org/snaptoken/kilo/02.enteringRawMode.html
    pub fn enable_raw(&mut self) -> Result<(), EnableRawError> {
        if let Err(error) = termios::tcgetattr(self.stdin.as_raw_fd(), &mut self.termios) {
            return Err(EnableRawError::FailedToGetAttrs(error));
        }

        self.termios.c_iflag &= !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
        self.termios.c_oflag &= !(OPOST);
        self.termios.c_cflag |= CS8;
        self.termios.c_lflag &= !(ECHO | ICANON | ISIG | IEXTEN);

        // self.termios.c_cc[VMIN] = 0; // Min number of bytes before read() will return
        // self.termios.c_cc[VTIME] = 1; // Max time to wait before read() returns (100ms)

        if let Err(error) = termios::tcsetattr(self.stdin.as_raw_fd(), TCSAFLUSH, &self.termios) {
            return Err(EnableRawError::FailedToSetAttrs(error));
        }

        return Ok(());
    }

    pub fn get_tostop_attr(&self) -> bool {
        (self.termios.c_cflag & TOSTOP) != 0
    }

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

extern "C" fn _handle_sigwinch(_signal: libc::c_int) {
    unsafe {
        if let Some(resized_tx_) = RESIZED_TX {
            // NOTE: There is probably a race condition here where Term could get dropped and close the fds?
            let buffer: [u8; 1] = [1; 1];
            write(resized_tx_, &buffer[..]).unwrap();
        }
    }
}

#[derive(Debug)]
pub enum ReadError {
    IOError(IOError),
    SizeError(SizeError),
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

#[derive(Debug)]
pub enum SetReadMinError {
    FailedToSetAttrs(IOError),
}

#[derive(Debug)]
pub enum SetReadTimeoutError {
    FailedToSetAttrs(IOError),
}

#[derive(Debug)]
pub enum SaveAttrsError {
    FailedToGetAttrs(IOError),
}

#[derive(Debug)]
pub enum RestoreAttrsError {
    AttrsNotSavedError,
    FailedToSetAttrs(IOError),
}

#[derive(Debug)]
pub enum EnableRawError {
    FailedToSetAttrs(IOError),
    FailedToGetAttrs(IOError),
}

#[derive(Debug)]
pub enum SizeError {
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

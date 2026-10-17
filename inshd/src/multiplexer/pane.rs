//! One of the rectangles a window is divided into, and the program running in it when it has one.

use std::ffi::{CString, OsString};
use std::fs::File;
use std::io::Error as IoError;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::path::PathBuf;

use insh_api::{PaneKind, PaneRow, PaneScreen};
use rend::Size;
use terminal_emulator::{Parser, Screen, ScreenProps};

use nix::libc::{self, ioctl, setenv, winsize as WindowSize, TIOCSWINSZ};
use nix::pty::{forkpty, ForkptyResult, Winsize};
use nix::sys::signal::{killpg, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::{chdir, execvp, Pid};

use crossbeam::channel::{self, Receiver, Sender};
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// The properties of a program which is run in a pane.
#[derive(TypedBuilder)]
pub struct Props {
    /// The program to run.
    pub program: OsString,
    /// The arguments to give it.
    #[builder(default)]
    pub args: Vec<OsString>,
    /// The directory to run it in.
    #[builder(default)]
    pub dir: Option<PathBuf>,
    /// How many rows of it are kept after they have scrolled off the top of it.
    pub buffer_size: usize,
}

/// A program running on a pseudoterminal, and the screen which what it writes means to show.
struct Program {
    /// A sender of what was typed, which the thread writing to the pseudoterminal takes from.
    ///
    /// NOTE: What was typed is handed to a thread of its own rather than written here, because a
    /// pseudoterminal only takes so much before it makes whoever is writing wait for the program
    /// to read some of it, and waiting here would stop everything.
    typed_tx: Sender<Vec<u8>>,
    /// A handle on the pseudoterminal, which is what it is told its size with.
    master: File,
    /// The program itself.
    child: Pid,
    /// What the bytes it writes mean to show.
    screen: Screen,
    /// Turns the bytes it writes into the things they say to do.
    parser: Parser,
    /// Whether it has finished.
    finished: bool,
}

/// One of the rectangles a window is divided into.
///
/// A pane holds a program running on a pseudoterminal, or nothing at all when it is one which a
/// client draws for itself. A client starts a program in a pane and the pane goes back to holding
/// nothing once that program has finished, which is what makes the browser what a pane shows
/// whenever there is nothing else to show in it.
pub struct Pane {
    /// Which pane it is.
    id: Uuid,
    /// How big it is.
    size: Size,
    /// The program running in it, which is nothing at all for a pane a client draws itself.
    program: Option<Program>,
}

impl Pane {
    /// What a program running in a pane is told the terminal it is in is.
    ///
    /// The screen of a pane does what this one does, so this is what a program is told to write
    /// for. Saying so is what stops one from asking for something which would not be understood.
    const TERM: &'static str = "xterm-256color";

    /// Return a pane of the given size with nothing running in it.
    pub fn empty(size: Size) -> Self {
        return Self {
            id: Uuid::new_v4(),
            size: Self::at_least_one(size),
            program: None,
        };
    }

    /// Return which pane it is.
    pub fn id(&self) -> Uuid {
        return self.id;
    }

    /// Return what it holds.
    pub fn kind(&self) -> PaneKind {
        return match self.program {
            Some(_) => PaneKind::Program,
            None => PaneKind::Component,
        };
    }

    /// Return a size which has at least one row and one column.
    fn at_least_one(size: Size) -> Size {
        return Size::new(size.rows.max(1), size.columns.max(1));
    }

    /// Return the program running in the pane, which is nothing at all when there is none.
    pub fn child(&self) -> Option<Pid> {
        return self.program.as_ref().map(|program| program.child);
    }

    /// Start a program on a pseudoterminal of its own in the pane, and return the pseudoterminals
    /// to read what it writes from and to write what was typed to.
    ///
    /// Whichever program was running in it already is asked to finish first, so that a pane never
    /// has two of them in it.
    // NOTE: clippy gets confused by the fork and complains some code is unreachable because of it.
    #[allow(unreachable_code)]
    pub fn run(&mut self, props: Props) -> Result<(File, File, Receiver<Vec<u8>>), IoError> {
        self.close();

        let size: Size = self.size;
        let window_size: Winsize = Winsize {
            ws_row: size.rows.try_into().unwrap_or(u16::MAX),
            ws_col: size.columns.try_into().unwrap_or(u16::MAX),
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let program: CString = CString::new(props.program.clone().into_vec())?;
        let mut args: Vec<CString> = vec![program.clone()];
        for arg in props.args {
            args.push(CString::new(arg.into_vec())?);
        }

        let term: CString = CString::new("TERM").unwrap();
        let term_value: CString = CString::new(Self::TERM).unwrap();

        let master: RawFd;
        let child: Pid;
        match unsafe { forkpty(&window_size, None) } {
            Ok(ForkptyResult::Parent {
                child: child_,
                master: master_,
            }) => {
                master = master_.into_raw_fd();
                child = child_;
            }
            Ok(ForkptyResult::Child) => {
                if let Some(dir) = &props.dir {
                    let _ = chdir(dir);
                }

                let overwrite = 1;
                unsafe {
                    setenv(term.as_ptr(), term_value.as_ptr(), overwrite);
                }

                let _ = execvp(&program, &args);
                // NOTE: Getting here means the program could not be run at all, and this is a fork
                // of the daemon, so it has to go rather than carry on as a second one of it.
                unsafe { libc::_exit(1) };
            }
            Err(error) => return Err(IoError::from(error)),
        }

        // NOTE: The handles are duplicates of each other so that dropping one of them does not
        // close what the others are using.
        let write_to: File = unsafe { File::from_raw_fd(master) };
        let read_from: File = unsafe { File::from_raw_fd(libc::dup(master)) };
        let resize_with: File = unsafe { File::from_raw_fd(libc::dup(master)) };

        let (typed_tx, typed_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel::unbounded();

        self.program = Some(Program {
            typed_tx,
            master: resize_with,
            child,
            screen: Screen::from(
                ScreenProps::builder()
                    .size(size)
                    .scrollback(props.buffer_size)
                    .build(),
            ),
            parser: Parser::default(),
            finished: false,
        });

        return Ok((read_from, write_to, typed_rx));
    }

    /// Look the given number of rows further back through what has scrolled off the top, and
    /// return whether it moved at all.
    pub fn look_back(&mut self, rows: isize) -> bool {
        return match &mut self.program {
            Some(program) => program.screen.look_back(rows),
            None => false,
        };
    }

    /// Take the bytes the program wrote as having been written on the screen.
    pub fn wrote(&mut self, bytes: &[u8]) {
        let program: &mut Program = match &mut self.program {
            Some(program) => program,
            None => return,
        };

        let actions = program.parser.feed(bytes);
        program.screen.feed(actions);
    }

    /// Send what was typed to the program.
    ///
    /// This only hands it to the thread which writes to the pseudoterminal, so it is never waited
    /// on however little of it the program is reading.
    pub fn write(&mut self, bytes: &[u8]) {
        let id: Uuid = self.id;
        let program: &mut Program = match &mut self.program {
            Some(program) if !program.finished => program,
            _ => return,
        };

        if let Err(error) = program.typed_tx.send(bytes.to_vec()) {
            log::warn!("Failed to send what was typed to pane {}: {}", id, error);
        }
    }

    /// Change how big the pane is, telling whichever program is running in it so.
    pub fn resize(&mut self, size: Size) {
        let size: Size = Self::at_least_one(size);
        if size == self.size {
            return;
        }
        self.size = size;

        let id: Uuid = self.id;
        let program: &mut Program = match &mut self.program {
            Some(program) => program,
            None => return,
        };
        program.screen.resize(size);

        let window_size: WindowSize = WindowSize {
            ws_row: size.rows.try_into().unwrap_or(u16::MAX),
            ws_col: size.columns.try_into().unwrap_or(u16::MAX),
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // NOTE: Nothing is sent to the program itself. The kernel signals whatever is running on
        // the pseudoterminal that it changed size, which is what a program watches for.
        let result: libc::c_int =
            unsafe { ioctl(program.master.as_raw_fd(), TIOCSWINSZ, &window_size) };
        if result == -1 {
            log::warn!(
                "Failed to tell pane {} that it changed size: {}",
                id,
                IoError::last_os_error()
            );
        }
    }

    /// Return what has changed about the screen since this was last asked for, which is nothing at
    /// all when nothing has or when there is no program in the pane.
    pub fn screen(&mut self) -> Option<PaneScreen> {
        let id: Uuid = self.id;
        let program: &mut Program = self.program.as_mut()?;

        let damaged = program.screen.take_damage();
        let bells: usize = program.screen.take_bells();

        if damaged.is_empty() && bells == 0 {
            return None;
        }

        let rows: Vec<PaneRow> = damaged
            .into_iter()
            .map(|row| {
                PaneRow::builder()
                    .row(row)
                    .line(program.screen.line(row).clone())
                    .build()
            })
            .collect();

        return Some(Self::pane_screen(id, &program.screen, rows, bells));
    }

    /// Return the whole of the screen, which is what a client which has not seen it before needs.
    pub fn whole_screen(&mut self) -> Option<PaneScreen> {
        let id: Uuid = self.id;
        let program: &mut Program = self.program.as_mut()?;

        // Asking for what changed as well takes it as having been sent, so that it is not sent
        // again straight after this.
        program.screen.take_damage();
        let bells: usize = program.screen.take_bells();

        let rows: Vec<PaneRow> = (0..program.screen.size().rows)
            .map(|row| {
                PaneRow::builder()
                    .row(row)
                    .line(program.screen.line(row).clone())
                    .build()
            })
            .collect();

        return Some(Self::pane_screen(id, &program.screen, rows, bells));
    }

    /// Return the rows along with everything else a client is told about the screen.
    fn pane_screen(id: Uuid, screen: &Screen, rows: Vec<PaneRow>, bells: usize) -> PaneScreen {
        return PaneScreen::builder()
            .pane(id)
            .rows(rows)
            .cursor(match screen.is_cursor_shown() {
                true => Some(screen.cursor()),
                false => None,
            })
            .modes(screen.modes())
            .title(screen.title().map(ToString::to_string))
            .bells(bells)
            .looking_back(screen.looking_back())
            .build();
    }

    /// Take the program as having finished and leave the pane holding nothing.
    ///
    /// NOTE: Nothing is waited for here. Whichever thread was reading the program is the one which
    /// waits for it, because waiting has no end of its own and doing it here would stop the one
    /// thread which serves every client.
    pub fn finished(&mut self) {
        self.program = None;
    }

    /// Ask whichever program is in the pane to finish, and leave the pane holding nothing.
    ///
    /// The whole group is asked rather than only the program itself, since a shell has whatever it
    /// is running in the foreground in a group of its own.
    pub fn close(&mut self) {
        let id: Uuid = self.id;
        let mut program: Program = match self.program.take() {
            Some(program) => program,
            None => return,
        };
        if program.finished {
            return;
        }
        program.finished = true;

        if let Err(error) = killpg(program.child, Signal::SIGHUP) {
            log::warn!("Failed to ask pane {} to finish: {}", id, error);
        }

        // NOTE: Waiting without hanging about is enough, because the reader is what notices the
        // program has gone and the pane is waited for properly then.
        let _ = waitpid(program.child, Some(WaitPidFlag::WNOHANG));
    }
}

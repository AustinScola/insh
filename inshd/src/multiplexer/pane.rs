//! One of the rectangles a window is divided into, and the program running in it when it has one.

use std::env;
use std::ffi::{CString, OsString};
use std::fs::{self, File};
use std::io::Error as IoError;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::path::PathBuf;

use insh_api::{PaneKind, PaneRow, PaneScreen};
use rend::{Location, Seeking, Size};
use terminal_emulator::{Parser, Screen, ScreenProps};

use nix::libc::{self, ioctl, winsize as WindowSize, TIOCSWINSZ};
use nix::pty::{forkpty, ForkptyResult, Winsize};
use nix::sys::signal::{killpg, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::{chdir, execve, Pid};

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
    /// The directory the program running in it, or which ran in it last, was started in.
    ///
    /// NOTE: This outlives the program, since a client which has not seen the pane before shows
    /// the browser in it once the program has finished, and this is where that was.
    dir: Option<PathBuf>,
}

impl Pane {
    /// What a program running in a pane is told the terminal it is in is.
    ///
    /// The screen of a pane does what this one does, so this is what a program is told to write
    /// for. Saying so is what stops one from asking for something which would not be understood.
    const TERM: &'static str = "xterm-256color";

    /// The environment variable which bash takes a function from, and the function, which makes
    /// running insh in a shell in a pane leave that shell rather than start insh inside of insh.
    ///
    /// NOTE: Whatever it was run with is ignored, since there is nothing to be done with it from
    /// inside of a pane yet.
    const NESTED_INSH: (&'static str, &'static str) = ("BASH_FUNC_insh%%", "() { exit; }");

    /// Return where the program is, the way a shell finds it on the path, which is the program
    /// itself when it is a path already or is not on the path at all.
    fn find(program: &OsString) -> OsString {
        if program.as_bytes().contains(&b'/') {
            return program.clone();
        }

        let found: Option<PathBuf> = env::var_os("PATH").and_then(|paths| {
            env::split_paths(&paths)
                .map(|dir| dir.join(program))
                .find(|path| {
                    fs::metadata(path).is_ok_and(|metadata| {
                        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                    })
                })
        });
        return found.map_or_else(|| program.clone(), PathBuf::into_os_string);
    }

    /// Return the environment a program is run in, which is the daemon's own with the terminal
    /// it is in and the function which keeps insh from being run inside of itself.
    fn env() -> Result<Vec<CString>, IoError> {
        let set: [(&str, &str); 2] = [("TERM", Self::TERM), Self::NESTED_INSH];

        let mut env: Vec<CString> = Vec::new();
        for (name, value) in env::vars_os() {
            if set.iter().any(|(set_name, _)| name == *set_name) {
                continue;
            }
            let mut variable: Vec<u8> = name.into_vec();
            variable.push(b'=');
            variable.extend(value.into_vec());
            env.push(CString::new(variable)?);
        }
        for (name, value) in set {
            env.push(CString::new(format!("{}={}", name, value))?);
        }

        return Ok(env);
    }

    /// Return a pane of the given size with nothing running in it.
    pub fn empty(size: Size) -> Self {
        return Self {
            id: Uuid::new_v4(),
            size: Self::at_least_one(size),
            program: None,
            dir: None,
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

    /// Return the directory the program running in it, or which ran in it last, was started in.
    pub fn dir(&self) -> Option<&PathBuf> {
        return self.dir.as_ref();
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
        self.dir = props.dir.clone();

        let size: Size = self.size;
        let window_size: Winsize = Winsize {
            ws_row: size.rows.try_into().unwrap_or(u16::MAX),
            ws_col: size.columns.try_into().unwrap_or(u16::MAX),
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // NOTE: Everything the program is run with is worked out before forking, since the
        // daemon has other threads and the fork of it may only do what is safe to do then, which
        // is not allocating or looking anything up.
        let program: CString = CString::new(props.program.clone().into_vec())?;
        let path: CString = CString::new(Self::find(&props.program).into_vec())?;
        let mut args: Vec<CString> = vec![program];
        for arg in props.args {
            args.push(CString::new(arg.into_vec())?);
        }
        let env: Vec<CString> = Self::env()?;

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

                let _ = execve(&path, &args, &env);
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

    /// Look the given number of rows further back through what has scrolled off the top, holding
    /// what is being looked at where it is or letting it go, and return whether it moved at all.
    pub fn look_back(&mut self, rows: isize, hold: bool) -> bool {
        let program: &mut Program = match &mut self.program {
            Some(program) => program,
            None => return false,
        };

        program.screen.hold(hold);
        return program.screen.look_back(rows);
    }

    /// Return the text written from the one spot to the other, the rows of which are counted from
    /// the first row the program ever wrote on, which is nothing at all when there is no program
    /// in the pane.
    pub fn text(&self, from: Location, to: Location) -> Option<String> {
        return self
            .program
            .as_ref()
            .map(|program| program.screen.text(from, to));
    }

    /// Return where the phrase is next written after the given spot, or before it when looking
    /// up, the rows of which are counted from the first row the program ever wrote on.
    pub fn search(&self, phrase: &str, from: Location, down: bool) -> Option<Location> {
        return self
            .program
            .as_ref()?
            .screen
            .search(&Seeking::from(phrase), from, down);
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
        let status_changed: bool = program.screen.take_status_changed();

        if damaged.is_empty() && bells == 0 && !status_changed {
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
        program.screen.take_status_changed();
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
            .first_shown_row(screen.first_shown_row())
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

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test]
    fn test_moving_the_cursor_without_writing_on_a_row_is_sent() {
        let mut pane: Pane = Pane::empty(Size::new(2, 8));
        let _pseudoterminals = pane
            .run(
                Props::builder()
                    .program(OsString::from("cat"))
                    .buffer_size(0)
                    .build(),
            )
            .unwrap();
        pane.wrote(b"abc");
        pane.screen();

        // This is what bash writes when the left arrow key is pressed.
        pane.wrote(b"\x08");

        let screen: PaneScreen = pane.screen().unwrap();
        assert_eq!(screen.rows().len(), 0);
        assert_eq!(
            screen.cursor(),
            Some(Location::builder().row(0).column(2).build())
        );
        // It was only moved the once, so there is nothing more to say about it.
        assert!(pane.screen().is_none());

        pane.close();
    }

    #[test_case("sh", true; "a program on the path")]
    #[test_case("/bin/sh", true; "a path already")]
    #[test_case("not-a-program-anywhere", false; "a program which is nowhere")]
    fn test_find(program: &str, found: bool) {
        let path: OsString = Pane::find(&OsString::from(program));

        assert_eq!(PathBuf::from(&path).is_absolute(), found);
    }

    #[test]
    fn test_the_terminal_is_set_once_in_the_environment() {
        let env: Vec<CString> = Pane::env().unwrap();

        let terms: Vec<&CString> = env
            .iter()
            .filter(|variable| variable.as_bytes().starts_with(b"TERM="))
            .collect();
        assert_eq!(terms.len(), 1);
        assert_eq!(
            terms[0].as_bytes(),
            format!("TERM={}", Pane::TERM).as_bytes()
        );
    }

    #[test]
    fn test_running_insh_in_bash_in_a_pane_leaves_bash() {
        let mut pane: Pane = Pane::empty(Size::new(2, 20));
        let (mut read_from, _write_to, _typed_rx) = pane
            .run(
                Props::builder()
                    .program(OsString::from("bash"))
                    .args(vec![
                        OsString::from("--norc"),
                        OsString::from("-c"),
                        OsString::from("insh browse; echo still here"),
                    ])
                    .buffer_size(0)
                    .build(),
            )
            .unwrap();

        let mut written: Vec<u8> = Vec::new();
        let _ = std::io::Read::read_to_end(&mut read_from, &mut written);

        assert!(!String::from_utf8_lossy(&written).contains("still here"));

        pane.close();
    }
}

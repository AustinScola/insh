use crate::ascii::ASCII;
use crate::component::Component;
use crate::event::Event;
use crate::output_forwarder::OutputForwarder;
use crate::program::{Program, ProgramCleanup, ProgramSetup};
use crate::program_monitor::{ProgramEvent, ProgramMonitor};
use crate::requester::Requester;
use crate::response_handler::ResponseHandler;
use crate::stopper::Stopper;
use crate::system_effect::SystemEffect;
use crate::term_event_forwarder::TermEventForwarder;
use crate::StdoutPipe;

use rend::{Fabric, Renderer, Size};
use term::{Term, TermEvent};

use std::collections::VecDeque;
use std::ffi::{c_int, CString, OsString};
use std::fs::File;
use std::io::{self, Error as IOError, Stdout, Write};
use std::os::fd::FromRawFd;
use std::os::fd::RawFd;
use std::os::unix::ffi::OsStringExt;
use std::panic;
use std::thread::{self, JoinHandle};

use crossbeam::channel::{self, Receiver, Sender};
use crossbeam::select;
use crossterm::cursor::{Hide as HideCursor, MoveTo as MoveCursorTo, Show as ShowCursor};
use crossterm::style::Print;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::terminal::{Clear as ClearTerminal, ClearType as TerminalClearType};
use crossterm::{ExecutableCommand, QueueableCommand};
use nix::libc;
use nix::libc::{ioctl, setenv, winsize as WindowSize, TIOCSWINSZ};
use nix::pty::{forkpty, ForkptyResult, Winsize};
use nix::unistd::Pid;
use nix::unistd::{chdir, execvp, ForkResult};
use typed_builder::TypedBuilder;
use uuid::Uuid;

#[derive(TypedBuilder)]
pub struct App {
    #[builder(setter(skip), default=Term::new())]
    term: Term,
    #[builder(setter(skip), default=io::stdout())]
    stdout: Stdout,
    #[builder(setter(skip), default=Renderer::new())]
    renderer: Renderer,

    #[builder(setter(skip), default)]
    unused_term_events: VecDeque<TermEvent>,

    #[builder(setter(skip), default)]
    size: Size,
}

impl App {
    pub fn run<Props, Request, Response>(
        &mut self,
        options: AppRunOptions<Props, Request, Response>,
    ) where
        Request: Send + 'static,
        Response: Send + 'static,
    {
        let AppRunOptions {
            mut root,
            starting_effects,
            starting_term_events,
            requester,
            requester_stopper,
            response_handler,
            response_handler_stopper,
        } = options;

        self.set_up();

        let requester_handle: Option<JoinHandle<_>>;
        let response_handler_handle: Option<JoinHandle<_>>;
        // TODO: Join the term even forwarder thread when we are done.
        let _term_event_forwarder_handle: JoinHandle<_>;
        // NOTE: This code block is used to control the lifetime of the channels.
        {
            let (request_tx, request_rx): (Sender<Request>, Receiver<Request>) =
                channel::unbounded();
            let (response_tx, response_rx): (Sender<Response>, Receiver<Response>) =
                channel::unbounded();
            let (term_event_tx, term_event_rx): (Sender<TermEvent>, Receiver<TermEvent>) =
                channel::unbounded();

            if let Some(starting_term_events) = starting_term_events {
                for term_event in starting_term_events {
                    term_event_tx.send(term_event).unwrap();
                }
            }

            if let Some(mut requester) = requester {
                // Spawn the requester.
                requester_handle = Some(
                    thread::Builder::new()
                        .name("requster".to_string())
                        .spawn(move || requester.run(request_rx))
                        .unwrap(),
                );
            } else {
                requester_handle = None;
            }

            // Spawn the response handler.
            response_handler_handle = match response_handler {
                Some(mut response_handler) => Some(
                    thread::Builder::new()
                        .name("response-handler".to_string())
                        .spawn(move || response_handler.run(response_tx))
                        .unwrap(),
                ),
                None => None,
            };

            // Spawn the terminal event forwarder.
            let mut term_event_forwarder = TermEventForwarder::builder()
                .term_event_tx(term_event_tx)
                .build();
            _term_event_forwarder_handle = thread::Builder::new()
                .name("input-forwarder".to_string())
                .spawn(move || term_event_forwarder.run())
                .unwrap();

            #[cfg(feature = "logging")]
            log::info!("Running.");

            self.size = Size::from(terminal::size().unwrap());

            if let Some(effects) = starting_effects {
                for effect in effects {
                    match effect {
                        SystemEffect::RunProgram { program } => {
                            let size_before = self.size;
                            self.run_program(program, &term_event_rx);
                            if self.size != size_before {
                                // NOTE: We don't handle the effect if one is generated from the resize.
                                let event = Event::TermEvent(TermEvent::Resize(self.size));
                                let _effect: Option<SystemEffect<Request>> = root.handle(event);
                            }
                        }
                        SystemEffect::Request(request) => {
                            request_tx.send(request).unwrap();
                        }
                        SystemEffect::Bell => {
                            self.make_bell_sound();
                        }
                        SystemEffect::Exit => {
                            #[cfg(feature = "logging")]
                            log::info!("Exiting.");
                            self.teardown();
                            return;
                        }
                    }
                }
            }

            loop {
                let fabric: Fabric = root.render(self.size);

                self.renderer.render(fabric);

                let mut event: Event<Response>;
                if let Some(term_event) = self.unused_term_events.pop_front() {
                    event = Event::TermEvent(term_event);
                } else {
                    select! {
                        recv(term_event_rx) -> term_event => {
                            let term_event: TermEvent = match term_event {
                                Ok(term_event) => term_event,
                                #[allow(unused_variables)]
                                Err(error) => {
                                    #[cfg(feature = "logging")]
                                    log::error!("Error receiving terminal event from channel: {}", error);
                                    break;
                                }
                            };
                            if let TermEvent::Resize(size) = term_event {
                                self.size = size;
                            }
                            event = Event::TermEvent(term_event);
                        },
                        recv(response_rx) -> response => {
                            let response: Response = match response {
                                Ok(response) => response,
                                #[allow(unused_variables)]
                                Err(error) => {
                                    #[cfg(feature = "logging")]
                                    log::error!("Error receiving response from channel: {}", error);
                                    break;
                                }
                            };
                            event = Event::Response(response);
                        }
                    }
                }

                let effect: Option<SystemEffect<Request>> = root.handle(event);
                match effect {
                    Some(SystemEffect::RunProgram { program }) => {
                        let size_before = self.size;
                        self.run_program(program, &term_event_rx);
                        if self.size != size_before {
                            // NOTE: We don't handle the effect if one is generated from the resize.
                            event = Event::TermEvent(TermEvent::Resize(self.size));
                            let _effect: Option<SystemEffect<Request>> = root.handle(event);
                        }
                    }
                    Some(SystemEffect::Request(request)) => {
                        request_tx.send(request).unwrap();
                    }
                    Some(SystemEffect::Bell) => {
                        self.make_bell_sound();
                    }
                    Some(SystemEffect::Exit) => {
                        #[cfg(feature = "logging")]
                        log::info!("Exiting.");
                        break;
                    }
                    None => {}
                }
            }
        }

        // Ensure the requester thread joins.
        if let Some(requester_handle) = requester_handle {
            if let Some(mut requester_stopper) = requester_stopper {
                #[cfg(feature = "logging")]
                log::info!("Stopping requester thread...");
                requester_stopper.stop();
                #[cfg(feature = "logging")]
                log::info!("Stopped requester thread.");
            }

            #[cfg(feature = "logging")]
            log::info!("Joining requester thread...");
            requester_handle.join().unwrap();
            #[cfg(feature = "logging")]
            log::info!("Requester thread joined.");
        }

        // Ensure the response handler thread joins.
        if let Some(response_handler_handle) = response_handler_handle {
            if let Some(mut response_handler_stopper) = response_handler_stopper {
                #[cfg(feature = "logging")]
                log::info!("Stopping response handler thread...");
                response_handler_stopper.stop();
                #[cfg(feature = "logging")]
                log::info!("Response handler thread stopped.");
            }

            #[cfg(feature = "logging")]
            log::info!("Joining response handler thread...");
            response_handler_handle.join().unwrap();
            #[cfg(feature = "logging")]
            log::info!("Response handler thread joined.");
        }

        self.teardown();
    }

    fn set_up(&mut self) {
        self.lazy_enable_alternate_terminal();
        self.term.save_attrs().unwrap();
        self.term.enable_raw().unwrap();
        self.lazy_hide_cursor();
        self.lazy_clear_screen();

        self.change_panic_hook();
    }

    fn teardown(&mut self) {
        self.lazy_disable_alternate_terminal();
        self.term.restore_attrs().unwrap();
        self.lazy_show_cursor();
    }

    // NOTE: clippy gets confused by the fork and complains some code is unreachable b/c of it.
    #[allow(unreachable_code)]
    fn run_program(&mut self, program: Box<dyn Program>, term_event_rx: &Receiver<TermEvent>) {
        let program_uuid: Uuid = Uuid::new_v4();

        #[cfg(feature = "logging")]
        log::info!("Running program {}...", program_uuid);

        let setup: ProgramSetup = program.setup();
        self.setup_program(&program_uuid, setup);

        let cleanup: ProgramCleanup = program.cleanup();

        let stdout_pipe: Option<Box<dyn StdoutPipe>> = program.stdout_pipe();

        let filename: OsString = program.filename();
        let mut args: Vec<OsString> = vec![filename.clone()];
        args.extend(program.args());

        // Convert the filename and args to a C strings.
        let filename: CString = CString::new(filename.into_vec()).unwrap();
        let args: Vec<CString> = args
            .into_iter()
            .map(|string| CString::new(string.into_vec()).unwrap())
            .collect();

        #[cfg(feature = "logging")]
        log::debug!("Program has filename {:?} and args {:?}", filename, args);

        // Open a psuedo terminal.
        let window_size: Winsize = Winsize {
            ws_row: self.size.rows.try_into().unwrap(),
            ws_col: self.size.columns.try_into().unwrap(),
            ws_xpixel: 0, // ?
            ws_ypixel: 0, // ?
        };
        let termios = None;

        #[cfg(feature = "logging")]
        log::info!("Forking program...");

        #[allow(unused_assignments)]
        let mut master: RawFd = 0;

        let child: Pid;
        match unsafe { forkpty(&window_size, termios) } {
            Ok(ForkptyResult {
                master: master_,
                fork_result: ForkResult::Parent { child: child_, .. },
            }) => {
                master = master_;
                #[cfg(feature = "logging")]
                log::debug!("Program has a pid of {}.", child_);
                child = child_;
            }
            Ok(ForkptyResult {
                fork_result: ForkResult::Child,
                ..
            }) => {
                // Set the working dir
                if let Some(cwd) = program.cwd() {
                    chdir(&cwd).unwrap();
                }

                for env_var in program.env() {
                    let overwrite = 1;
                    unsafe {
                        setenv(env_var.name.as_ptr(), env_var.value.as_ptr(), overwrite);
                    }
                }

                // Execute the program.
                execvp(&filename, &args).unwrap();
            }
            #[allow(unused_variables)]
            Err(error) => {
                #[cfg(feature = "logging")]
                log::error!("Failed to fork program: {}", error);
                return;
            }
        }

        #[cfg(feature = "logging")]
        log::info!("Program forked.");

        // Spawn a thread that monitors the program.
        let (program_event_tx, program_event_rx): (Sender<ProgramEvent>, Receiver<ProgramEvent>) =
            channel::unbounded();
        let program_monitor = ProgramMonitor::builder()
            .child(child)
            .program_event_tx(program_event_tx)
            .build();
        let program_monitor_handle: JoinHandle<_> = thread::Builder::new()
            .name("program-monitor".to_string())
            .spawn(move || program_monitor.run())
            .unwrap();

        let mut master_stdin: File;
        let mut master_stdout: File;
        unsafe {
            master_stdin = File::from_raw_fd(master);
            // NOTE: We need to duplicate the fd so that we don't double close it with the file is
            // dropped.
            master_stdout = File::from_raw_fd(libc::dup(master));
        }

        // Spawn a thread to handle the stdout of the command.
        let outputer_handle: JoinHandle<_> = match stdout_pipe {
            Some(mut stdout_pipe) => {
                #[cfg(feature = "logging")]
                log::debug!("Spawning program stdout pipe...");
                let handle: JoinHandle<_> = thread::Builder::new()
                    .name("program-stdout-pipe".to_string())
                    .spawn(move || stdout_pipe.run(&mut master_stdout))
                    .unwrap();
                #[cfg(feature = "logging")]
                log::debug!("Spawned program stdout pipe.");

                handle
            }
            None => {
                let mut output_forwarder: OutputForwarder = OutputForwarder::builder()
                    .master_stdout(master_stdout)
                    .build();

                #[cfg(feature = "logging")]
                log::debug!("Spawning program output forwarder...");
                let handle: JoinHandle<_> = thread::Builder::new()
                    .name("program-stdout-pipe".to_string())
                    .spawn(move || output_forwarder.run())
                    .unwrap();
                #[cfg(feature = "logging")]
                log::debug!("Spawned program output forwarder.");

                handle
            }
        };

        loop {
            let event: ProgramLoopEvent = if let Some(term_event) =
                self.unused_term_events.pop_front()
            {
                ProgramLoopEvent::TermEvent(term_event)
            } else {
                select! {
                    recv(term_event_rx) -> term_event => {
                        let term_event: TermEvent = match term_event {
                            Ok(term_event) => term_event,
                            #[allow(unused_variables)]
                            Err(error) => {
                                #[cfg(feature = "logging")]
                                log::warn!("Failed to receive terminal event from channel: {}", error);
                                break;
                            }
                        };
                        ProgramLoopEvent::TermEvent(term_event)
                    }
                    recv(program_event_rx) -> program_event => {
                        let program_event: ProgramEvent = match program_event {
                            Ok(program_event) => program_event,
                            #[allow(unused_variables)]
                            Err(error) => {
                                #[cfg(feature = "logging")]
                                log::warn!("Failed to receive program event from channel: {}", error);
                                break;
                            }
                        };
                        ProgramLoopEvent::ProgramEvent(program_event)
                    }
                }
            };

            match event {
                ProgramLoopEvent::TermEvent(term_event) => match &term_event {
                    TermEvent::KeyEvent(key_event) => {
                        let bytes: Vec<u8> = match TryInto::<Vec<u8>>::try_into(key_event) {
                            Ok(bytes) => bytes,
                            #[allow(unused_variables)]
                            Err(error) => {
                                #[cfg(feature = "logging")]
                                log::warn!("Failed to convert input to bytes: {}", error);
                                continue;
                            }
                        };

                        if let Err(_error) = master_stdin.write(&bytes) {
                            self.unused_term_events.push_back(term_event);
                            break;
                        }
                    }
                    TermEvent::Resize(size) => {
                        self.size = *size;
                        #[cfg(feature = "logging")]
                        log::debug!("Signaling terminal resize to program...");
                        let size: WindowSize = WindowSize {
                            ws_row: size.rows.try_into().unwrap(),
                            ws_col: size.columns.try_into().unwrap(),
                            ws_xpixel: 0,
                            ws_ypixel: 0,
                        };
                        let result: c_int;
                        unsafe {
                            result = ioctl(master, TIOCSWINSZ, &size);
                        }
                        if result == -1 {
                            #[allow(unused_variables)]
                            let error = IOError::last_os_error();
                            #[cfg(feature = "logging")]
                            log::warn!("Failed to signal terminal resize to program: {}", error);
                        } else {
                            #[cfg(feature = "logging")]
                            log::debug!("Signaled terminal resize to program.");
                        };
                    }
                },
                ProgramLoopEvent::ProgramEvent(program_event) => match program_event {
                    ProgramEvent::Done => {
                        #[cfg(feature = "logging")]
                        log::info!("Program finished running.");
                        break;
                    }
                },
            }
        }

        #[cfg(feature = "logging")]
        log::debug!("Waiting for program monitor to stop...");
        program_monitor_handle.join().unwrap();
        #[cfg(feature = "logging")]
        log::debug!("Program monitor stopped.");

        #[cfg(feature = "logging")]
        log::debug!("Waiting for outputer stop...");
        outputer_handle.join().unwrap();
        #[cfg(feature = "logging")]
        log::debug!("Outputer stopped.");

        self.cleanup_program(&program_uuid, cleanup);

        #[cfg(feature = "logging")]
        log::info!("Done running program.");
    }

    /// Run set up for a program.
    #[allow(unused_variables)]
    fn setup_program(&mut self, program_uuid: &Uuid, setup: ProgramSetup) {
        #[cfg(feature = "logging")]
        log::debug!("Setting up program {}...", program_uuid);
        if setup.clear_screen {
            self.lazy_clear_screen();
        }
        if setup.cursor_home {
            self.lazy_move_cursor_home();
        }
        if setup.cursor_visible == Some(true) {
            self.lazy_show_cursor();
        }
        if setup.any() {
            self.update_terminal();
        }
        #[cfg(feature = "logging")]
        log::debug!("Done setting up program {}.", program_uuid);
    }

    /// Run cleanup for a program.
    #[allow(unused_variables)]
    fn cleanup_program(&mut self, program_uuid: &Uuid, cleanup: ProgramCleanup) {
        #[cfg(feature = "logging")]
        log::debug!("Cleaning up program {}...", program_uuid);

        if cleanup.hide_cursor {
            self.lazy_hide_cursor();
        }
        if cleanup.enable_raw_terminal {
            self.term.enable_raw().unwrap();
        }
        if cleanup.any() {
            self.update_terminal();
        }

        #[cfg(feature = "logging")]
        log::debug!("Done cleaning up program {}.", program_uuid);
    }

    fn lazy_enable_alternate_terminal(&mut self) {
        self.stdout.queue(EnterAlternateScreen).unwrap();
    }

    fn lazy_disable_alternate_terminal(&mut self) {
        self.stdout.queue(LeaveAlternateScreen).unwrap();
    }

    fn lazy_clear_screen(&mut self) {
        self.stdout
            .queue(ClearTerminal(TerminalClearType::All))
            .unwrap();
    }

    fn lazy_hide_cursor(&mut self) {
        self.stdout.queue(HideCursor).unwrap();
    }

    fn lazy_show_cursor(&mut self) {
        self.stdout.queue(ShowCursor).unwrap();
    }

    fn lazy_move_cursor_home(&mut self) {
        self.stdout.queue(MoveCursorTo(0, 0)).unwrap();
    }

    fn make_bell_sound(&mut self) {
        self.stdout.execute(Print(ASCII::Bell)).unwrap();
    }

    fn update_terminal(&mut self) {
        self.stdout.flush().unwrap();
    }

    fn change_panic_hook(&mut self) {
        let hook_before = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let mut stdout = io::stdout();
            stdout.queue(LeaveAlternateScreen).unwrap();
            stdout.queue(ShowCursor).unwrap();
            stdout.flush().unwrap();
            terminal::disable_raw_mode().unwrap();
            hook_before(info);
        }));
    }
}

#[derive(TypedBuilder)]
pub struct AppRunOptions<Props, Request, Response>
where
    Request: Send,
    Response: Send,
{
    root: Box<dyn Component<Props, Event<Response>, SystemEffect<Request>>>,

    #[builder(default, setter(into))]
    starting_effects: Option<Vec<SystemEffect<Request>>>,

    #[builder(default, setter(into))]
    starting_term_events: Option<Vec<TermEvent>>,

    /// Makes requests.
    #[builder(default, setter(into))]
    requester: Option<Box<dyn Requester<Request>>>,

    /// Stops the requester.
    #[builder(default, setter(into))]
    requester_stopper: Option<Box<dyn Stopper>>,

    /// Handles responses and sends them to the app.
    #[builder(default, setter(into))]
    response_handler: Option<Box<dyn ResponseHandler<Response>>>,

    /// Stops the responses handler.
    #[builder(default, setter(into))]
    response_handler_stopper: Option<Box<dyn Stopper>>,
}

enum ProgramLoopEvent {
    TermEvent(TermEvent),
    ProgramEvent(ProgramEvent),
}

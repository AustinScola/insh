//! The app.

use std::collections::VecDeque;
use std::ffi::{c_int, CString, OsString};
use std::fs::File;
use std::io::{self, Error as IOError, Stdout, Write};
use std::os::fd::FromRawFd;
use std::os::fd::IntoRawFd;
use std::os::fd::RawFd;
use std::os::unix::ffi::OsStringExt;
use std::panic;
use std::thread::{self, JoinHandle};
use std::time::Duration;

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

use ansi::{BracketedPaste, ControlFunction, EraseInDisplay, Mode};
use rend::{Fabric, Renderer, Size};
use term::{SavedAttrs, Term, TermEvent};

use crossbeam::channel::{self, Receiver, Sender};
use crossbeam::select;
use nix::libc;
use nix::libc::{ioctl, setenv, winsize as WindowSize, TIOCSWINSZ};
use nix::pty::{forkpty, ForkptyResult, Winsize};
use nix::unistd::Pid;
use nix::unistd::{chdir, execvp};
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Runs a component in the terminal.
#[derive(TypedBuilder)]
pub struct App {
    /// The terminal.
    #[builder(setter(skip), default=Term::new())]
    term: Term,
    /// What is written to the terminal.
    #[builder(setter(skip), default=io::stdout())]
    stdout: Stdout,
    /// Draws the component on the screen.
    #[builder(default=Renderer::builder().writer(io::stdout()).build())]
    renderer: Renderer,

    /// The unhandled terminal events.
    #[builder(setter(skip), default)]
    unused_term_events: VecDeque<TermEvent>,

    /// The size of the terminal.
    #[builder(setter(skip), default)]
    size: Size,

    /// How long to wait for the rest of an escape sequence before deciding that the escape key was
    /// pressed on its own.
    #[builder(default=Term::DEFAULT_ESCAPE_TIMEOUT)]
    escape_timeout: Duration,
}

impl App {
    /// The most events to handle before drawing the screen again.
    ///
    /// The events handled between one drawing of the screen and the next have all been superseded
    /// by the ones after them, so there is no reason to stop at any particular number of them. This
    /// is only so that events arriving without a pause — the results of a large search coming in
    /// faster than they can be handled, say — cannot put drawing the screen off for ever.
    const MAX_EVENTS_PER_RENDER: usize = 1024;

    /// Run the app.
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
                        .name("requester".to_string())
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
            let term_event_forwarder = TermEventForwarder::builder()
                .term_event_tx(term_event_tx)
                .escape_timeout(self.escape_timeout)
                .build();
            _term_event_forwarder_handle = thread::Builder::new()
                .name("input-forwarder".to_string())
                .spawn(move || term_event_forwarder.run())
                .unwrap();

            #[cfg(feature = "logging")]
            log::info!("Running.");

            self.size = Term::size().unwrap();

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
                        SystemEffect::Requests(requests) => {
                            for request in requests {
                                request_tx.send(request).unwrap();
                            }
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

            'app: loop {
                let fabric: Fabric = root.render(self.size);

                self.renderer.render(fabric);

                // Wait for something to happen, however long it takes. Nothing is going to change
                // until it does, so there is nothing to draw in the meantime.
                let mut event: Event<Response>;
                if let Some(term_event) = self.unused_term_events.pop_front() {
                    event = Event::TermEvent(self.note_resize(term_event));
                } else {
                    select! {
                        recv(term_event_rx) -> term_event => {
                            let term_event: TermEvent = match term_event {
                                Ok(term_event) => term_event,
                                #[allow(unused_variables)]
                                Err(error) => {
                                    #[cfg(feature = "logging")]
                                    log::error!("Error receiving terminal event from channel: {}", error);
                                    break 'app;
                                }
                            };
                            event = Event::TermEvent(self.note_resize(term_event));
                        },
                        recv(response_rx) -> response => {
                            let response: Response = match response {
                                Ok(response) => response,
                                #[allow(unused_variables)]
                                Err(error) => {
                                    #[cfg(feature = "logging")]
                                    log::error!("Error receiving response from channel: {}", error);
                                    break 'app;
                                }
                            };
                            event = Event::Response(response);
                        }
                    }
                }

                // Handle that and anything else which is already waiting, and only then draw.
                //
                // NOTE: Nothing is waited for here, so the screen is drawn as soon as there is
                // nothing left to handle. The events which are skipped over are only ever ones
                // which have already been superseded: drawing the screen for the first of a burst
                // of key presses when the rest of them are already in hand would be drawing the
                // selection somewhere it has already moved on from.
                let mut handled: usize = 0;
                loop {
                    let effect: Option<SystemEffect<Request>> = root.handle(event);
                    match effect {
                        Some(SystemEffect::RunProgram { program }) => {
                            let size_before = self.size;
                            self.run_program(program, &term_event_rx);
                            if self.size != size_before {
                                // NOTE: We don't handle the effect if one is generated from the resize.
                                let event = Event::TermEvent(TermEvent::Resize(self.size));
                                let _effect: Option<SystemEffect<Request>> = root.handle(event);
                            }

                            // The program drew over the screen, so put it back before handling
                            // anything else.
                            break;
                        }
                        Some(SystemEffect::Request(request)) => {
                            request_tx.send(request).unwrap();
                        }
                        Some(SystemEffect::Requests(requests)) => {
                            for request in requests {
                                request_tx.send(request).unwrap();
                            }
                        }
                        Some(SystemEffect::Bell) => {
                            self.make_bell_sound();
                        }
                        Some(SystemEffect::Exit) => {
                            #[cfg(feature = "logging")]
                            log::info!("Exiting.");
                            break 'app;
                        }
                        None => {}
                    }

                    handled += 1;
                    if handled >= Self::MAX_EVENTS_PER_RENDER {
                        break;
                    }

                    event = match self.pending_event(&term_event_rx, &response_rx) {
                        Some(event) => event,
                        None => {
                            break;
                        }
                    };
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

    /// Return an event which is already waiting to be handled, or `None` if there are none.
    ///
    /// Nothing here waits, so this says what there is to get on with rather than holding out for
    /// more to turn up.
    fn pending_event<Response>(
        &mut self,
        term_event_rx: &Receiver<TermEvent>,
        response_rx: &Receiver<Response>,
    ) -> Option<Event<Response>> {
        if let Some(term_event) = self.unused_term_events.pop_front() {
            return Some(Event::TermEvent(self.note_resize(term_event)));
        }

        if let Ok(term_event) = term_event_rx.try_recv() {
            return Some(Event::TermEvent(self.note_resize(term_event)));
        }

        match response_rx.try_recv() {
            Ok(response) => Some(Event::Response(response)),
            Err(_) => None,
        }
    }

    /// Take note of the size of the terminal if the event is that it was resized, and return the
    /// event either way.
    fn note_resize(&mut self, term_event: TermEvent) -> TermEvent {
        if let TermEvent::Resize(size) = term_event {
            self.size = size;

            // NOTE: What a terminal does with what it is showing when it is resized — how it wraps
            // the rows again, which of them it throws away, where it leaves the cursor — is not
            // something which can be worked out, so none of it is taken for granted afterwards.
            // Several resizes can be handled between one drawing of the screen and the next, so it
            // is not enough for the renderer to notice that the size it is drawing has changed:
            // dragging the edge of a window out and back lands on the size it started at.
            self.renderer.forget();
        }
        term_event
    }

    /// Set up the terminal.
    fn set_up(&mut self) {
        self.lazy_enable_alternate_terminal();
        self.term.save_attrs().unwrap();
        self.term.enable_raw().unwrap();
        self.lazy_enable_bracketed_paste();
        self.lazy_hide_cursor();

        // NOTE: The colors are put back before the screen is cleared rather than after, because a
        // terminal which erases in the color it is writing on would otherwise clear it to whatever
        // color it was left in.
        self.renderer.reset();
        self.lazy_clear_screen();

        self.change_panic_hook();
    }

    /// Teardown the terminal.
    fn teardown(&mut self) {
        // NOTE: The colors are put back before the shell has the terminal again, or it would carry
        // on writing in whichever ones the last frame left it in.
        self.renderer.reset();

        self.lazy_disable_bracketed_paste();
        self.lazy_disable_alternate_terminal();
        self.term.restore_attrs().unwrap();
        self.lazy_show_cursor();
    }

    /// Run a program.
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

        // Open a pseudo terminal.
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
            Ok(ForkptyResult::Parent {
                child: child_,
                master: master_,
            }) => {
                master = master_.into_raw_fd();
                #[cfg(feature = "logging")]
                log::debug!("Program has a pid of {}.", child_);
                child = child_;
            }
            Ok(ForkptyResult::Child) => {
                // Set the working dir.
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
                ProgramLoopEvent::TermEvent(term_event) => {
                    let bytes: Option<Vec<u8>> = match &term_event {
                        TermEvent::KeyEvent(key_event) => Some(Vec::from(key_event)),
                        TermEvent::Paste(text) => {
                            // NOTE: The terminal only wraps pasted text when the program which is
                            // running has asked it to, so the markers go back on for it to find.
                            let mut bytes: Vec<u8> = BracketedPaste::START.to_vec();
                            bytes.extend_from_slice(text.as_bytes());
                            bytes.extend_from_slice(BracketedPaste::END);
                            Some(bytes)
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
                                log::warn!(
                                    "Failed to signal terminal resize to program: {}",
                                    error
                                );
                            } else {
                                #[cfg(feature = "logging")]
                                log::debug!("Signaled terminal resize to program.");
                            };

                            None
                        }
                    };

                    if let Some(bytes) = bytes {
                        if let Err(_error) = master_stdin.write_all(&bytes) {
                            self.unused_term_events.push_back(term_event);
                            break;
                        }
                    }
                }
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

        // NOTE: The program takes the terminal over while it runs, including saying for itself
        // whether pasted text is wrapped, so stop asking for it on its behalf.
        self.lazy_disable_bracketed_paste();

        // NOTE: A frame leaves the terminal writing in the colors the last thing it drew was
        // written in, so they are put back before the program has the terminal. A terminal which
        // erases in the color it is writing on would otherwise clear the screen to whichever color
        // the footer happened to be written on.
        self.renderer.reset();

        if setup.clear_screen {
            self.lazy_clear_screen();
        }
        if setup.cursor_home {
            self.lazy_move_cursor_home();
        }
        if setup.cursor_visible == Some(true) {
            self.lazy_show_cursor();
        }
        self.update_terminal();
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

        // The program will have stopped the terminal from wrapping pasted text on its way out.
        self.lazy_enable_bracketed_paste();

        self.update_terminal();

        // NOTE: The program had the terminal to itself and left it however it liked, in whichever
        // colors and showing whatever it drew, so what it is writing in is put back and none of
        // what is on the screen is taken for granted when the next frame is drawn.
        self.renderer.reset();

        #[cfg(feature = "logging")]
        log::debug!("Done cleaning up program {}.", program_uuid);
    }

    /// Queue switching to the second screen.
    fn lazy_enable_alternate_terminal(&mut self) {
        self.lazy_control_function(&Self::alternate_terminal(true));
    }

    /// Queue switching back to the first screen.
    fn lazy_disable_alternate_terminal(&mut self) {
        self.lazy_control_function(&Self::alternate_terminal(false));
    }

    /// Queue clearing the screen.
    fn lazy_clear_screen(&mut self) {
        self.lazy_control_function(&ControlFunction::EraseInDisplay(EraseInDisplay::All));
    }

    /// Queue hiding the cursor.
    fn lazy_hide_cursor(&mut self) {
        self.lazy_control_function(&Self::cursor_visible(false));
    }

    /// Queue showing the cursor.
    fn lazy_show_cursor(&mut self) {
        self.lazy_control_function(&Self::cursor_visible(true));
    }

    /// Queue asking the terminal to wrap pasted text.
    fn lazy_enable_bracketed_paste(&mut self) {
        self.lazy_control_function(&Self::bracketed_paste(true));
    }

    /// Queue asking the terminal to stop wrapping pasted text.
    fn lazy_disable_bracketed_paste(&mut self) {
        self.lazy_control_function(&Self::bracketed_paste(false));
    }

    /// Queue moving the cursor to the top left of the screen.
    fn lazy_move_cursor_home(&mut self) {
        self.lazy_control_function(&ControlFunction::CursorPosition { row: 1, column: 1 });
    }

    /// Return the control function which switches to the second screen and back, remembering where
    /// the cursor is on the way in and out.
    fn alternate_terminal(set: bool) -> ControlFunction {
        ControlFunction::SetMode {
            modes: vec![Mode::AlternateScreenAndSaveCursor],
            set,
        }
    }

    /// Return the control function which shows and hides the cursor.
    fn cursor_visible(set: bool) -> ControlFunction {
        ControlFunction::SetMode {
            modes: vec![Mode::CursorVisible],
            set,
        }
    }

    /// Return the control function for wrapping pasted text.
    fn bracketed_paste(set: bool) -> ControlFunction {
        ControlFunction::SetMode {
            modes: vec![Mode::BracketedPaste],
            set,
        }
    }

    /// Queue the escape code for the control function, but don't send it.
    fn lazy_control_function(&mut self, function: &ControlFunction) {
        self.stdout.write_all(&Vec::from(function)).unwrap();
    }

    /// Ring the bell.
    fn make_bell_sound(&mut self) {
        self.stdout.write_all(&[ASCII::Bell as u8]).unwrap();
        self.update_terminal();
    }

    /// Send everything which is queued to the terminal.
    fn update_terminal(&mut self) {
        self.stdout.flush().unwrap();
    }

    /// Change the panic hook so that the terminal is put back first.
    fn change_panic_hook(&mut self) {
        let hook_before = panic::take_hook();

        // NOTE: The attributes are taken a copy of because the hook has to be able to put the
        // terminal back without the app, which is not reachable by the time it runs.
        let saved_attrs: Option<SavedAttrs> = self.term.saved_attrs();

        panic::set_hook(Box::new(move |info| {
            let mut stdout = io::stdout();
            stdout
                .write_all(&Vec::from(&Self::alternate_terminal(false)))
                .unwrap();
            stdout
                .write_all(&Vec::from(&Self::cursor_visible(true)))
                .unwrap();
            stdout.flush().unwrap();

            if let Some(saved_attrs) = saved_attrs {
                saved_attrs.restore().unwrap();
            }

            hook_before(info);
        }));
    }
}

/// The options for running an app.
#[derive(TypedBuilder)]
pub struct AppRunOptions<Props, Request, Response>
where
    Request: Send,
    Response: Send,
{
    /// The root component.
    root: Box<dyn Component<Props, Event<Response>, SystemEffect<Request>>>,

    /// The starting effects.
    #[builder(default, setter(into))]
    starting_effects: Option<Vec<SystemEffect<Request>>>,

    /// The starting terminal events.
    #[builder(default, setter(into))]
    starting_term_events: Option<Vec<TermEvent>>,

    /// Makes requests.
    #[builder(default, setter(into))]
    requester: Option<Box<dyn Requester<Request>>>,

    /// Stops the requester.
    #[builder(default, setter(into))]
    requester_stopper: Option<Box<dyn Stopper>>,

    /// Handles responses.
    #[builder(default, setter(into))]
    response_handler: Option<Box<dyn ResponseHandler<Response>>>,

    /// Stops the response handler.
    #[builder(default, setter(into))]
    response_handler_stopper: Option<Box<dyn Stopper>>,
}

/// A program loop event.
enum ProgramLoopEvent {
    /// A terminal event.
    TermEvent(TermEvent),
    /// A program event.
    ProgramEvent(ProgramEvent),
}

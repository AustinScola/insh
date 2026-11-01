//! Contains the [`Session`] component, which shows the panes and windows of a session.

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use crate::config::Config;

    use rend::Size;

    use typed_builder::TypedBuilder;
    use uuid::Uuid;

    /// The properties of a session.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// The request whose responses say what is happening to the session.
        pub request: Uuid,
        /// How big the terminal the client is in is.
        pub size: Size,
        /// The directory which the browser in a pane shows.
        pub dir: PathBuf,
        /// The configuration.
        pub config: Config,
    }
}
pub use props::Props;

/// Contains the [`Event`] enum.
mod event {
    use insh_api::Response;
    use term::TermEvent;

    /// An event which a session handles.
    pub enum Event {
        /// Something happened to the terminal.
        TermEvent(TermEvent),
        /// The daemon said something.
        Response(Response),
    }
}
pub use event::Event;

/// Contains the [`Effect`] enum.
mod effect {
    use insh_api::Request;

    /// Something which a session asks for.
    pub enum Effect {
        /// Ask the daemon for something.
        Request(Request),
        /// Ask the daemon for more than one thing, in the order they are in.
        Requests(Vec<Request>),
        /// Make the bell sound.
        Bell,
        /// Stop showing the session.
        Quit,
    }
}
pub use effect::Effect;

/// Contains the [`Toward`] enum.
mod toward {
    /// Which way along the screen to go from one pane to another.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Toward {
        /// To the pane on the left.
        Left,
        /// To the pane below.
        Down,
        /// To the pane above.
        Up,
        /// To the pane on the right.
        Right,
    }
}
use toward::Toward;

/// Contains the [`Action`] enum.
mod action {
    use super::super::prompt::Command;

    use term::KeyEvent;

    /// What a key the multiplexer takes means.
    #[derive(Clone)]
    pub enum Action {
        /// Do one of the commands of the multiplexer.
        ///
        /// NOTE: Every key does one of those rather than something of its own, so that whatever
        /// can be done with a key can be typed at the prompt as well.
        Run(Command),
        /// Send the keys to the pane what is typed goes to.
        Send(Vec<KeyEvent>),
    }
}
use action::Action;

/// Contains the [`Session`] component.
mod session {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::super::borders::Borders;
    use super::super::copying::{Copying, Moving};
    use super::super::pane::{Effect as PaneEffect, Event as PaneEvent, Pane, Props as PaneProps};
    use super::super::prompt::{Asking, Command, Effect as PromptEffect, Mode, Prompt};
    use super::super::tree::{Effect as TreeEffect, Tree};
    use super::{Action, Effect, Event, Props, Toward};
    use crate::clipboard::Clipboard;
    use crate::color::Color;
    use crate::components::common::{Footer, FooterInfo, FooterProps};
    use crate::config::Config;

    use insh_api::{
        AttachRequestParams, DragPaneRequestParams, KillPaneRequestParams,
        ListSessionsRequestParams, NewSessionRequestParams, NewWindowRequestParams, PaneLayout,
        RenameSessionRequestParams, RenameWindowRequestParams, Request, RequestParams,
        ResizeClientRequestParams, Response, ResponseParams, RunCommandRequestParams,
        RunInPaneRequestParams, ScrollPaneRequestParams, SelectPaneRequestParams,
        SelectWindowRequestParams, Split, SplitPaneRequestParams, SwapPaneRequestParams,
        SyncPanesRequestParams, WritePaneRequestParams, ZoomPaneRequestParams,
    };
    use rend::{Fabric, Location, Size};
    use term::{Key, KeyEvent, KeyMods, MouseButton, MouseEvent, MouseEventKind, TermEvent};
    use terminal_emulator::Modes;
    use til::{CommandParser, Component, KeyPattern, Parsed};

    use uuid::Uuid;

    /// A line between two panes which is being dragged.
    pub struct HeldLine {
        /// The panes the line is on the far side of, which is more than one when it was taken hold
        /// of where lines meet end to end.
        panes: Vec<Uuid>,
        /// Which row or column the line was last moved to.
        at: usize,
    }

    /// The lines between panes which are being dragged, which is both of them when they were taken
    /// hold of where they meet.
    pub struct Dragging {
        /// The line down the screen, which has a pane on its left.
        beside: Option<HeldLine>,
        /// The line across the screen, which has a pane above it.
        below: Option<HeldLine>,
    }

    /// Shows the panes and windows of a session.
    ///
    /// The daemon owns all of it, so nothing here decides anything: it draws what it is sent, and
    /// sends on what is typed. The one thing it does decide is what the keys after the prefix
    /// mean, since those are for the multiplexer rather than for a program.
    pub struct Session {
        /// The state of the session.
        state: State,
    }

    impl Component<Props, Event, Effect> for Session {
        fn new(props: Props) -> Self {
            Self {
                state: State::from(props),
            }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            match event {
                Event::TermEvent(term_event) => self.handle_term_event(term_event),
                Event::Response(response) => self.handle_response(response),
            }
        }

        fn render(&self, size: Size) -> Fabric {
            self.state.render(size)
        }
    }

    impl Session {
        /// Do what the terminal event says.
        fn handle_term_event(&mut self, term_event: TermEvent) -> Option<Effect> {
            // NOTE: The list of sessions and the prompt are drawn over the panes and take what is
            // typed while they are up, so nothing reaches a pane until they have been closed.
            if self.state.tree.is_some() {
                return self.handle_tree_event(term_event);
            }
            if self.state.prompt.is_some() {
                return self.handle_prompt_event(term_event);
            }
            if self.state.copying.is_some() {
                return self.handle_copying(term_event);
            }

            match term_event {
                TermEvent::KeyEvent(key_event) => self.handle_key_event(key_event),
                TermEvent::Mouse(mouse_event) => self.handle_mouse_event(mouse_event),
                TermEvent::Paste(text) => {
                    let bytes: Vec<u8> = self.state.pasted(&text);
                    self.state.write(bytes).map(Effect::Request)
                }
                TermEvent::Resize(size) => {
                    self.state.size = size;
                    Some(Effect::Request(
                        Request::builder()
                            .params(RequestParams::ResizeClient(
                                ResizeClientRequestParams::builder().size(size).build(),
                            ))
                            .build(),
                    ))
                }
            }
        }

        /// Do what the key says, which is either one of the commands of the multiplexer or
        /// something to send on to the program in the pane.
        fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<Effect> {
            let action: Action = match self.state.parse(key_event) {
                // The keys so far begin a command, so nothing is sent on until it is known
                // whether they finish one.
                Parsed::Pending => return None,
                Parsed::Command(action) => action,
                Parsed::Unknown(keys) => Action::Send(keys),
            };

            self.state.perform(action)
        }

        /// Do what the list of sessions says, which is either showing one of them or closing.
        fn handle_tree_event(&mut self, term_event: TermEvent) -> Option<Effect> {
            // Changing size still goes to the daemon, since the panes behind the list are drawn
            // for whatever size the client is.
            if let TermEvent::Resize(size) = term_event {
                return Some(Effect::Request(
                    Request::builder()
                        .params(RequestParams::ResizeClient(
                            ResizeClientRequestParams::builder().size(size).build(),
                        ))
                        .build(),
                ));
            }

            let tree: &mut Tree = self.state.tree.as_mut()?;

            match tree.handle(term_event)? {
                TreeEffect::Close => {
                    self.state.tree = None;
                    None
                }
                TreeEffect::Show {
                    session,
                    window,
                    pane,
                } => {
                    self.state.tree = None;

                    // NOTE: Showing a window of the session which is already being shown is only
                    // a matter of looking at it. One of another session means showing that
                    // session first, and the window is looked at once it is.
                    let showing: bool = self
                        .state
                        .layout
                        .as_ref()
                        .is_some_and(|layout| *layout.session() == session);

                    let mut asked: Vec<Request> = vec![Request::builder()
                        .params(RequestParams::SelectWindow(
                            SelectWindowRequestParams::builder().window(window).build(),
                        ))
                        .build()];

                    // A pane of that window was picked rather than the window as a whole, so what
                    // is typed goes to it once the window is being looked at.
                    if let Some(pane) = pane {
                        asked.push(
                            Request::builder()
                                .params(RequestParams::SelectPane(
                                    SelectPaneRequestParams::builder().pane(pane).build(),
                                ))
                                .build(),
                        );
                    }

                    if showing {
                        return Some(Effect::Requests(asked));
                    }

                    let attach = Request::builder()
                        .params(RequestParams::Attach(
                            AttachRequestParams::builder()
                                .session(Some(session))
                                .size(self.state.size)
                                .build(),
                        ))
                        .build();

                    // NOTE: The daemon tells a client about the session it is showing on the
                    // request it attached with, so showing another session means being told about
                    // it on another request. Taking note of which one is what keeps the responses
                    // from being taken for a pane's.
                    self.state.request = *attach.uuid();

                    let mut requests: Vec<Request> = vec![attach];
                    requests.extend(asked);
                    Some(Effect::Requests(requests))
                }
            }
        }

        /// Move about what has scrolled off the top of a pane, which is what is done while it is
        /// being looked back through rather than what is typed going to the program.
        fn handle_copying(&mut self, term_event: TermEvent) -> Option<Effect> {
            let key_event: KeyEvent = match term_event {
                // NOTE: The keys are offered to the parser first, so that the commands of the
                // multiplexer work while looking back the way they do in tmux. Without that the
                // only way out is the one key which takes it, and anyone who reaches for the
                // prefix instead finds that nothing at all answers.
                TermEvent::KeyEvent(key_event) => match self.state.parse(key_event) {
                    Parsed::Pending => return None,
                    Parsed::Command(action) => return self.state.stop_copying(action),
                    Parsed::Unknown(mut keys) => keys.drain(..).next()?,
                },
                TermEvent::Mouse(_) => return None,
                TermEvent::Resize(size) => {
                    self.state.size = size;
                    return Some(Effect::Request(
                        Request::builder()
                            .params(RequestParams::ResizeClient(
                                ResizeClientRequestParams::builder().size(size).build(),
                            ))
                            .build(),
                    ));
                }
                TermEvent::Paste(_) => return None,
            };

            if key_event.key != Key::Char('g') {
                self.state.copying.as_mut()?.other_key();
            }

            // Going about the pane and taking from it, which is the rest of what the keys mean.
            let moving: Moving = match key_event.key {
                Key::Char('q') => {
                    self.state.leave_copying();
                    // Looking back no further is what puts the pane back to what is on it now.
                    return self.state.look_back(isize::MIN);
                }
                // NOTE: Giving up on what is being taken is not giving up on looking back through
                // the pane, which is how tmux has it with vi keys: escape clears the selection and
                // q is what leaves.
                Key::Escape => {
                    self.state.copying.as_mut()?.unmark();
                    return None;
                }
                Key::Char(' ' | 'v') => {
                    self.state.copying.as_mut()?.mark();
                    return None;
                }
                Key::CarriageReturn | Key::Char('y') => return self.state.copy(),
                Key::Char('h') | Key::Left => Moving::Left,
                Key::Char('j') | Key::Down => Moving::Down,
                Key::Char('k') | Key::Up => Moving::Up,
                Key::Char('l') | Key::Right => Moving::Right,
                Key::Char('0') | Key::Home => Moving::Start,
                Key::Char('$') | Key::End => Moving::End,
                // NOTE: These move the pane itself rather than where in it is being looked at,
                // since there is no going the whole way in a pane which is one screen big.
                Key::Char('g') => {
                    return match self.state.copying.as_mut()?.go_back() {
                        true => self.state.look_back(isize::MAX),
                        // The first of the two was pressed, so nothing moves until the other does.
                        false => None,
                    };
                }
                Key::Char('G') => return self.state.look_back(isize::MIN),
                _ => return None,
            };

            // Going past the top or the bottom of the pane is what looks further back through what
            // has scrolled off it, since there is nowhere left in the pane to go that way.
            let rows: isize = self.state.copying.as_mut()?.move_toward(moving);
            match rows {
                0 => None,
                rows => self.state.look_back(rows),
            }
        }

        /// Do what was typed at the prompt.
        fn handle_prompt_event(&mut self, term_event: TermEvent) -> Option<Effect> {
            if let TermEvent::Resize(size) = term_event {
                self.state.size = size;
                return Some(Effect::Request(
                    Request::builder()
                        .params(RequestParams::ResizeClient(
                            ResizeClientRequestParams::builder().size(size).build(),
                        ))
                        .build(),
                ));
            }

            let prompt: &mut Prompt = self.state.prompt.as_mut()?;

            match prompt.handle(term_event)? {
                PromptEffect::Request(request) => Some(Effect::Request(request)),
                PromptEffect::Close => {
                    self.state.prompt = None;
                    None
                }
                PromptEffect::Run { command, typed } => {
                    self.state.prompt = None;

                    // NOTE: The daemon is told what was typed so that it is suggested next time,
                    // the same way it is told which directories have been gone to. It is only told
                    // about what was typed at the prompt and not about what a key did, since a key
                    // is not something there is any completing.
                    let remember = Request::builder()
                        .params(RequestParams::RunCommand(
                            RunCommandRequestParams::builder().command(typed).build(),
                        ))
                        .build();

                    let done: Option<Effect> = self.state.run(command);
                    State::after(Some(Effect::Request(remember)), done)
                }
                PromptEffect::Name { what, name } => {
                    self.state.prompt = None;
                    self.state.name(what, name)
                }
            }
        }

        /// Do what was done with the mouse.
        ///
        /// Pressing in a pane sends what is typed there, and pressing on one of the lines between
        /// two panes takes hold of it so that moving the mouse drags it. What is done with the
        /// mouse otherwise is left alone, since a program in a pane which has asked for the
        /// reports itself should be getting them, and passing them on is something to do once a
        /// pane can say it wants them.
        fn handle_mouse_event(&mut self, mouse_event: MouseEvent) -> Option<Effect> {
            let (row, column) = (mouse_event.row, mouse_event.column);

            match mouse_event.kind {
                MouseEventKind::Press(MouseButton::Left) => {
                    if let Some(dragging) = self.state.take_hold(row, column) {
                        self.state.dragging = Some(dragging);
                        return None;
                    }

                    let pane: Uuid = self.state.pane_at(row, column)?;
                    Some(Effect::Request(
                        Request::builder()
                            .params(RequestParams::SelectPane(
                                SelectPaneRequestParams::builder().pane(pane).build(),
                            ))
                            .build(),
                    ))
                }
                MouseEventKind::Drag(MouseButton::Left) => self.state.dragged(row, column),
                MouseEventKind::Release(_) => {
                    self.state.dragging = None;
                    None
                }
                _ => None,
            }
        }

        /// Take note of what the daemon said.
        fn handle_response(&mut self, response: Response) -> Option<Effect> {
            // NOTE: The sessions are answered on the request which asked for them rather than on
            // the one the session is reported on, so they are taken before the rest is sorted out
            // by which request it is for.
            if let ResponseParams::Sessions(params) = response.params() {
                if let Some(tree) = self.state.tree.as_mut() {
                    tree.show(params.sessions());
                }
                return None;
            }

            // NOTE: The same goes for what the daemon suggests the rest of what is being typed at
            // the prompt is, which is asked for on a request of the prompt's own.
            if let ResponseParams::SuggestCommand(_) = response.params() {
                if let Some(prompt) = self.state.prompt.as_mut() {
                    prompt.handle_response(&response);
                }
                return None;
            }

            if response.uuid() != self.state.request() {
                // NOTE: A pane which shows one of insh's own components asks the daemon for things
                // of its own, and the answers come back on those requests rather than on the one
                // the session is reported on. Every pane is given it, and the ones which did not
                // ask ignore it, which is how a component tells its own answers from anyone else's.
                return self.state.tell_panes(response);
            }

            match response.params() {
                ResponseParams::Layout(params) => {
                    let asked: Vec<Request> = self.state.relayout(params.layout().clone());
                    for screen in params.screens() {
                        self.state.apply(screen);
                    }

                    match asked.is_empty() {
                        true => None,
                        false => Some(Effect::Requests(asked)),
                    }
                }
                ResponseParams::PaneScreen(params) => {
                    let bells: usize = params.screen().bells();
                    self.state.apply(params.screen());
                    match bells > 0 && self.state.bell() {
                        true => Some(Effect::Bell),
                        false => None,
                    }
                }
                // NOTE: Nothing is done about a pane finishing beyond waiting for the layout which
                // follows it, since that is what says which panes are left.
                ResponseParams::PaneExited(_) => None,
                ResponseParams::Detached(_) => Some(Effect::Quit),
                _ => None,
            }
        }

        /// Return the request which starts showing a session on a terminal of the given size.
        pub fn attach(size: Size) -> Request {
            Request::builder()
                .params(RequestParams::Attach(
                    AttachRequestParams::builder().size(size).build(),
                ))
                .build()
        }
    }

    /// The state of a session.
    pub struct State {
        /// The request whose responses say what is happening to the session.
        request: Uuid,
        /// The directory which the browser in a new pane shows.
        dir: PathBuf,
        /// The configuration.
        config: Config,
        /// Where everything is, which is nothing at all until the daemon has said.
        layout: Option<insh_api::Layout>,
        /// The panes of the window being looked at.
        panes: HashMap<Uuid, Pane>,
        /// Turns the keys pressed after the prefix into the commands of the multiplexer.
        command_parser: CommandParser<Action>,
        /// Turns the keys pressed into the commands which are done again without the prefix.
        repeat_parser: CommandParser<Action>,
        /// Whether the last thing done was one of the commands which is done again without the
        /// prefix.
        repeating: bool,
        /// The line which is being dragged, if one is.
        dragging: Option<Dragging>,
        /// The list of the sessions and the windows in them, while it is being shown.
        tree: Option<Tree>,
        /// Where a command for the multiplexer is typed, while it is being typed.
        prompt: Option<Prompt>,
        /// Whether what has scrolled off the top of a pane is being looked back through.
        copying: Option<Copying>,
        /// Whether the bell is sounded.
        bell: bool,
        /// How big the terminal the client is in is.
        size: Size,
        /// What the machine is called, which the footer shows.
        hostname: String,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                request: props.request,
                dir: props.dir,
                config: props.config.clone(),
                layout: None,
                panes: HashMap::new(),
                command_parser: Self::command_parser(),
                repeat_parser: Self::repeat_parser(),
                repeating: false,
                dragging: None,
                tree: None,
                prompt: None,
                copying: None,
                bell: props.config.general().bell(),
                // NOTE: How big the client is has to be known from the start rather than waited
                // for, because nothing says so until the terminal changes size and showing
                // another session in the meantime would ask for one of no size at all.
                size: props.size,
                hostname: Self::hostname(),
            }
        }
    }

    impl State {
        /// The key which the commands of the multiplexer are pressed after.
        ///
        /// Everything which is not one of them is sent on to the program in the pane, so this is
        /// what keeps the two of them apart.
        const PREFIX: Key = Key::Char('b');

        /// Return the parser for the commands which are done again without the prefix, which are
        /// the ones which move a line between two panes.
        fn repeat_parser() -> CommandParser<Action> {
            CommandParser::new()
                .bind(
                    [KeyPattern::exact(Key::Right, KeyMods::CONTROL)],
                    Action::Run(Command::ResizePaneRight),
                )
                .bind(
                    [KeyPattern::exact(Key::Left, KeyMods::CONTROL)],
                    Action::Run(Command::ResizePaneLeft),
                )
                .bind(
                    [KeyPattern::exact(Key::Down, KeyMods::CONTROL)],
                    Action::Run(Command::ResizePaneDown),
                )
                .bind(
                    [KeyPattern::exact(Key::Up, KeyMods::CONTROL)],
                    Action::Run(Command::ResizePaneUp),
                )
        }

        /// Return the parser for the commands of the multiplexer.
        fn command_parser() -> CommandParser<Action> {
            let prefix = || KeyPattern::exact(Self::PREFIX, KeyMods::CONTROL);
            // NOTE: The keys are matched whatever is held with them, because a character like
            // `%` is typed with shift and would otherwise never match.
            let then = KeyPattern::any;
            let run = Action::Run;

            CommandParser::new()
                .bind(
                    [prefix(), then(Key::Char('%'))],
                    run(Command::SplitVertically),
                )
                .bind(
                    [prefix(), then(Key::Char('"'))],
                    run(Command::SplitHorizontally),
                )
                .bind([prefix(), then(Key::Char('x'))], run(Command::KillPane))
                // NOTE: An arrow on its own goes to the pane that way and one held with control
                // moves the line on that side of it, which is how tmux has them. The letters do
                // the same as the arrows, for whoever would rather not reach for them.
                .bind([prefix(), then(Key::Char('h'))], run(Command::FocusLeft))
                .bind(
                    [prefix(), KeyPattern::exact(Key::Left, KeyMods::NONE)],
                    run(Command::FocusLeft),
                )
                .bind([prefix(), then(Key::Char('j'))], run(Command::FocusDown))
                .bind(
                    [prefix(), KeyPattern::exact(Key::Down, KeyMods::NONE)],
                    run(Command::FocusDown),
                )
                .bind([prefix(), then(Key::Char('k'))], run(Command::FocusUp))
                .bind(
                    [prefix(), KeyPattern::exact(Key::Up, KeyMods::NONE)],
                    run(Command::FocusUp),
                )
                .bind([prefix(), then(Key::Char('l'))], run(Command::FocusRight))
                .bind(
                    [prefix(), KeyPattern::exact(Key::Right, KeyMods::NONE)],
                    run(Command::FocusRight),
                )
                .bind(
                    [prefix(), KeyPattern::exact(Key::Left, KeyMods::CONTROL)],
                    run(Command::ResizePaneLeft),
                )
                .bind(
                    [prefix(), KeyPattern::exact(Key::Right, KeyMods::CONTROL)],
                    run(Command::ResizePaneRight),
                )
                .bind(
                    [prefix(), KeyPattern::exact(Key::Up, KeyMods::CONTROL)],
                    run(Command::ResizePaneUp),
                )
                .bind(
                    [prefix(), KeyPattern::exact(Key::Down, KeyMods::CONTROL)],
                    run(Command::ResizePaneDown),
                )
                .bind(
                    [prefix(), then(Key::Char('y'))],
                    run(Command::SynchronizePanes),
                )
                .bind([prefix(), then(Key::Char('z'))], run(Command::ToggleZoom))
                // The braces are what tmux moves a pane about with.
                .bind(
                    [prefix(), then(Key::Char('{'))],
                    run(Command::SwitchPanePrevious),
                )
                .bind(
                    [prefix(), then(Key::Char('}'))],
                    run(Command::SwitchPaneNext),
                )
                .bind(
                    [prefix(), then(Key::Char('$'))],
                    run(Command::RenameSession { name: None }),
                )
                .bind(
                    [prefix(), then(Key::Char(','))],
                    run(Command::RenameWindow { name: None }),
                )
                .bind(
                    [prefix(), then(Key::Char('c'))],
                    run(Command::NewWindow { name: None }),
                )
                .bind([prefix(), then(Key::Char('s'))], run(Command::ListSessions))
                .bind(
                    [prefix(), then(Key::Char('['))],
                    run(Command::ChangeMode { mode: Mode::Copy }),
                )
                .bind(
                    [prefix(), then(Key::Char(':'))],
                    run(Command::ChangeMode { mode: Mode::Prompt }),
                )
        }

        /// Return what the machine is called, which is nothing at all when it will not say.
        fn hostname() -> String {
            return nix::unistd::gethostname()
                .ok()
                .and_then(|name| name.into_string().ok())
                .unwrap_or_default();
        }

        /// Return the time of day, which is what the footer shows at the far right.
        fn now() -> String {
            return chrono::Local::now().format("%H:%M %d-%b-%y").to_string();
        }

        /// Return the request whose responses say what is happening to the session.
        pub fn request(&self) -> &Uuid {
            &self.request
        }

        /// Return whether the bell is sounded.
        pub fn bell(&self) -> bool {
            self.bell
        }

        /// Return what the keys pressed so far mean.
        ///
        /// Once a line between two panes has been moved, an arrow held with control goes on
        /// moving it without the prefix for as long as nothing else is typed. That is what lets a
        /// line be moved as far as control is held down, rather than only as far as it is moved
        /// before some length of time has gone by.
        pub fn parse(&mut self, key_event: KeyEvent) -> Parsed<Action> {
            if self.repeating {
                if let Parsed::Command(action) = self.repeat_parser.parse(key_event.clone()) {
                    return Parsed::Command(action);
                }

                // Anything else is the end of moving the line, and is taken as what it is.
                self.repeating = false;
            }

            let parsed: Parsed<Action> = self.command_parser.parse(key_event);

            // NOTE: Whether another arrow can follow without the prefix is only worked out once
            // the keys have finished a command, so that the keys which begin one do not stop the
            // one before it from being done again.
            if let Parsed::Command(action) = &parsed {
                self.repeating = matches!(
                    action,
                    Action::Run(
                        Command::ResizePaneDown
                            | Command::ResizePaneUp
                            | Command::ResizePaneLeft
                            | Command::ResizePaneRight
                    )
                );
            }

            parsed
        }

        /// Return which pane what is typed goes to.
        fn active(&self) -> Option<Uuid> {
            self.layout.as_ref().map(|layout| *layout.pane())
        }

        /// Do what the action says.
        pub fn perform(&mut self, action: Action) -> Option<Effect> {
            return match action {
                Action::Run(command) => self.run(command),
                Action::Send(keys) => self.send(keys),
            };
        }

        /// Return what the window being looked at is called.
        fn window_name(&self) -> Option<String> {
            let layout = self.layout.as_ref()?;

            return layout
                .windows()
                .iter()
                .find(|window| window.window() == layout.window())
                .map(|window| window.name().to_string());
        }

        /// Call the session or the window being looked at what was typed.
        pub fn name(&mut self, what: Asking, name: String) -> Option<Effect> {
            let params: RequestParams = match what {
                Asking::WindowName => RequestParams::RenameWindow(
                    RenameWindowRequestParams::builder().name(name).build(),
                ),
                _ => RequestParams::RenameSession(
                    RenameSessionRequestParams::builder().name(name).build(),
                ),
            };

            Some(Effect::Request(Request::builder().params(params).build()))
        }
        /// Do the command, which is either one which was typed at the prompt or one which a key
        /// the multiplexer takes does.
        pub fn run(&mut self, command: Command) -> Option<Effect> {
            // NOTE: Which pane what is typed goes to is only asked for by the commands which are
            // about a pane. Listing the sessions, making one, and detaching are none of them, and
            // asking first would be giving up on those whenever there is no pane to be had.
            let params: RequestParams = match command {
                Command::SplitVertically => RequestParams::SplitPane(
                    SplitPaneRequestParams::builder()
                        .pane(self.active()?)
                        .split(Split::Beside)
                        .build(),
                ),
                Command::SplitHorizontally => RequestParams::SplitPane(
                    SplitPaneRequestParams::builder()
                        .pane(self.active()?)
                        .split(Split::Below)
                        .build(),
                ),
                Command::KillPane => RequestParams::KillPane(
                    KillPaneRequestParams::builder()
                        .pane(self.active()?)
                        .build(),
                ),
                Command::FocusDown => Self::focus(self.pane_toward(Toward::Down)?),
                Command::FocusUp => Self::focus(self.pane_toward(Toward::Up)?),
                Command::FocusLeft => Self::focus(self.pane_toward(Toward::Left)?),
                Command::FocusRight => Self::focus(self.pane_toward(Toward::Right)?),
                Command::ResizePaneDown => Self::resize(self.active()?, Split::Below, 1),
                Command::ResizePaneUp => Self::resize(self.active()?, Split::Below, -1),
                Command::ResizePaneLeft => Self::resize(self.active()?, Split::Beside, -1),
                Command::ResizePaneRight => Self::resize(self.active()?, Split::Beside, 1),
                Command::SynchronizePanes => {
                    RequestParams::SyncPanes(SyncPanesRequestParams::builder().build())
                }
                Command::ToggleZoom => RequestParams::ZoomPane(
                    ZoomPaneRequestParams::builder()
                        .pane(self.active()?)
                        .build(),
                ),
                Command::SwitchPanePrevious => Self::swap(self.active()?, false),
                Command::SwitchPaneNext => Self::swap(self.active()?, true),
                // NOTE: Saying what something is called without saying what to call it is what
                // asks for a name, the way tmux asks for one at its own prompt. What it is called
                // already is what is typed over.
                Command::RenameSession { name } => match name {
                    Some(name) => RequestParams::RenameSession(
                        RenameSessionRequestParams::builder().name(name).build(),
                    ),
                    None => {
                        let called: String = self
                            .layout
                            .as_ref()
                            .map(|layout| layout.name().to_string())
                            .unwrap_or_default();
                        self.prompt = Some(Prompt::asking(Asking::SessionName, called));
                        return None;
                    }
                },
                Command::RenameWindow { name } => match name {
                    Some(name) => RequestParams::RenameWindow(
                        RenameWindowRequestParams::builder().name(name).build(),
                    ),
                    None => {
                        let called: String = self.window_name().unwrap_or_default();
                        self.prompt = Some(Prompt::asking(Asking::WindowName, called));
                        return None;
                    }
                },
                Command::NewSession { name } => {
                    RequestParams::NewSession(NewSessionRequestParams::builder().name(name).build())
                }
                Command::NewWindow { name } => {
                    RequestParams::NewWindow(NewWindowRequestParams::builder().name(name).build())
                }
                Command::ChangeMode { mode } => return self.change_mode(mode),
                Command::ListSessions => {
                    self.tree = Some(Tree::default());
                    RequestParams::ListSessions(ListSessionsRequestParams::builder().build())
                }
                Command::Detach => return Some(Effect::Quit),
            };

            Some(Effect::Request(Request::builder().params(params).build()))
        }

        /// Return what asking for what is typed to go to the pane asks for.
        fn focus(pane: Uuid) -> RequestParams {
            return RequestParams::SelectPane(
                SelectPaneRequestParams::builder().pane(pane).build(),
            );
        }

        /// Return what asking for the line on the given side of the pane to move asks for.
        fn resize(pane: Uuid, split: Split, by: isize) -> RequestParams {
            return RequestParams::DragPane(
                DragPaneRequestParams::builder()
                    .panes(vec![pane])
                    .split(split)
                    .by(by)
                    .build(),
            );
        }

        /// Return what asking for the pane to swap with the one before or after it asks for.
        fn swap(pane: Uuid, after: bool) -> RequestParams {
            return RequestParams::SwapPane(
                SwapPaneRequestParams::builder()
                    .pane(pane)
                    .after(after)
                    .build(),
            );
        }

        /// Show something other than the panes, which nothing is asked of the daemon for.
        fn change_mode(&mut self, mode: Mode) -> Option<Effect> {
            match mode {
                Mode::Prompt => {
                    self.prompt = Some(Prompt::asking(Asking::Command, String::new()));
                }
                Mode::Copy => {
                    // NOTE: A pane which draws one of insh's own components has nothing which has
                    // scrolled off the top of it, so there is nothing in it to look back through,
                    // but what is drawn in it is there to be read and taken all the same.
                    let pane: Uuid = self.active()?;
                    let at: Location = self
                        .panes
                        .get(&pane)
                        .and_then(Pane::copy_start)
                        .unwrap_or(Location { row: 0, column: 0 });
                    self.copying = Some(Copying::of(self.pane_size(&pane)?, at));
                    self.tell_panes_about_focus();
                }
            }

            None
        }

        /// Take hold of the lines which are drawn in the spot, which is nothing at all when no line
        /// is drawn there and both of them when it is where two lines meet.
        ///
        /// A line is on the far side of whichever pane is just before it, which is the pane the
        /// daemon is told to move it on the far side of.
        pub fn take_hold(&self, row: usize, column: usize) -> Option<Dragging> {
            let layout = self.layout.as_ref()?;
            if self.pane_drawn_at(row, column).is_some() {
                return None;
            }

            // NOTE: The row after the last of the panes is the footer rather than a line, even
            // though it is directly below them.
            let bottom: usize = layout
                .panes()
                .iter()
                .map(|pane| pane.at().row + pane.size().rows)
                .max()?;
            if row >= bottom {
                return None;
            }

            // NOTE: The row before a pane and the row after it count as well as the ones it is in,
            // and the same goes for the columns, because the spot where two lines meet is off the
            // corner of a pane rather than along the edge of one.
            let along = |spot: usize, start: usize, length: usize| {
                spot + 1 >= start && spot <= start + length
            };

            // A line down the screen has a pane on its left, and one across the screen has one
            // above it. Which it is says which way it moves when it is dragged.
            let beside: Vec<Uuid> = layout
                .panes()
                .iter()
                .filter(|pane| {
                    let (at, size) = (pane.at(), pane.size());
                    column == at.column + size.columns && along(row, at.row, size.rows)
                })
                .map(|pane| *pane.pane())
                .collect();
            let below: Vec<Uuid> = layout
                .panes()
                .iter()
                .filter(|pane| {
                    let (at, size) = (pane.at(), pane.size());
                    row == at.row + size.rows && along(column, at.column, size.columns)
                })
                .map(|pane| *pane.pane())
                .collect();

            let beside: Option<HeldLine> = (!beside.is_empty()).then_some(HeldLine {
                panes: beside,
                at: column,
            });
            let below: Option<HeldLine> = (!below.is_empty()).then_some(HeldLine {
                panes: below,
                at: row,
            });

            match (&beside, &below) {
                (None, None) => None,
                _ => Some(Dragging { beside, below }),
            }
        }

        /// Move the lines which are being dragged to where the mouse is now.
        pub fn dragged(&mut self, row: usize, column: usize) -> Option<Effect> {
            let dragging: &mut Dragging = self.dragging.as_mut()?;

            let beside: Option<Effect> =
                Self::move_line(dragging.beside.as_mut(), Split::Beside, column);
            let below: Option<Effect> = Self::move_line(dragging.below.as_mut(), Split::Below, row);

            return match (beside, below) {
                (None, below) => below,
                (beside, None) => beside,
                (beside, below) => Self::after(beside, below),
            };
        }

        /// Return what moving the line to the given row or column asks for, which is nothing when
        /// it is there already.
        fn move_line(line: Option<&mut HeldLine>, split: Split, now: usize) -> Option<Effect> {
            let line: &mut HeldLine = line?;

            let by: isize = isize::try_from(now).ok()? - isize::try_from(line.at).ok()?;
            if by == 0 {
                return None;
            }
            line.at = now;

            Some(Effect::Request(
                Request::builder()
                    .params(RequestParams::DragPane(
                        DragPaneRequestParams::builder()
                            .panes(line.panes.clone())
                            .split(split)
                            .by(by)
                            .build(),
                    ))
                    .build(),
            ))
        }

        /// Return the pane which is drawn in the given spot, whether or not it is the one what is
        /// typed goes to.
        fn pane_drawn_at(&self, row: usize, column: usize) -> Option<Uuid> {
            let layout = self.layout.as_ref()?;

            layout
                .panes()
                .iter()
                .find(|pane| {
                    let (at, size) = (pane.at(), pane.size());
                    row >= at.row
                        && row < at.row + size.rows
                        && column >= at.column
                        && column < at.column + size.columns
                })
                .map(|pane| *pane.pane())
        }

        /// Return the pane which is drawn in the given spot, which is none at all when that is
        /// not in one — the lines drawn between panes and the footer are not.
        ///
        /// The pane what is typed already goes to is not given back, so that clicking in it does
        /// not ask the daemon for something which is already so.
        pub fn pane_at(&self, row: usize, column: usize) -> Option<Uuid> {
            let layout = self.layout.as_ref()?;

            let clicked = layout.panes().iter().find(|pane| {
                let (at, size) = (pane.at(), pane.size());
                row >= at.row
                    && row < at.row + size.rows
                    && column >= at.column
                    && column < at.column + size.columns
            })?;

            match clicked.pane() == layout.pane() {
                true => None,
                false => Some(*clicked.pane()),
            }
        }

        /// Return the pane which is next along in the given direction, which is none at all when
        /// there is not one that way.
        ///
        /// Of the panes which are wholly that way, the nearest is taken, and of those the one
        /// which lines up with this pane the most. Lining up matters more than being near, since
        /// a pane which is alongside is the one meant even when a nearer one only clips a corner.
        fn pane_toward(&self, toward: Toward) -> Option<Uuid> {
            let layout = self.layout.as_ref()?;
            let from: &PaneLayout = layout
                .panes()
                .iter()
                .find(|pane| pane.pane() == layout.pane())?;

            let (at, size) = (from.at(), from.size());
            let (near, far): (usize, usize) = match toward {
                Toward::Left | Toward::Right => (at.row, at.row + size.rows),
                Toward::Up | Toward::Down => (at.column, at.column + size.columns),
            };

            layout
                .panes()
                .iter()
                .filter(|pane| pane.pane() != layout.pane())
                .filter_map(|pane| {
                    let (corner, other) = (pane.at(), pane.size());

                    let gap: usize = match toward {
                        Toward::Left => at.column.checked_sub(corner.column + other.columns)?,
                        Toward::Right => corner.column.checked_sub(at.column + size.columns)?,
                        Toward::Up => at.row.checked_sub(corner.row + other.rows)?,
                        Toward::Down => corner.row.checked_sub(at.row + size.rows)?,
                    };

                    let (start, end): (usize, usize) = match toward {
                        Toward::Left | Toward::Right => (corner.row, corner.row + other.rows),
                        Toward::Up | Toward::Down => (corner.column, corner.column + other.columns),
                    };
                    let overlap: usize = end.min(far).saturating_sub(start.max(near));

                    Some((pane, gap, overlap))
                })
                .max_by_key(|(_, gap, overlap)| (*overlap, usize::MAX - *gap))
                .map(|(pane, _, _)| *pane.pane())
        }

        /// Send what was typed where it goes.
        ///
        /// A pane with a program in it has it sent to the daemon, and one with one of insh's own
        /// components in it is given it here, because the daemon knows nothing about what is in
        /// one of those. When the window is being typed in together it goes to every pane of it
        /// either way, which is why the components are gone through here rather than the one pane
        /// what is typed usually goes to.
        pub fn send(&mut self, keys: Vec<KeyEvent>) -> Option<Effect> {
            let synchronized: bool = self
                .layout
                .as_ref()
                .is_some_and(insh_api::Layout::synchronized);
            let active: Uuid = self.active()?;

            let typed_in: Vec<Uuid> = match synchronized {
                true => self
                    .layout
                    .as_ref()?
                    .panes()
                    .iter()
                    .map(|pane| *pane.pane())
                    .collect(),
                false => vec![active],
            };

            let mut requests: Vec<Request> = Vec::new();
            let mut bell: bool = false;

            // NOTE: The daemon sends what is typed to every pane of a window which is being typed
            // in together itself, so it is only told once however many panes there are.
            if typed_in.iter().any(|pane| self.is_program(pane)) {
                let bytes: Vec<u8> = keys.iter().flat_map(Vec::<u8>::from).collect();
                if let Some(request) = self.write(bytes) {
                    requests.push(request);
                }
            }

            for pane in typed_in {
                if self.is_program(&pane) {
                    continue;
                }

                let events: Vec<PaneEvent> = keys
                    .iter()
                    .map(|key| PaneEvent::TermEvent(TermEvent::KeyEvent(key.clone())))
                    .collect();

                match self.tell_pane(&pane, events) {
                    Some(Effect::Request(request)) => requests.push(request),
                    Some(Effect::Requests(more)) => requests.extend(more),
                    Some(Effect::Bell) => bell = true,
                    _ => {}
                }
            }

            return match (requests.is_empty(), bell) {
                (false, _) => Some(Effect::Requests(requests)),
                (true, true) => Some(Effect::Bell),
                (true, false) => None,
            };
        }

        /// Return which of the panes of the window what is typed goes to is, counting from zero
        /// the way tmux counts them.
        pub fn pane_number(&self) -> usize {
            let layout = match &self.layout {
                Some(layout) => layout,
                None => return 0,
            };

            return layout
                .panes()
                .iter()
                .position(|pane| pane.pane() == layout.pane())
                .unwrap_or(0);
        }

        /// Return the request which looks the given number of rows further back through what has
        /// scrolled off the top of the pane what is typed goes to.
        /// Take what is being taken from the pane, put it on the clipboard, and stop looking back
        /// through the pane.
        ///
        /// Nothing is copied when nothing is being taken, so that the key does nothing at all
        /// rather than putting an empty string over whatever was on the clipboard already.
        pub fn copy(&mut self) -> Option<Effect> {
            let pane: Uuid = self.active()?;
            let size: Size = self.pane_size(&pane)?;
            let drawn: Fabric = self.panes.get(&pane)?.render(size);

            if let Some(text) = self.copying.as_ref()?.text(&drawn) {
                let mut clipboard = Clipboard::new();
                clipboard.copy(text);
            }

            self.leave_copying();
            return self.look_back(isize::MIN);
        }

        /// Return how big the pane is, which is nothing at all when it is not one of the panes
        /// being shown.
        pub fn pane_size(&self, pane: &Uuid) -> Option<Size> {
            return Self::size_of(self.layout.as_ref()?, pane);
        }

        /// Return how big the pane is where the panes are laid out.
        fn size_of(layout: &insh_api::Layout, pane: &Uuid) -> Option<Size> {
            return layout
                .panes()
                .iter()
                .find(|shown| shown.pane() == pane)
                .map(|shown| shown.size());
        }

        /// Put the pane back to what is on it now and then do the command.
        ///
        /// Both are asked for at once because a component only ever gives back the one effect, and
        /// the pane has to be put back first or else it is left showing what has scrolled off
        /// while the command has already moved on to another pane.
        pub fn stop_copying(&mut self, action: Action) -> Option<Effect> {
            self.leave_copying();

            let stop: Option<Effect> = self.look_back(isize::MIN);
            let done: Option<Effect> = self.perform(action);

            return Self::after(stop, done);
        }

        /// Stop looking back through the pane.
        pub fn leave_copying(&mut self) {
            self.copying = None;
            self.tell_panes_about_focus();
        }

        /// Tell every pane whether what is typed is going to it, so that only the ones it is going
        /// to show which of their rows is selected.
        ///
        /// Every pane of a window which is being typed in together is one of them, and none of them
        /// are while a pane is being looked back through, since the keys go to that instead.
        fn tell_panes_about_focus(&mut self) {
            let layout = match &self.layout {
                Some(layout) => layout,
                None => return,
            };

            let copying: bool = self.copying.is_some();
            for (id, pane) in &mut self.panes {
                pane.handle(PaneEvent::SetFocus {
                    focused: !copying && (layout.synchronized() || id == layout.pane()),
                });
            }
        }

        /// Return one effect which asks for the first thing and then the second.
        ///
        /// Only asking for things can be put together, since a component only ever gives back the
        /// one effect. Quitting is worth more than whatever was asked for first, because there is
        /// nothing left to show the answer in, and anything else which is not a request cannot be
        /// asked for alongside one.
        pub fn after(first: Option<Effect>, then: Option<Effect>) -> Option<Effect> {
            return match (first, then) {
                (Some(Effect::Request(first)), Some(Effect::Request(then))) => {
                    Some(Effect::Requests(vec![first, then]))
                }
                (Some(Effect::Request(first)), Some(Effect::Requests(then))) => {
                    let mut requests: Vec<Request> = vec![first];
                    requests.extend(then);
                    Some(Effect::Requests(requests))
                }
                (_, Some(Effect::Quit)) => Some(Effect::Quit),
                (Some(first), _) => Some(first),
                (None, then) => then,
            };
        }

        /// Look the given number of rows further back through what has scrolled off the top of
        /// the pane what is typed goes to.
        pub fn look_back(&self, rows: isize) -> Option<Effect> {
            let pane: Uuid = self.active()?;

            Some(Effect::Request(
                Request::builder()
                    .params(RequestParams::ScrollPane(
                        ScrollPaneRequestParams::builder()
                            .pane(pane)
                            .rows(rows)
                            .build(),
                    ))
                    .build(),
            ))
        }

        /// Return whether there is a program running in the pane.
        pub fn is_program(&self, pane: &Uuid) -> bool {
            self.panes.get(pane).is_some_and(Pane::is_program)
        }

        /// Give the events to the pane and ask for whatever it asks for.
        pub fn tell_pane(&mut self, pane: &Uuid, events: Vec<PaneEvent>) -> Option<Effect> {
            let dir: PathBuf = self.dir.clone();
            let shown = self.panes.get_mut(pane)?;

            let mut requests: Vec<Request> = Vec::new();
            let mut bell: bool = false;

            for event in events {
                match shown.handle(event) {
                    Some(PaneEffect::Request(request)) => requests.push(request),
                    Some(PaneEffect::Requests(more)) => requests.extend(more),
                    Some(PaneEffect::Run {
                        program,
                        args,
                        dir: run_in,
                    }) => {
                        requests.push(
                            Request::builder()
                                .params(RequestParams::RunInPane(
                                    RunInPaneRequestParams::builder()
                                        .pane(*pane)
                                        .program(program)
                                        .args(args)
                                        .dir(Some(run_in.unwrap_or_else(|| dir.clone())))
                                        .build(),
                                ))
                                .build(),
                        );
                    }
                    Some(PaneEffect::Bell) => bell = true,
                    None => {}
                }
            }

            match (requests.is_empty(), bell) {
                (false, _) => Some(Effect::Requests(requests)),
                (true, true) => Some(Effect::Bell),
                (true, false) => None,
            }
        }

        /// Return the request which sends the bytes to the pane what is typed goes to.
        ///
        /// The daemon is what sends it on to every pane of a window which is being typed in
        /// together, so only the one pane is named here however many of them there are.
        pub fn write(&self, bytes: Vec<u8>) -> Option<Request> {
            if bytes.is_empty() {
                return None;
            }
            let pane: Uuid = self.active()?;

            Some(
                Request::builder()
                    .params(RequestParams::WritePane(
                        WritePaneRequestParams::builder()
                            .pane(pane)
                            .bytes(bytes)
                            .build(),
                    ))
                    .build(),
            )
        }

        /// Return the bytes which the pasted text is sent to the program as.
        ///
        /// A terminal only wraps pasted text when the program running in it has asked for it, so
        /// the markers only go on when the program has.
        pub fn pasted(&self, text: &str) -> Vec<u8> {
            let wrapped: bool = self
                .active()
                .and_then(|pane| self.panes.get(&pane))
                .is_some_and(|pane| pane.modes().contains(Modes::BRACKETED_PASTE));

            if !wrapped {
                return text.as_bytes().to_vec();
            }

            let mut bytes: Vec<u8> = ansi::BracketedPaste::START.to_vec();
            bytes.extend_from_slice(text.as_bytes());
            bytes.extend_from_slice(ansi::BracketedPaste::END);
            bytes
        }

        /// Take note of where everything is now, and return what the panes need asked of the
        /// daemon in order to have anything to show.
        pub fn relayout(&mut self, layout: insh_api::Layout) -> Vec<Request> {
            // NOTE: What was being looked back through is gone when what is typed goes somewhere
            // else now or the pane holds something else, so where in it was being read from no
            // longer means anything.
            let was_active: Option<PaneLayout> = self.layout.as_ref().and_then(|old| {
                old.panes()
                    .iter()
                    .find(|shown| shown.pane() == old.pane())
                    .cloned()
            });
            let still_there: bool = was_active.as_ref().is_some_and(|was_active| {
                layout.pane() == was_active.pane()
                    && layout.panes().iter().any(|shown| {
                        shown.pane() == was_active.pane() && shown.kind() == was_active.kind()
                    })
            });
            let mut put_back: Option<Request> = None;
            if self.copying.is_some() && !still_there {
                self.copying = None;
                put_back = was_active
                    .filter(|was_active| {
                        layout
                            .panes()
                            .iter()
                            .any(|shown| shown.pane() == was_active.pane())
                    })
                    .map(|was_active| {
                        Request::builder()
                            .params(RequestParams::ScrollPane(
                                ScrollPaneRequestParams::builder()
                                    .pane(*was_active.pane())
                                    .rows(isize::MIN)
                                    .build(),
                            ))
                            .build()
                    });
            }

            // The panes of a window which is no longer being looked at are not drawn any more.
            self.panes
                .retain(|pane, _| layout.panes().iter().any(|shown| shown.pane() == pane));

            for shown in layout.panes() {
                match self.panes.get_mut(shown.pane()) {
                    Some(pane) => pane.relayout(shown.clone()),
                    None => {
                        let pane = Pane::new(
                            PaneProps::builder()
                                .layout(shown.clone())
                                .dir(self.dir.clone())
                                .config(self.config.clone())
                                .build(),
                        );
                        self.panes.insert(*shown.pane(), pane);
                    }
                }
            }

            // NOTE: How big the pane being looked back through is, taken before the layout is
            // put away, since only the pane what is typed goes to is the one being read from.
            let active_size: Option<Size> = Self::size_of(&layout, layout.pane());

            self.layout = Some(layout);
            self.tell_panes_about_focus();

            // NOTE: A line between two panes being moved, or the terminal changing size, changes
            // how big the pane being looked back through is, and this is where being told so
            // arrives. Where in the pane is being read from is kept inside it so that it never
            // points past the end of what is drawn.
            if let (Some(copying), Some(size)) = (self.copying.as_mut(), active_size) {
                copying.resize(size);
            }

            let mut asked: Vec<Request> =
                self.panes.values_mut().flat_map(Pane::take_asked).collect();
            asked.extend(put_back);
            return asked;
        }

        /// Give what the daemon said to every pane, since any of them may have asked for it.
        pub fn tell_panes(&mut self, response: Response) -> Option<Effect> {
            let panes: Vec<Uuid> = self.panes.keys().copied().collect();

            let mut requests: Vec<Request> = Vec::new();
            for pane in panes {
                match self.tell_pane(&pane, vec![PaneEvent::Response(response.clone())]) {
                    Some(Effect::Request(request)) => requests.push(request),
                    Some(Effect::Requests(more)) => requests.extend(more),
                    _ => {}
                }
            }

            match requests.is_empty() {
                true => None,
                false => Some(Effect::Requests(requests)),
            }
        }

        /// Take note of what has changed about the screen of a pane.
        pub fn apply(&mut self, screen: &insh_api::PaneScreen) {
            if let Some(pane) = self.panes.get_mut(screen.pane()) {
                pane.handle(PaneEvent::Screen(screen.clone()));
            }
        }

        /// Draw the panes, the lines between them, and the footer along the bottom.
        pub fn render(&self, size: Size) -> Fabric {
            let rows: usize = size.rows.saturating_sub(1);
            let mut fabric = Fabric::new(Size::new(rows, size.columns));

            let layout = match &self.layout {
                Some(layout) => layout,
                // Nothing has been said about the session yet, so there is nothing to draw.
                None => return fabric.quilt_bottom(self.footer(size.columns)),
            };

            for shown in layout.panes() {
                let pane: &Pane = match self.panes.get(shown.pane()) {
                    Some(pane) => pane,
                    None => continue,
                };
                let mut drawn: Fabric = pane.render(shown.size());

                // What is being taken out of the pane is marked over what the program wrote, so
                // that it is plain where in it is being read from and how far along it has got.
                if shown.pane() == layout.pane() {
                    if let Some(copying) = &self.copying {
                        copying.show(&mut drawn);
                    }
                }

                fabric.blit(&drawn, shown.at());
            }

            // NOTE: The lines are drawn once all of the panes are, and all together rather than a
            // pane at a time, because which character a spot is drawn as depends on which of the
            // spots around it are lines as well.
            Borders::of(layout.panes(), layout.pane(), Size::new(rows, size.columns))
                .draw(&mut fabric);

            // The cursor of the pane what is typed goes to is the one the terminal shows, so that
            // it is where the program which is being typed at has it. Nothing is typed at while
            // the pane is being looked back through, and where in it is being read from is marked
            // rather than shown with the cursor, so there is none to show then.
            fabric.set_cursor(match self.copying {
                Some(_) => None,
                None => self.panes.get(layout.pane()).and_then(Pane::cursor_at),
            });

            // The list of sessions is drawn over the panes rather than beside them, so that it
            // does not change how big any of them is while it is up.
            if let Some(tree) = &self.tree {
                fabric = tree.render(Size::new(rows, size.columns));
                fabric.set_cursor(None);
            }

            // The prompt is typed in where the footer usually is, which is what tmux does.
            let along_the_bottom: Fabric = match &self.prompt {
                Some(prompt) => prompt.render(Size::new(1, size.columns)),
                None => self.footer(size.columns),
            };

            fabric.quilt_bottom(along_the_bottom)
        }

        /// Draw the footer, which says which session it is and which windows it has.
        fn footer(&self, columns: usize) -> Fabric {
            let info = SessionInfo { state: self };

            let props = FooterProps::builder()
                .name(Self::now())
                .info(&info)
                .background(Color::SessionFooterBackground)
                .build();
            Footer::new(props).render(Size::new(1, columns))
        }
    }

    /// What a session shows in its footer.
    struct SessionInfo<'session> {
        /// The session it is for.
        state: &'session State,
    }

    impl FooterInfo for SessionInfo<'_> {
        /// Return what the session is called and the windows of it, with the one being looked at
        /// marked, along with whatever the window is doing which is out of the ordinary.
        fn text(&self) -> String {
            let layout = match &self.state.layout {
                Some(layout) => layout,
                None => return String::new(),
            };

            // NOTE: Only where you are is said, and not the other windows of the session. The
            // switcher is what lists them, and listing them here would say the most about a
            // session with the most windows, which is when there is the least room to say it in.
            let name: &str = layout
                .windows()
                .iter()
                .find(|window| window.window() == layout.window())
                .map(|window| window.name())
                .unwrap_or_default();

            // NOTE: A window which is being typed in together is not being typed in at any one of
            // its panes, so none of them is named. Leaving it out is what says the window is being
            // typed in together, so it is not said again in words.
            let where_you_are: String = match layout.synchronized() {
                true => format!("{}:{}", layout.name(), name),
                false => format!("{}:{}:{}", layout.name(), name, self.state.pane_number()),
            };

            // NOTE: Nothing is said about what the window is doing which is out of the ordinary.
            // Being zoomed or being looked back through is plain from the pane itself, and the
            // footer saying so as well is one more thing moving about down there to read past.
            return where_you_are;
        }

        /// Return what the machine is called, which goes just before the time of day.
        ///
        /// NOTE: Nothing about whichever pane what is typed goes to is shown here, so that the
        /// right of the footer says the same thing whatever is in the panes.
        fn position(&self) -> String {
            self.state.hostname.clone()
        }
    }
}
pub use session::Session;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Effect, Event, Props, Session};
    use crate::color::Color;
    use crate::config::Config;

    use file_info::FileInfo;
    use file_type::FileType;
    use insh_api::{
        GetFilesResponseParams, Layout, LayoutResponseParams, PaneKind, PaneLayout, PaneScreen,
        Request, RequestParams, Response, ResponseParams, Split, Window,
    };
    use rend::{Fabric, Location, Size};
    use term::{Key, KeyEvent, KeyMods, MouseButton, MouseEvent, MouseEventKind, TermEvent};
    use terminal_emulator::Modes;
    use til::Component;

    use uuid::Uuid;

    /// The request the session is reported on.
    const REQUEST: Uuid = Uuid::from_u128(100);

    /// Return the pane with the given number.
    fn pane(number: u128) -> Uuid {
        Uuid::from_u128(number)
    }

    /// Return a pane of the given number at the given spot and size.
    fn shown(number: u128, at: (usize, usize), size: (usize, usize), kind: PaneKind) -> PaneLayout {
        PaneLayout::builder()
            .pane(pane(number))
            .at(Location::builder().row(at.0).column(at.1).build())
            .size(Size::new(size.0, size.1))
            .kind(kind)
            .build()
    }

    /// Return the response which says where the panes are.
    fn layout_response(panes: Vec<PaneLayout>, focused: u128, screens: Vec<PaneScreen>) -> Event {
        let window: Uuid = Uuid::from_u128(200);
        let layout: Layout = Layout::builder()
            .session(Uuid::from_u128(201))
            .name("0")
            .windows(vec![Window::builder().window(window).name("0").build()])
            .window(window)
            .panes(panes)
            .pane(pane(focused))
            .build();

        Event::Response(
            Response::builder()
                .uuid(REQUEST)
                .params(ResponseParams::Layout(
                    LayoutResponseParams::builder()
                        .layout(layout)
                        .screens(screens)
                        .build(),
                ))
                .build(),
        )
    }

    /// Return a session which has been told about the panes, along with what it asked for.
    fn session(
        panes: Vec<PaneLayout>,
        focused: u128,
        screens: Vec<PaneScreen>,
    ) -> (Session, Effect) {
        let mut session: Session = Session::new(
            Props::builder()
                .request(REQUEST)
                .size(Size::new(24, 80))
                .dir(PathBuf::from("/tmp"))
                .config(Config::default())
                .build(),
        );
        let effect: Option<Effect> = session.handle(layout_response(panes, focused, screens));

        (session, effect.unwrap_or(Effect::Requests(Vec::new())))
    }

    /// Press the key in the session.
    fn press(session: &mut Session, key: Key, mods: KeyMods) -> Option<Effect> {
        session.handle(Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
            key,
            mods,
        })))
    }

    /// Start looking back through the pane.
    fn enter_copy_mode(session: &mut Session) {
        press(session, Key::Char('b'), KeyMods::CONTROL);
        press(session, Key::Char('['), KeyMods::NONE);
    }

    /// Do something with the mouse in the given spot.
    fn mouse(
        session: &mut Session,
        kind: MouseEventKind,
        row: usize,
        column: usize,
    ) -> Option<Effect> {
        session.handle(Event::TermEvent(TermEvent::Mouse(MouseEvent {
            kind,
            row,
            column,
            mods: KeyMods::NONE,
        })))
    }

    /// Return which lines are asked to move, and by how much.
    fn drags(effect: Option<Effect>) -> Vec<(Vec<Uuid>, bool, isize)> {
        let requests: Vec<Request> = match effect {
            Some(Effect::Request(request)) => vec![request],
            Some(Effect::Requests(requests)) => requests,
            _ => Vec::new(),
        };

        requests
            .iter()
            .map(|request| match request.params() {
                RequestParams::DragPane(params) => (
                    params.panes().to_vec(),
                    matches!(params.split(), Split::Beside),
                    params.by(),
                ),
                _ => panic!("Expected a line to be asked to move."),
            })
            .collect()
    }

    /// Return four panes in a square, whose lines cross at row 10 and column 40.
    fn square() -> Vec<PaneLayout> {
        vec![
            shown(1, (0, 0), (10, 40), PaneKind::Component),
            shown(2, (0, 41), (10, 39), PaneKind::Component),
            shown(3, (11, 0), (12, 40), PaneKind::Component),
            shown(4, (11, 41), (12, 39), PaneKind::Component),
        ]
    }

    /// Return the session showing a browser in a pane to the right of another, with files in it.
    fn browsing() -> Session {
        let panes: Vec<PaneLayout> = vec![
            shown(1, (0, 0), (23, 40), PaneKind::Component),
            shown(2, (0, 41), (23, 39), PaneKind::Component),
        ];
        let (mut session, effect): (Session, Effect) = session(panes, 2, Vec::new());
        let requests: Vec<Request> = match effect {
            Effect::Requests(requests) => requests,
            _ => panic!("Expected the panes to ask for their files."),
        };

        let files: Vec<FileInfo> = vec![FileInfo::builder()
            .path(PathBuf::from("/tmp/file"))
            .r#type(Ok(FileType::File))
            .build()];
        for request in requests
            .iter()
            .filter(|request| matches!(request.params(), RequestParams::GetFiles(_)))
        {
            session.handle(Event::Response(
                Response::builder()
                    .uuid(*request.uuid())
                    .params(ResponseParams::GetFiles(
                        GetFilesResponseParams::builder()
                            .result(Ok(files.clone()))
                            .build(),
                    ))
                    .build(),
            ));
        }

        session
    }

    /// Return the background of the spot which is drawn.
    fn background(fabric: &Fabric, row: usize, column: usize) -> Option<ansi::Color> {
        fabric.spot(row, column).style().background()
    }

    #[test_case::test_case(1, 0; "a pane in the top left, where the cursor is the same either way")]
    #[test_case::test_case(2, 41; "a pane on the right, which is the end of the line if the corner is counted twice")]
    fn test_copy_mode_starts_at_the_cursor_of_a_program(focused: u128, column: usize) {
        let cursor: Location = Location::builder().row(2).column(3).build();
        let panes: Vec<PaneLayout> = vec![
            shown(1, (0, 0), (23, 40), PaneKind::Program),
            shown(2, (0, 41), (23, 39), PaneKind::Program),
        ];
        let screen: PaneScreen = PaneScreen::builder()
            .pane(pane(focused))
            .rows(Vec::new())
            .cursor(Some(cursor))
            .modes(Modes::default())
            .build();
        let (mut session, _): (Session, Effect) = session(panes, focused, vec![screen]);

        enter_copy_mode(&mut session);
        let fabric: Fabric = session.render(Size::new(24, 80));

        assert_eq!(
            background(&fabric, 2, column + 3),
            Some(Color::Highlight.into())
        );
    }

    #[test]
    fn test_copy_mode_starts_at_the_start_of_the_selected_row_of_the_browser() {
        let mut session: Session = browsing();

        enter_copy_mode(&mut session);
        let fabric: Fabric = session.render(Size::new(24, 80));

        // The directory bar is the row above the files.
        assert_eq!(background(&fabric, 1, 41), Some(Color::Highlight.into()));
        assert_eq!(
            background(&fabric, 1, 42),
            Some(Color::UnfocusedHighlight.into())
        );
    }

    #[test]
    fn test_only_the_pane_what_is_typed_goes_to_is_yellow_when_panes_are_first_shown() {
        // Being told about every pane at once is what attaching to a session looks like.
        let session: Session = browsing();

        let fabric: Fabric = session.render(Size::new(24, 80));

        assert_eq!(
            background(&fabric, 1, 0),
            Some(Color::UnfocusedHighlight.into())
        );
        assert_eq!(background(&fabric, 1, 41), Some(Color::Highlight.into()));
    }

    #[test]
    fn test_the_selected_row_of_the_browser_is_yellow_again_after_copy_mode() {
        let mut session: Session = browsing();

        enter_copy_mode(&mut session);
        press(&mut session, Key::Char('q'), KeyMods::NONE);
        let fabric: Fabric = session.render(Size::new(24, 80));

        assert_eq!(background(&fabric, 1, 42), Some(Color::Highlight.into()));
    }

    #[test]
    fn test_copy_mode_is_left_when_what_is_typed_goes_to_another_pane() {
        let mut session: Session = browsing();
        enter_copy_mode(&mut session);

        let panes: Vec<PaneLayout> = vec![
            shown(1, (0, 0), (23, 40), PaneKind::Component),
            shown(2, (0, 41), (23, 39), PaneKind::Component),
        ];
        let effect: Option<Effect> = session.handle(layout_response(panes, 1, Vec::new()));
        let fabric: Fabric = session.render(Size::new(24, 80));

        // The pane which was being looked back through is put back to what is on it now.
        let requests: Vec<Request> = match effect {
            Some(Effect::Requests(requests)) => requests,
            _ => panic!("Expected the pane to be put back."),
        };
        assert!(requests.iter().any(|request| matches!(
            request.params(),
            RequestParams::ScrollPane(params) if *params.pane() == pane(2)
        )));
        assert_eq!(background(&fabric, 1, 1), Some(Color::Highlight.into()));
        assert_eq!(
            background(&fabric, 1, 41),
            Some(Color::UnfocusedHighlight.into())
        );
    }

    #[test]
    fn test_copy_mode_is_left_when_the_pane_holds_something_else() {
        let panes = |kind: PaneKind| {
            vec![
                shown(1, (0, 0), (23, 40), PaneKind::Component),
                shown(2, (0, 41), (23, 39), kind),
            ]
        };
        let (mut session, _): (Session, Effect) = session(panes(PaneKind::Program), 2, Vec::new());
        enter_copy_mode(&mut session);

        session.handle(layout_response(panes(PaneKind::Component), 2, Vec::new()));

        // Keys go to what is in the pane again rather than to moving about in it.
        assert!(press(&mut session, Key::Char('b'), KeyMods::CONTROL).is_none());
        let fabric: Fabric = session.render(Size::new(24, 80));
        assert_ne!(background(&fabric, 0, 41), Some(Color::Highlight.into()));
    }

    #[test]
    fn test_copy_mode_stays_when_a_line_is_moved() {
        let mut session: Session = browsing();
        enter_copy_mode(&mut session);

        let panes: Vec<PaneLayout> = vec![
            shown(1, (0, 0), (23, 30), PaneKind::Component),
            shown(2, (0, 31), (23, 49), PaneKind::Component),
        ];
        session.handle(layout_response(panes, 2, Vec::new()));
        let fabric: Fabric = session.render(Size::new(24, 80));

        assert_eq!(
            background(&fabric, 1, 32),
            Some(Color::UnfocusedHighlight.into())
        );
    }

    #[test]
    fn test_the_footer_is_not_a_line_to_take_hold_of() {
        let (mut session, _): (Session, Effect) = session(square(), 1, Vec::new());

        mouse(
            &mut session,
            MouseEventKind::Press(MouseButton::Left),
            23,
            5,
        );
        let moved = drags(mouse(
            &mut session,
            MouseEventKind::Drag(MouseButton::Left),
            20,
            5,
        ));

        assert!(moved.is_empty());
    }

    #[test]
    fn test_where_lines_cross_moves_both_of_them() {
        let (mut session, _): (Session, Effect) = session(square(), 1, Vec::new());

        mouse(
            &mut session,
            MouseEventKind::Press(MouseButton::Left),
            10,
            40,
        );
        let moved = drags(mouse(
            &mut session,
            MouseEventKind::Drag(MouseButton::Left),
            12,
            37,
        ));

        assert_eq!(
            moved,
            vec![
                (vec![pane(1), pane(3)], true, -3),
                (vec![pane(1), pane(2)], false, 2),
            ]
        );
    }

    #[test]
    fn test_where_lines_cross_moves_only_the_line_which_the_mouse_went_across() {
        let (mut session, _): (Session, Effect) = session(square(), 1, Vec::new());

        mouse(
            &mut session,
            MouseEventKind::Press(MouseButton::Left),
            10,
            40,
        );
        let moved = drags(mouse(
            &mut session,
            MouseEventKind::Drag(MouseButton::Left),
            10,
            41,
        ));

        assert_eq!(moved, vec![(vec![pane(1), pane(3)], true, 1)]);
    }

    #[test]
    fn test_a_line_running_into_another_moves_both_of_them() {
        // One pane on the left and two stacked on the right.
        let panes: Vec<PaneLayout> = vec![
            shown(1, (0, 0), (23, 40), PaneKind::Component),
            shown(2, (0, 41), (10, 39), PaneKind::Component),
            shown(3, (11, 41), (12, 39), PaneKind::Component),
        ];
        let (mut session, _): (Session, Effect) = session(panes, 1, Vec::new());

        mouse(
            &mut session,
            MouseEventKind::Press(MouseButton::Left),
            10,
            40,
        );
        let moved = drags(mouse(
            &mut session,
            MouseEventKind::Drag(MouseButton::Left),
            11,
            41,
        ));

        assert_eq!(
            moved,
            vec![(vec![pane(1)], true, 1), (vec![pane(2)], false, 1)]
        );
    }

    #[test]
    fn test_a_straight_run_of_a_line_moves_only_that_line() {
        let (mut session, _): (Session, Effect) = session(square(), 1, Vec::new());

        mouse(
            &mut session,
            MouseEventKind::Press(MouseButton::Left),
            5,
            40,
        );
        let moved = drags(mouse(
            &mut session,
            MouseEventKind::Drag(MouseButton::Left),
            7,
            42,
        ));

        assert_eq!(moved, vec![(vec![pane(1)], true, 2)]);
    }
}

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
    use std::ops::Range;
    use std::path::PathBuf;

    use super::super::borders::Borders;
    use super::super::copying::{Copying, Moving};
    use super::super::pane::{Effect as PaneEffect, Event as PaneEvent, Pane, Props as PaneProps};
    use super::super::prompt::{Asking, Command, Effect as PromptEffect, Mode, Prompt};
    use super::super::tree::{Effect as TreeEffect, Tree};
    use super::{Action, Effect, Event, Props, Toward};
    use crate::clipboard::Clipboard;
    use crate::color::Color;
    use crate::components::common::{Footer, FooterInfo, FooterProps, Hover};
    use crate::config::Config;
    use crate::typed_path::TypedPath;

    use insh_api::{
        AttachRequestParams, CopyPaneRequestParams, DirScope, DragPaneRequestParams,
        KillPaneRequestParams, ListSessionsRequestParams, NewSessionRequestParams,
        NewWindowRequestParams, PaneKind, PaneLayout, RenameSessionRequestParams,
        RenameWindowRequestParams, Request, RequestParams, ResizeClientRequestParams, Response,
        ResponseParams, RunCommandRequestParams, RunInPaneRequestParams, ScrollPaneRequestParams,
        SearchPaneRequestParams, SelectPaneRequestParams, SelectWindowRequestParams,
        SetDirRequestParams, Split, SplitPaneRequestParams, SwapPaneRequestParams,
        SyncPanesRequestParams, WritePaneRequestParams, ZoomPaneRequestParams,
    };
    use rend::{Cell, Fabric, Location, Seeking, Size};
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
            // NOTE: A terminal says nothing about hiding the mouse, but most hide it while what
            // is typed is being typed, so nothing is grayed again until it moves.
            if let Event::TermEvent(TermEvent::KeyEvent(_) | TermEvent::Paste(_)) = &event {
                self.state.hovering = None;
            }

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
            if self.state.is_copying() {
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
                self.state.size = size;
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
                //
                // The pane goes on being looked back through while they are done, the way it does in
                // tmux, so that going to another pane and back again finds it where it was left.
                TermEvent::KeyEvent(key_event) => match self.state.parse(key_event) {
                    Parsed::Pending => return None,
                    Parsed::Command(action) => return self.state.perform(action),
                    Parsed::Unknown(mut keys) => keys.drain(..).next()?,
                },
                TermEvent::Mouse(mouse_event) => return self.handle_mouse_event(mouse_event),
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
                self.state.active_copying()?.other_key();
            }

            // Going about the pane and taking from it, which is the rest of what the keys mean.
            let moving: Moving = match key_event.key {
                Key::Char('q') => return self.state.leave_copying(),
                // NOTE: Giving up on what is being taken is not giving up on looking back through
                // the pane, which is how tmux has it with vi keys: escape clears the selection and
                // q is what leaves.
                Key::Escape => {
                    self.state.active_copying()?.unmark();
                    return None;
                }
                Key::Char(' ' | 'v') => {
                    let top: usize = self.state.first_shown_row();
                    let drawn: Fabric = self.state.drawn_active()?;
                    self.state.active_copying()?.mark(&drawn, top);
                    return None;
                }
                Key::CarriageReturn | Key::Char('y') => return self.state.copy(),
                // NOTE: A slash looks down and a question mark looks up, the way they do in vim,
                // and n and p look again the same way and the other way.
                Key::Char('/') => return self.state.ask_search(true),
                Key::Char('?') => return self.state.ask_search(false),
                Key::Char('n') => return self.state.search_again(false),
                Key::Char('p') => return self.state.search_again(true),
                Key::Char('h') | Key::Left => Moving::Left,
                Key::Char('j') | Key::Down => Moving::Down,
                Key::Char('k') | Key::Up => Moving::Up,
                Key::Char('l') | Key::Right => Moving::Right,
                Key::Char('0') | Key::Home => Moving::Start,
                Key::Char('$') | Key::End => Moving::End,
                Key::Char('w') => Moving::NextWord,
                Key::Char('W') => Moving::NextBigWord,
                Key::Char('b') => Moving::PreviousWord,
                Key::Char('B') => Moving::PreviousBigWord,
                Key::Char('e') => Moving::EndOfWord,
                Key::Char('E') => Moving::EndOfBigWord,
                // NOTE: These move the pane itself rather than where in it is being looked at,
                // since there is no going the whole way in a pane which is one screen big.
                Key::Char('g') => {
                    return match self.state.active_copying()?.go_back() {
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
            let drawn: Fabric = self.state.drawn_active()?;
            let rows: isize = self.state.active_copying()?.move_toward(moving, &drawn);
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
                    // NOTE: This is complained about rather than left to the daemon because only
                    // the client knows what a relative path is from, and it is checked before the
                    // prompt goes so that what was wrong can be said where it was typed.
                    if let Command::SetDir { dir, .. } = &command {
                        let dir: PathBuf = self.state.typed_dir(dir);
                        if !dir.is_dir() {
                            let message = format!("Not a directory: {}", dir.display());
                            if let Some(prompt) = self.state.prompt.as_mut() {
                                prompt.complain(message);
                            }
                            return None;
                        }
                    }

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
                    match what {
                        Asking::SearchDown => self.state.search(name, true),
                        Asking::SearchUp => self.state.search(name, false),
                        _ => self.state.name(what, name),
                    }
                }
            }
        }

        /// Do what was done with the mouse.
        ///
        /// Pressing in a pane sends what is typed there, and pressing on one of the lines between
        /// two panes takes hold of it so that moving the mouse drags it, and scrolling looks back
        /// through what has scrolled off the top of a pane, the way it does in tmux. A program in
        /// a pane which has asked to be told about the mouse is told instead, the way tmux tells
        /// it, apart from while it is being looked back through.
        fn handle_mouse_event(&mut self, mouse_event: MouseEvent) -> Option<Effect> {
            let (row, column) = (mouse_event.row, mouse_event.column);
            self.state.hovering = Some(Location { row, column });

            if let Some((pane, bytes)) = self.state.mouse_report(&mouse_event) {
                match mouse_event.kind {
                    MouseEventKind::Press(_) => self.state.pressed_in = Some(pane),
                    MouseEventKind::Release(_) => self.state.pressed_in = None,
                    _ => {}
                }
                let report: Option<Effect> = self.state.write_to(pane, bytes).map(Effect::Request);

                // NOTE: Pressing in a pane which what is typed does not go to makes it the one it
                // goes to first, so that the report goes to it rather than to the pane before.
                let pressed: bool = matches!(mouse_event.kind, MouseEventKind::Press(_));
                return match pressed && Some(pane) != self.state.active() {
                    false => report,
                    true => State::after(
                        Some(Effect::Request(
                            Request::builder().params(State::focus(pane)).build(),
                        )),
                        report,
                    ),
                };
            }

            match mouse_event.kind {
                MouseEventKind::Press(MouseButton::Left) => {
                    if let Some(dragging) = self.state.take_hold(row, column) {
                        self.state.dragging = Some(dragging);
                        return None;
                    }

                    // The footer is where the prompt is typed, so pressing on it opens the prompt.
                    if row + 1 == self.state.size.rows {
                        return self.state.change_mode(Mode::Prompt);
                    }

                    // NOTE: Pressing in a pane which what is typed does not go to makes it the one
                    // it goes to, and one of insh's own components is told where it was pressed
                    // as well, so that pressing on a row of it does what pressing there means.
                    let focus: Option<Effect> = self.state.pane_at(row, column).map(|pane| {
                        Effect::Request(
                            Request::builder()
                                .params(RequestParams::SelectPane(
                                    SelectPaneRequestParams::builder().pane(pane).build(),
                                ))
                                .build(),
                        )
                    });
                    let told: Option<Effect> = self.state.tell_component(&mouse_event);
                    State::after(focus, told)
                }
                MouseEventKind::Drag(MouseButton::Left) => self.state.dragged(row, column),
                // NOTE: The mouse wheel scrolls one of insh's own components which what is typed
                // goes to itself, and anywhere else it is what the multiplexer does with it.
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                    if self.state.is_active_component(row, column) =>
                {
                    self.state.tell_component(&mouse_event)
                }
                MouseEventKind::ScrollUp => self.state.scroll(row, column, State::SCROLLED_ROWS),
                MouseEventKind::ScrollDown => self.state.scroll(row, column, -State::SCROLLED_ROWS),
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

            // NOTE: The same goes for the text taken from a pane, which is only put on the
            // clipboard once the daemon has said what it is, since it is the one which has the
            // rows which have scrolled off the top.
            if let ResponseParams::CopyPane(params) = response.params() {
                let mut clipboard = Clipboard::new();
                clipboard.copy(params.text().to_string());
                return None;
            }

            // NOTE: The same goes for where something being looked for in a pane was found.
            if let ResponseParams::SearchPane(params) = response.params() {
                return self.state.found(response.uuid(), params.found());
            }

            // NOTE: The same goes for what the daemon suggests the rest of what is being typed at
            // the prompt is, which is asked for on a request of the prompt's own.
            if let ResponseParams::SuggestCommand(_) = response.params() {
                if let Some(prompt) = self.state.prompt.as_mut() {
                    prompt.handle_response(&response);
                }
                return None;
            }

            // NOTE: A directory which is being typed at the prompt is suggested the way one typed
            // in a directory bar is, and the bar of a browser in a pane asks for the same thing, so
            // the prompt only takes the answer to the request which it asked.
            if let Some(prompt) = self.state.prompt.as_mut() {
                if prompt.is_suggestion(response.uuid()) {
                    prompt.handle_response(&response);
                    return None;
                }
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
        /// Where in each pane which is being looked back through is being looked at.
        copying: HashMap<Uuid, Copying>,
        /// The request which asked where something being looked for is, and the pane it is being
        /// looked for in, while it is being waited for.
        searching: Option<(Uuid, Uuid)>,
        /// What was last looked for, and whether it was looked for going down, which is what is
        /// looked for again.
        last_search: Option<(String, bool)>,
        /// Where the mouse last was, which is nowhere until it has been moved.
        hovering: Option<Location>,
        /// The pane of a program which was last told the mouse was pressed in it, which is told
        /// what is done with the mouse after that even before it is the one what is typed goes to.
        pressed_in: Option<Uuid>,
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
                copying: HashMap::new(),
                searching: None,
                last_search: None,
                hovering: None,
                pressed_in: None,
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

        /// How many rows one turn of the mouse wheel, or one step of scrolling with a trackpad,
        /// scrolls a pane by.
        pub const SCROLLED_ROWS: isize = 3;

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
        pub fn active(&self) -> Option<Uuid> {
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
                        self.prompt = Some(
                            Prompt::builder()
                                .asking(Asking::SessionName)
                                .typed(called)
                                .paths(self.typed_paths())
                                .build(),
                        );
                        return None;
                    }
                },
                Command::RenameWindow { name } => match name {
                    Some(name) => RequestParams::RenameWindow(
                        RenameWindowRequestParams::builder().name(name).build(),
                    ),
                    None => {
                        let called: String = self.window_name().unwrap_or_default();
                        self.prompt = Some(
                            Prompt::builder()
                                .asking(Asking::WindowName)
                                .typed(called)
                                .paths(self.typed_paths())
                                .build(),
                        );
                        return None;
                    }
                },
                Command::SetDir { session, dir, .. } => RequestParams::SetDir(
                    SetDirRequestParams::builder()
                        .scope(match session {
                            true => DirScope::Session,
                            false => DirScope::Window,
                        })
                        .dir(self.typed_dir(&dir))
                        .build(),
                ),
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

        /// Return the directory a new pane of the window being looked at starts in.
        fn starting_dir(&self) -> PathBuf {
            return self
                .layout
                .as_ref()
                .and_then(|layout| layout.dir())
                .unwrap_or(&self.dir)
                .clone();
        }

        /// Return the directory which was typed, where a relative one is from the directory of the
        /// pane what is typed goes to.
        pub fn typed_dir(&self, typed: &str) -> PathBuf {
            return self.typed_paths().resolve(typed);
        }

        /// Return what a path typed at the prompt means, which is from the directory of the pane
        /// what is typed goes to when it is a relative one.
        fn typed_paths(&self) -> TypedPath {
            let from: PathBuf = self
                .active()
                .and_then(|pane| self.panes.get(&pane))
                .map_or_else(|| self.starting_dir(), |pane| pane.dir().to_path_buf());

            return TypedPath::builder()
                .home(dirs::home_dir())
                .from(from)
                .build();
        }

        /// Return what asking for what is typed to go to the pane asks for.
        pub fn focus(pane: Uuid) -> RequestParams {
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
        pub fn change_mode(&mut self, mode: Mode) -> Option<Effect> {
            match mode {
                Mode::Prompt => {
                    self.prompt = Some(
                        Prompt::builder()
                            .asking(Asking::Command)
                            .paths(self.typed_paths())
                            .build(),
                    );
                }
                Mode::Copy => {
                    // NOTE: A pane which draws one of insh's own components has nothing which has
                    // scrolled off the top of it, so there is nothing in it to look back through,
                    // but what is drawn in it is there to be read and taken all the same.
                    let pane: Uuid = self.active()?;
                    if self.copying.contains_key(&pane) {
                        return None;
                    }
                    let at: Location = self
                        .panes
                        .get(&pane)
                        .and_then(Pane::copy_start)
                        .unwrap_or(Location { row: 0, column: 0 });
                    let mut copying: Copying = Copying::of(self.pane_size(&pane)?, at);
                    // NOTE: Looking back from the end of a prompt goes up along the ends of the
                    // rows, the way it does after going to the end of one.
                    if self.is_program(&pane) {
                        copying = copying.sticking_past_the_end(&self.drawn_active()?);
                    }
                    self.copying.insert(pane, copying);
                    self.tell_panes_about_focus();

                    // NOTE: What is on the screen of a program is held where it is while it is
                    // looked back through, rather than going on scrolling away as the program goes
                    // on writing, the way tmux holds it.
                    return match self.is_program(&pane) {
                        true => self.look_back(0),
                        false => None,
                    };
                }
            }

            None
        }

        /// Return whether the pane what is typed goes to is being looked back through.
        pub fn is_copying(&self) -> bool {
            return self
                .active()
                .is_some_and(|pane| self.copying.contains_key(&pane));
        }

        /// Return where in the pane what is typed goes to is being looked at, while it is being
        /// looked back through.
        pub fn active_copying(&mut self) -> Option<&mut Copying> {
            let pane: Uuid = self.active()?;
            return self.copying.get_mut(&pane);
        }

        /// Ask what to look for in the pane being looked back through, going down or up.
        pub fn ask_search(&mut self, down: bool) -> Option<Effect> {
            self.prompt = Some(
                Prompt::builder()
                    .asking(match down {
                        true => Asking::SearchDown,
                        false => Asking::SearchUp,
                    })
                    .paths(self.typed_paths())
                    .build(),
            );
            return None;
        }

        /// Look for the phrase in the pane being looked back through, going down or up, and take
        /// note of it as what is looked for again.
        pub fn search(&mut self, phrase: String, down: bool) -> Option<Effect> {
            let found: Option<Effect> = self.find(&phrase, down);
            self.last_search = Some((phrase, down));
            return found;
        }

        /// Look again for what was last looked for, the same way as before or the other way.
        pub fn search_again(&mut self, reverse: bool) -> Option<Effect> {
            let (phrase, down): (String, bool) = self.last_search.clone()?;
            return self.find(&phrase, down != reverse);
        }

        /// Look for the phrase from where in the pane being looked back through is being looked
        /// at.
        ///
        /// A program has rows which have scrolled off the top of it, and only the daemon has
        /// those, so it is asked where the phrase is. One of insh's own components has nothing
        /// which has scrolled off, so it is looked for in what is drawn.
        fn find(&mut self, phrase: &str, down: bool) -> Option<Effect> {
            let pane: Uuid = self.active()?;
            let top: usize = self.first_shown_row();
            let drawn: Fabric = self.drawn_active()?;
            let copying: &mut Copying = self.copying.get_mut(&pane)?;

            if !self.panes.get(&pane).is_some_and(Pane::is_program) {
                return match copying.search(&Seeking::from(phrase), &drawn, down) {
                    Some(at) => {
                        copying.go_to(at);
                        None
                    }
                    None => self.bell.then_some(Effect::Bell),
                };
            }

            let request = Request::builder()
                .params(RequestParams::SearchPane(
                    SearchPaneRequestParams::builder()
                        .pane(pane)
                        .phrase(phrase.to_string())
                        .from(copying.at(&drawn, top))
                        .down(down)
                        .build(),
                ))
                .build();
            self.searching = Some((*request.uuid(), pane));

            return Some(Effect::Request(request));
        }

        /// Go to where the daemon found what was being looked for, which is counted from the
        /// first row the program in the pane ever wrote on, and look back through the pane as far
        /// as it takes for that to be on the screen.
        ///
        /// It is looked back through no further than that, so that the row it is on is the top or
        /// the bottom one, which is never past either end of what there is to look at.
        pub fn found(&mut self, request: &Uuid, found: Option<Location>) -> Option<Effect> {
            let (asked, pane): (Uuid, Uuid) = self.searching?;
            if asked != *request {
                return None;
            }
            self.searching = None;

            let found: Location = match found {
                Some(found) => found,
                None => return self.bell.then_some(Effect::Bell),
            };
            let rows: usize = self.pane_size(&pane)?.rows;
            let top: usize = self.panes.get(&pane)?.first_shown_row();
            let bottom: usize = top + rows.saturating_sub(1);

            let (row, back): (usize, isize) = match found.row {
                row if row < top => (0, isize::try_from(top - row).ok()?),
                row if row > bottom => {
                    (rows.saturating_sub(1), -isize::try_from(row - bottom).ok()?)
                }
                row => (row - top, 0),
            };
            self.copying.get_mut(&pane)?.go_to(Location {
                row,
                column: found.column,
            });

            return match back {
                0 => None,
                back => self.look_back_in(pane, back),
            };
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
            let clicked: Uuid = self.pane_under(row, column)?;

            match Some(clicked) == self.active() {
                true => None,
                false => Some(clicked),
            }
        }

        /// Return the pane which is drawn in the spot, which is none at all when it is one of the
        /// lines between them or the footer.
        fn pane_under(&self, row: usize, column: usize) -> Option<Uuid> {
            let layout = self.layout.as_ref()?;

            let under = layout.panes().iter().find(|pane| {
                let (at, size) = (pane.at(), pane.size());
                row >= at.row
                    && row < at.row + size.rows
                    && column >= at.column
                    && column < at.column + size.columns
            })?;

            Some(*under.pane())
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
            let top: usize = self.first_shown_row();
            let drawn: Fabric = self.drawn_active()?;
            let copying: &Copying = self.copying.get(&pane)?;

            // NOTE: What is being taken from a program may be more than fits on the screen, and
            // only the daemon has the rows which have scrolled off, so it is asked for the text.
            // One of insh's own components has nothing which has scrolled off, so all of what is
            // being taken from it is what is drawn.
            let taken: Option<Effect> = match self.is_program(&pane) {
                true => copying.selection(&drawn, top).map(|(from, to)| {
                    Effect::Request(
                        Request::builder()
                            .params(RequestParams::CopyPane(
                                CopyPaneRequestParams::builder()
                                    .pane(pane)
                                    .from(from)
                                    .to(to)
                                    .build(),
                            ))
                            .build(),
                    )
                }),
                false => {
                    if let Some(text) = copying.text(&drawn, top) {
                        let mut clipboard = Clipboard::new();
                        clipboard.copy(text);
                    }
                    None
                }
            };

            let stop: Option<Effect> = self.leave_copying();
            return Self::after(taken, stop);
        }

        /// Return what is drawn in the pane what is typed goes to.
        pub fn drawn_active(&self) -> Option<Fabric> {
            let pane: Uuid = self.active()?;
            let size: Size = self.pane_size(&pane)?;
            return Some(self.panes.get(&pane)?.render(size));
        }

        /// Return which row the top one of the pane what is typed goes to is, counted from the
        /// first row the program in it ever wrote on.
        pub fn first_shown_row(&self) -> usize {
            return self
                .active()
                .and_then(|pane| self.panes.get(&pane))
                .map_or(0, Pane::first_shown_row);
        }

        /// Do what scrolling the given number of rows up, or down when that is negative, in the
        /// spot does.
        ///
        /// Scrolling in a pane which what is typed does not go to makes it the one it goes to,
        /// the way clicking in it does. Scrolling up in a program looks back through what has
        /// scrolled off the top of it, which stops again once it is scrolled back to the bottom,
        /// the way tmux has it. A program which is writing on the other screen has nothing which
        /// has scrolled off, so it is sent the arrow keys instead, which is what a terminal does
        /// to let a program like less or vim be scrolled through.
        pub fn scroll(&mut self, row: usize, column: usize, rows: isize) -> Option<Effect> {
            let pane: Uuid = self.pane_under(row, column)?;
            if Some(pane) != self.active() {
                return Some(Effect::Request(
                    Request::builder().params(Self::focus(pane)).build(),
                ));
            }
            if self.copying.contains_key(&pane) {
                return self.scroll_copying(rows);
            }
            if !self.is_program(&pane) {
                return None;
            }

            if self
                .panes
                .get(&pane)?
                .modes()
                .contains(Modes::ALTERNATE_SCREEN)
            {
                let key = KeyEvent {
                    key: match rows > 0 {
                        true => Key::Up,
                        false => Key::Down,
                    },
                    mods: KeyMods::NONE,
                };
                let bytes: Vec<u8> = (0..rows.unsigned_abs())
                    .flat_map(|_| Vec::<u8>::from(&key))
                    .collect();
                return self.write(bytes).map(Effect::Request);
            }

            // There is nothing below the bottom of a pane to scroll down to.
            if rows < 0 {
                return None;
            }

            self.change_mode(Mode::Copy);
            if let Some(copying) = self.copying.remove(&pane) {
                self.copying.insert(pane, copying.leaving_at_bottom());
            }
            return self.look_back(rows);
        }

        /// Scroll the pane which is being looked back through the given number of rows up, or
        /// down when that is negative.
        fn scroll_copying(&mut self, rows: isize) -> Option<Effect> {
            // NOTE: Looking back through it stops once it is scrolled back to the bottom, when it
            // was scrolling which began it, since that is the only way there is to have come.
            let looking_back: usize = self
                .active()
                .and_then(|pane| self.panes.get(&pane))
                .map_or(0, Pane::looking_back);
            let bottom: bool = rows < 0 && looking_back <= rows.unsigned_abs();
            if bottom && self.active_copying()?.leaves_at_bottom() {
                return self.leave_copying();
            }

            return self.look_back(rows);
        }

        /// Return what was done with the mouse, but where in the pane it was done rather than where
        /// on the whole screen.
        fn inside(&self, pane: &Uuid, mouse_event: &MouseEvent) -> Option<MouseEvent> {
            let at: Location = self
                .layout
                .as_ref()?
                .panes()
                .iter()
                .find(|shown| shown.pane() == pane)?
                .at();

            Some(MouseEvent {
                row: mouse_event.row.checked_sub(at.row)?,
                column: mouse_event.column.checked_sub(at.column)?,
                ..mouse_event.clone()
            })
        }

        /// Return whether the spot is in one of insh's own components which what is typed goes to
        /// and which is not being looked back through.
        pub fn is_active_component(&self, row: usize, column: usize) -> bool {
            return self.pane_under(row, column).is_some_and(|pane| {
                Some(pane) == self.active()
                    && !self.is_program(&pane)
                    && !self.copying.contains_key(&pane)
            });
        }

        /// Tell the one of insh's own components which is in the spot what was done with the
        /// mouse there, which nothing is told about in a program or a pane which is being looked
        /// back through.
        pub fn tell_component(&mut self, mouse_event: &MouseEvent) -> Option<Effect> {
            let pane: Uuid = self.pane_under(mouse_event.row, mouse_event.column)?;
            if self.is_program(&pane) || self.copying.contains_key(&pane) {
                return None;
            }

            let inside: MouseEvent = self.inside(&pane, mouse_event)?;
            return self.tell_pane(&pane, vec![PaneEvent::TermEvent(TermEvent::Mouse(inside))]);
        }

        /// Return the pane which a program has asked to be told about what was done with the
        /// mouse in it, and the bytes to tell it with, which is nothing at all when none has.
        ///
        /// Only pressing in a pane tells a pane which what is typed does not go to yet, since that
        /// is what makes it the one it goes to. A line between two panes which is being dragged is
        /// not a pane's business, nor is a pane which is being looked back through.
        pub fn mouse_report(&self, mouse_event: &MouseEvent) -> Option<(Uuid, Vec<u8>)> {
            if self.dragging.is_some() {
                return None;
            }

            let pane: Uuid = self.pane_under(mouse_event.row, mouse_event.column)?;
            // NOTE: Letting go of the mouse in a pane which was just pressed in to go to it is
            // its business, even though the daemon has not said that it is the one gone to yet.
            let pressed: bool = matches!(mouse_event.kind, MouseEventKind::Press(_));
            let ours: bool = Some(pane) == self.active() || Some(pane) == self.pressed_in;
            if self.copying.contains_key(&pane) || (!pressed && !ours) {
                return None;
            }
            let shown: &Pane = self.panes.get(&pane).filter(|shown| shown.is_program())?;

            let modes: Modes = shown.modes();
            let wanted: Modes = match mouse_event.kind {
                MouseEventKind::Moved => Modes::REPORT_MOUSE_MOTION,
                MouseEventKind::Drag(_) => Modes::REPORT_MOUSE_DRAGS | Modes::REPORT_MOUSE_MOTION,
                _ => {
                    Modes::REPORT_MOUSE_CLICKS
                        | Modes::REPORT_MOUSE_DRAGS
                        | Modes::REPORT_MOUSE_MOTION
                }
            };
            if !modes.intersects(wanted) {
                return None;
            }

            // The program is told where in its own pane the mouse is.
            let inside: MouseEvent = self.inside(&pane, mouse_event)?;
            let bytes: Vec<u8> =
                inside.report(modes.contains(Modes::CONTROL_SEQUENCE_MOUSE_REPORTS))?;
            return Some((pane, bytes));
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

        /// Stop looking back through the pane what is typed goes to.
        ///
        /// Looking back no further is what puts the pane back to what is on it now, and what lets
        /// go of what was being held on the screen of the program in it.
        pub fn leave_copying(&mut self) -> Option<Effect> {
            let pane: Uuid = self.active()?;
            self.copying.remove(&pane);
            self.tell_panes_about_focus();

            return self.look_back(isize::MIN);
        }

        /// Tell every pane whether what is typed is going to it, so that only the ones it is going
        /// to show which of their rows is selected.
        ///
        /// Every pane of a window which is being typed in together is one of them, and none of them
        /// are while the pane what is typed goes to is being looked back through, since the keys
        /// go to that instead. Nor is a pane which is being looked back through.
        fn tell_panes_about_focus(&mut self) {
            let layout = match &self.layout {
                Some(layout) => layout,
                None => return,
            };

            let copying: bool = self.copying.contains_key(layout.pane());
            for (id, pane) in &mut self.panes {
                pane.handle(PaneEvent::SetFocus {
                    focused: !copying
                        && !self.copying.contains_key(id)
                        && (layout.synchronized() || id == layout.pane()),
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
            return self.look_back_in(self.active()?, rows);
        }

        /// Look the given number of rows further back through what has scrolled off the top of
        /// the pane, which is held where it is for as long as it is being looked back through.
        fn look_back_in(&self, pane: Uuid, rows: isize) -> Option<Effect> {
            Some(Effect::Request(Self::scroll_request(
                pane,
                rows,
                self.copying.contains_key(&pane),
            )))
        }

        /// Return the request which looks the given number of rows further back through the pane,
        /// holding what is on the screen of it where it is or letting it go.
        fn scroll_request(pane: Uuid, rows: isize, hold: bool) -> Request {
            return Request::builder()
                .params(RequestParams::ScrollPane(
                    ScrollPaneRequestParams::builder()
                        .pane(pane)
                        .rows(rows)
                        .hold(hold)
                        .build(),
                ))
                .build();
        }

        /// Return whether there is a program running in the pane.
        pub fn is_program(&self, pane: &Uuid) -> bool {
            self.panes.get(pane).is_some_and(Pane::is_program)
        }

        /// Give the events to the pane and ask for whatever it asks for.
        pub fn tell_pane(&mut self, pane: &Uuid, events: Vec<PaneEvent>) -> Option<Effect> {
            let dir: PathBuf = self.starting_dir();
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
            return self.write_to(self.active()?, bytes);
        }

        /// Return the request which sends the bytes to the given pane.
        pub fn write_to(&self, pane: Uuid, bytes: Vec<u8>) -> Option<Request> {
            if bytes.is_empty() {
                return None;
            }

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
            // NOTE: A pane is looked back through for as long as it is shown and holds what it
            // held when that began, whichever pane what is typed goes to, the way tmux has it. One
            // whose program has finished has nothing left to look back through, and one in a
            // window which is no longer being looked at is let go of, since nothing would be left
            // to let go of what is held on its screen.
            let held: Vec<Uuid> = self.copying.keys().copied().collect();
            let mut put_back: Vec<Request> = Vec::new();
            for pane in held {
                let was: Option<PaneKind> =
                    self.panes.get(&pane).map(|shown| match shown.is_program() {
                        true => PaneKind::Program,
                        false => PaneKind::Component,
                    });
                let now: Option<PaneKind> = layout
                    .panes()
                    .iter()
                    .find(|shown| *shown.pane() == pane)
                    .map(PaneLayout::kind);
                if now.is_some() && now == was {
                    continue;
                }

                self.copying.remove(&pane);
                if was == Some(PaneKind::Program) && now.is_none() {
                    put_back.push(Self::scroll_request(pane, isize::MIN, false));
                }
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
                                .dir(shown.dir().or(layout.dir()).unwrap_or(&self.dir).clone())
                                .config(self.config.clone())
                                .build(),
                        );
                        self.panes.insert(*shown.pane(), pane);
                    }
                }
            }

            // NOTE: A line between two panes being moved, or the terminal changing size, changes
            // how big a pane being looked back through is, and this is where being told so
            // arrives. Where in the pane is being read from is kept inside it so that it never
            // points past the end of what is drawn.
            for (pane, copying) in self.copying.iter_mut() {
                if let Some(size) = Self::size_of(&layout, pane) {
                    copying.resize(size);
                }
            }

            self.layout = Some(layout);
            self.tell_panes_about_focus();

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
                if let Some(copying) = self.copying.get(shown.pane()) {
                    let focused: bool = shown.pane() == layout.pane();
                    copying.show(&mut drawn, pane.first_shown_row(), focused);
                }

                fabric.blit(&drawn, shown.at());
            }

            // Whatever pressing on would go to is grayed while the mouse is over it, and written in
            // black so that it can be read on the gray. Only what is written in a row of a list is
            // grayed, though the whole row is what the mouse is over.
            if let Some(hovering) = self.hovering {
                for shown in layout.panes() {
                    let (at, pane_size) = (shown.at(), shown.size());
                    let inside: bool = hovering.row >= at.row
                        && hovering.row < at.row + pane_size.rows
                        && hovering.column >= at.column
                        && hovering.column < at.column + pane_size.columns;
                    if !inside || self.copying.contains_key(shown.pane()) {
                        continue;
                    }
                    let hover: Hover = match self
                        .panes
                        .get(shown.pane())
                        .and_then(|pane| pane.hovers(hovering.row - at.row))
                    {
                        Some(hover) => hover,
                        None => continue,
                    };

                    let row: Range<usize> = at.column..at.column + pane_size.columns;
                    let columns: Range<usize> = match hover {
                        Hover::Row => row,
                        Hover::Text => match row.clone().rev().find(|column| {
                            *fabric.spot(hovering.row, *column).cell() != Cell::BLANK
                        }) {
                            Some(last) => at.column..last + 1,
                            None => continue,
                        },
                    };
                    fabric.set_background(Some(Color::Hover.into()), hovering.row, columns.clone());
                    fabric.set_color(Some(Color::InvertedText.into()), hovering.row, columns);
                }
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
            fabric.set_cursor(match self.copying.contains_key(layout.pane()) {
                true => None,
                false => self.panes.get(layout.pane()).and_then(Pane::cursor_at),
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
                None => {
                    let mut footer: Fabric = self.footer(size.columns);
                    if self.hovering.is_some_and(|hovering| hovering.row == rows) {
                        footer.set_background(Some(Color::Hover.into()), 0, 0..size.columns);
                        footer.set_color(Some(Color::InvertedText.into()), 0, 0..size.columns);
                    }
                    footer
                }
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
    use std::path::{Path, PathBuf};

    use super::{Effect, Event, Props, Session};
    use crate::color::Color;
    use crate::config::Config;

    use file_info::FileInfo;
    use file_type::FileType;
    use insh_api::{
        DirScope, GetFilesResponseParams, Layout, LayoutResponseParams, PaneKind, PaneLayout,
        PaneScreen, Request, RequestParams, Response, ResponseParams, SearchPaneResponseParams,
        Split, Window,
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

    /// Return a session with one program in it, writing on the screen the modes say, along with
    /// how far back it is being looked through.
    fn program(modes: Modes, looking_back: usize) -> Session {
        let screen: PaneScreen = PaneScreen::builder()
            .pane(pane(1))
            .rows(Vec::new())
            .cursor(Some(Location::builder().row(0).column(0).build()))
            .modes(modes)
            .looking_back(looking_back)
            .build();
        let (session, _): (Session, Effect) = session(
            vec![shown(1, (0, 0), (23, 80), PaneKind::Program)],
            1,
            vec![screen],
        );
        session
    }

    /// Return whether the pane is being looked back through, which is when the cursor of the
    /// program in it is not shown.
    fn copying(session: &Session) -> bool {
        session.render(Size::new(24, 80)).cursor().is_none()
    }

    /// Return how many rows the effect asks to look further back through the pane, which is
    /// nothing at all when it does not.
    fn scrolled(effect: &Option<Effect>) -> Option<isize> {
        let requests: Vec<&Request> = match effect {
            Some(Effect::Request(request)) => vec![request],
            Some(Effect::Requests(requests)) => requests.iter().collect(),
            _ => Vec::new(),
        };

        requests.iter().find_map(|request| match request.params() {
            RequestParams::ScrollPane(params) => Some(params.rows()),
            _ => None,
        })
    }

    #[test]
    fn test_scrolling_up_in_a_program_looks_back_through_it() {
        let mut session: Session = program(Modes::default(), 0);

        let effect: Option<Effect> = mouse(&mut session, MouseEventKind::ScrollUp, 5, 5);

        assert_eq!(scrolled(&effect), Some(3));
        assert!(copying(&session), "it should be in copy mode");
    }

    #[test]
    fn test_scrolling_down_in_a_program_which_is_not_being_looked_back_through_does_nothing() {
        let mut session: Session = program(Modes::default(), 0);

        let effect: Option<Effect> = mouse(&mut session, MouseEventKind::ScrollDown, 5, 5);

        assert!(effect.is_none());
        assert!(!copying(&session));
    }

    #[test]
    fn test_scrolling_back_to_the_bottom_stops_looking_back() {
        let mut session: Session = program(Modes::default(), 0);
        mouse(&mut session, MouseEventKind::ScrollUp, 5, 5);

        let effect: Option<Effect> = mouse(&mut session, MouseEventKind::ScrollDown, 5, 5);

        assert_eq!(scrolled(&effect), Some(isize::MIN));
        assert!(!copying(&session), "it should have left copy mode");
    }

    #[test]
    fn test_scrolling_back_to_the_bottom_does_not_stop_looking_back_when_it_was_asked_for() {
        let mut session: Session = program(Modes::default(), 0);
        enter_copy_mode(&mut session);

        mouse(&mut session, MouseEventKind::ScrollDown, 5, 5);

        assert!(copying(&session), "it should still be in copy mode");
    }

    #[test]
    fn test_scrolling_on_the_other_screen_sends_the_arrow_keys() {
        let mut session: Session = program(Modes::default() | Modes::ALTERNATE_SCREEN, 0);

        let effect: Option<Effect> = mouse(&mut session, MouseEventKind::ScrollUp, 5, 5);

        let written: Vec<u8> = match effect {
            Some(Effect::Request(request)) => match request.params() {
                RequestParams::WritePane(params) => params.bytes().to_vec(),
                _ => panic!("Expected the keys to be sent."),
            },
            _ => panic!("Expected the keys to be sent."),
        };
        assert_eq!(written, b"\x1b[A\x1b[A\x1b[A");
        assert!(!copying(&session));
    }

    #[test]
    fn test_copying_from_a_program_asks_the_daemon_for_the_text() {
        let mut session: Session = program(Modes::default(), 0);
        enter_copy_mode(&mut session);
        press(&mut session, Key::Char('v'), KeyMods::NONE);

        let effect: Option<Effect> = press(&mut session, Key::Char('y'), KeyMods::NONE);

        let requests: Vec<Request> = match effect {
            Some(Effect::Requests(requests)) => requests,
            _ => panic!("Expected the text to be asked for."),
        };
        assert!(matches!(
            requests[0].params(),
            RequestParams::CopyPane(params) if *params.pane() == pane(1)
        ));
    }

    /// Open the prompt and type the characters at it.
    fn type_at_prompt(session: &mut Session, typed: &str) {
        press(session, Key::Char('b'), KeyMods::CONTROL);
        press(session, Key::Char(':'), KeyMods::NONE);
        for character in typed.chars() {
            press(session, Key::Char(character), KeyMods::NONE);
        }
    }

    #[test_case::test_case("set-dir /tmp", DirScope::Window, "/tmp"; "the window by default")]
    #[test_case::test_case("set-dir -w /tmp", DirScope::Window, "/tmp"; "the window")]
    #[test_case::test_case("set-dir -s /tmp", DirScope::Session, "/tmp"; "the session")]
    #[test_case::test_case("set-dir ..", DirScope::Window, "/"; "from the directory of the pane")]
    fn test_setting_the_starting_directory_asks_the_daemon(
        typed: &str,
        scope: DirScope,
        dir: &str,
    ) {
        let mut session: Session = browsing();
        type_at_prompt(&mut session, typed);

        let requests: Vec<Request> = match press(&mut session, Key::CarriageReturn, KeyMods::NONE) {
            Some(Effect::Requests(requests)) => requests,
            _ => panic!("Expected the directory to be set."),
        };

        match requests.last().unwrap().params() {
            RequestParams::SetDir(params) => {
                assert_eq!(params.scope(), scope);
                assert_eq!(params.dir(), &PathBuf::from(dir));
            }
            _ => panic!("Expected the directory to be set."),
        }
    }

    #[test]
    fn test_a_starting_directory_which_is_not_a_directory_is_complained_about() {
        let mut session: Session = browsing();
        type_at_prompt(&mut session, "set-dir /no/such/dir");

        let effect: Option<Effect> = press(&mut session, Key::CarriageReturn, KeyMods::NONE);

        assert!(effect.is_none());
        let fabric: Fabric = session.render(Size::new(24, 80));
        assert!(fabric
            .text(23, 0..80)
            .starts_with("Not a directory: /no/such/dir"));
    }

    #[test_case::test_case(Some("/var"), "/var"; "the one which was set")]
    #[test_case::test_case(None, "/tmp"; "the one the client was started in when none was set")]
    fn test_a_new_pane_starts_in_the_starting_directory(set: Option<&str>, expected: &str) {
        let window: Uuid = Uuid::from_u128(200);
        let layout: Layout = Layout::builder()
            .session(Uuid::from_u128(201))
            .name("0")
            .windows(vec![Window::builder().window(window).name("0").build()])
            .window(window)
            .panes(vec![shown(1, (0, 0), (23, 80), PaneKind::Component)])
            .pane(pane(1))
            .dir(set.map(PathBuf::from))
            .build();
        let mut session: Session = Session::new(
            Props::builder()
                .request(REQUEST)
                .size(Size::new(24, 80))
                .dir(PathBuf::from("/tmp"))
                .config(Config::default())
                .build(),
        );

        let effect: Option<Effect> = session.handle(Event::Response(
            Response::builder()
                .uuid(REQUEST)
                .params(ResponseParams::Layout(
                    LayoutResponseParams::builder()
                        .layout(layout)
                        .screens(Vec::new())
                        .build(),
                ))
                .build(),
        ));

        let requests: Vec<Request> = match effect {
            Some(Effect::Requests(requests)) => requests,
            _ => panic!("Expected the pane to ask for its files."),
        };
        let asked: Option<&Path> = requests.iter().find_map(|request| match request.params() {
            RequestParams::GetFiles(params) => Some(params.dir()),
            _ => None,
        });
        assert_eq!(asked, Some(Path::new(expected)));
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
    fn test_copy_mode_stays_with_its_pane_when_what_is_typed_goes_to_another() {
        let mut session: Session = browsing();
        enter_copy_mode(&mut session);
        let panes = || {
            vec![
                shown(1, (0, 0), (23, 40), PaneKind::Component),
                shown(2, (0, 41), (23, 39), PaneKind::Component),
            ]
        };

        let effect: Option<Effect> = session.handle(layout_response(panes(), 1, Vec::new()));
        let fabric: Fabric = session.render(Size::new(24, 80));

        // Nothing is put back, and the pane which is being looked back through is still marked
        // where it is being looked at while the other one has what is typed, in gray.
        assert!(effect.is_none());
        assert_eq!(background(&fabric, 1, 1), Some(Color::Highlight.into()));
        assert_eq!(
            background(&fabric, 1, 41),
            Some(Color::UnfocusedHighlight.into())
        );
        assert_eq!(
            background(&fabric, 1, 42),
            Some(Color::UnfocusedHighlight.into())
        );

        // Coming back to it, the keys go to moving about in it again.
        session.handle(layout_response(panes(), 2, Vec::new()));
        let effect: Option<Effect> = press(&mut session, Key::Char('q'), KeyMods::NONE);
        assert_eq!(scrolled(&effect), Some(isize::MIN));
    }

    #[test]
    fn test_clicking_another_pane_while_looking_back_goes_to_it() {
        let mut session: Session = browsing();
        enter_copy_mode(&mut session);

        let effect: Option<Effect> =
            mouse(&mut session, MouseEventKind::Press(MouseButton::Left), 5, 5);

        assert!(matches!(
            effect,
            Some(Effect::Request(request))
                if matches!(request.params(), RequestParams::SelectPane(params) if *params.pane() == pane(1))
        ));
    }

    /// Return whether the effect asks for what is on the screen of a pane to be held, which is
    /// nothing at all when it does not ask about that.
    fn held(effect: &Option<Effect>) -> Option<bool> {
        let requests: Vec<&Request> = match effect {
            Some(Effect::Request(request)) => vec![request],
            Some(Effect::Requests(requests)) => requests.iter().collect(),
            _ => Vec::new(),
        };

        requests.iter().find_map(|request| match request.params() {
            RequestParams::ScrollPane(params) => Some(params.hold()),
            _ => None,
        })
    }

    #[test]
    fn test_the_screen_of_a_program_is_held_while_it_is_looked_back_through() {
        let mut session: Session = program(Modes::default(), 0);

        press(&mut session, Key::Char('b'), KeyMods::CONTROL);
        let entered: Option<Effect> = press(&mut session, Key::Char('['), KeyMods::NONE);
        let moved: Option<Effect> = press(&mut session, Key::Char('G'), KeyMods::NONE);
        let left: Option<Effect> = press(&mut session, Key::Char('q'), KeyMods::NONE);

        assert_eq!(held(&entered), Some(true));
        assert_eq!(held(&moved), Some(true));
        assert_eq!(held(&left), Some(false));
    }

    /// Look for the phrase going down through the pane being looked back through.
    fn search(session: &mut Session, phrase: &str) -> Option<Effect> {
        press(session, Key::Char('/'), KeyMods::NONE);
        for character in phrase.chars() {
            press(session, Key::Char(character), KeyMods::NONE);
        }
        press(session, Key::CarriageReturn, KeyMods::NONE)
    }

    #[test]
    fn test_searching_a_program_asks_the_daemon_and_goes_where_it_was_found() {
        let mut session: Session = program(Modes::default(), 0);
        enter_copy_mode(&mut session);

        let request: Request = match search(&mut session, "a b") {
            Some(Effect::Request(request)) => request,
            _ => panic!("Expected the daemon to be asked where it is."),
        };
        match request.params() {
            RequestParams::SearchPane(params) => {
                assert_eq!(params.phrase(), "a b");
                assert!(params.down());
            }
            _ => panic!("Expected the daemon to be asked where it is."),
        }

        // It was found above the top of the pane, so the pane is looked back through as far as
        // it takes to bring it onto the top row.
        let effect: Option<Effect> = session.handle(Event::Response(
            Response::builder()
                .uuid(*request.uuid())
                .params(ResponseParams::SearchPane(
                    SearchPaneResponseParams::builder()
                        .found(Some(Location::builder().row(0).column(4).build()))
                        .build(),
                ))
                .build(),
        ));
        assert_eq!(scrolled(&effect), None);
        let fabric: Fabric = session.render(Size::new(24, 80));
        assert_eq!(background(&fabric, 0, 4), Some(Color::Highlight.into()));

        // Looking again the other way asks again, the other way.
        match press(&mut session, Key::Char('p'), KeyMods::NONE) {
            Some(Effect::Request(request)) => match request.params() {
                RequestParams::SearchPane(params) => assert!(!params.down()),
                _ => panic!("Expected the daemon to be asked again."),
            },
            _ => panic!("Expected the daemon to be asked again."),
        }
    }

    #[test]
    fn test_searching_a_component_goes_where_it_is_drawn() {
        let mut session: Session = browsing();
        enter_copy_mode(&mut session);

        let effect: Option<Effect> = search(&mut session, "file");
        let fabric: Fabric = session.render(Size::new(24, 80));

        // The footer names the browser and the file is the selected row, so the first one after
        // the start of that row is the name of the file in it, a column in.
        assert!(effect.is_none());
        let found: usize = (41..80)
            .find(|column| background(&fabric, 1, *column) == Some(Color::Highlight.into()))
            .unwrap();
        assert_eq!(fabric.text(1, found..found + 4), "file");
    }

    #[test_case::test_case(MouseEventKind::Press(MouseButton::Left), b"\x1b[<0;3;2M"; "a press")]
    #[test_case::test_case(MouseEventKind::ScrollUp, b"\x1b[<64;3;2M"; "a scroll, which does not look back")]
    fn test_the_mouse_is_reported_to_a_program_which_asked(kind: MouseEventKind, expected: &[u8]) {
        let mut session: Session = program(
            Modes::default() | Modes::REPORT_MOUSE_CLICKS | Modes::CONTROL_SEQUENCE_MOUSE_REPORTS,
            0,
        );

        let effect: Option<Effect> = mouse(&mut session, kind, 1, 2);

        let written: Vec<u8> = match effect {
            Some(Effect::Request(request)) => match request.params() {
                RequestParams::WritePane(params) => params.bytes().to_vec(),
                _ => panic!("Expected the mouse to be reported."),
            },
            _ => panic!("Expected the mouse to be reported."),
        };
        assert_eq!(written, expected);
        assert!(!copying(&session));
    }

    #[test]
    fn test_pressing_in_another_program_which_asked_goes_to_it_and_reports_it() {
        let screen: PaneScreen = PaneScreen::builder()
            .pane(pane(1))
            .rows(Vec::new())
            .cursor(None)
            .modes(Modes::default() | Modes::REPORT_MOUSE_CLICKS)
            .build();
        let (mut session, _): (Session, Effect) = session(
            vec![
                shown(1, (0, 0), (23, 40), PaneKind::Program),
                shown(2, (0, 41), (23, 39), PaneKind::Program),
            ],
            2,
            vec![screen],
        );

        let effect: Option<Effect> =
            mouse(&mut session, MouseEventKind::Press(MouseButton::Left), 0, 0);

        let requests: Vec<Request> = match effect {
            Some(Effect::Requests(requests)) => requests,
            _ => panic!("Expected the pane to be gone to and the press reported."),
        };
        assert!(matches!(requests[0].params(), RequestParams::SelectPane(_)));
        assert!(matches!(
            requests[1].params(),
            RequestParams::WritePane(params) if params.bytes() == b"\x1b[M !!"
        ));

        // Letting go is reported to it as well, before the daemon has said it was gone to.
        let effect: Option<Effect> = mouse(
            &mut session,
            MouseEventKind::Release(MouseButton::Left),
            0,
            0,
        );
        assert!(matches!(
            effect,
            Some(Effect::Request(request)) if matches!(
                request.params(),
                RequestParams::WritePane(params) if params.bytes() == b"\x1b[M#!!"
            )
        ));
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
    fn test_pressing_on_the_footer_opens_the_prompt() {
        let mut session: Session = browsing();

        mouse(
            &mut session,
            MouseEventKind::Press(MouseButton::Left),
            23,
            5,
        );
        let fabric: Fabric = session.render(Size::new(24, 80));

        assert!(fabric.text(23, 0..80).starts_with(':'));
    }

    #[test]
    fn test_nothing_is_gray_once_something_is_typed() {
        let mut session: Session = browsing();

        mouse(&mut session, MouseEventKind::Moved, 23, 5);
        press(&mut session, Key::Char('j'), KeyMods::NONE);
        let fabric: Fabric = session.render(Size::new(24, 80));

        assert_ne!(background(&fabric, 23, 0), Some(Color::Hover.into()));
    }

    #[test]
    fn test_the_footer_is_gray_while_the_mouse_is_over_it() {
        let mut session: Session = browsing();

        mouse(&mut session, MouseEventKind::Moved, 23, 5);
        let fabric: Fabric = session.render(Size::new(24, 80));

        assert_eq!(background(&fabric, 23, 0), Some(Color::Hover.into()));
    }

    #[test_case::test_case(1, 41, false; "the selected file of the browser which the keys go to")]
    #[test_case::test_case(0, 41, true; "the directory bar of the browser which the keys go to")]
    #[test_case::test_case(0, 1, true; "the directory bar of the browser which the keys do not go to")]
    #[test_case::test_case(5, 1, false; "a row with no file in it")]
    #[test_case::test_case(22, 1, false; "the footer of a pane")]
    fn test_what_the_mouse_is_over_is_gray(row: usize, column: usize, expected: bool) {
        let mut session: Session = browsing();
        let before: Fabric = session.render(Size::new(24, 80));

        mouse(&mut session, MouseEventKind::Moved, row, column);
        let fabric: Fabric = session.render(Size::new(24, 80));

        let grayed: bool = background(&before, row, column + 10) != Some(Color::Hover.into())
            && background(&fabric, row, column + 10) == Some(Color::Hover.into());
        assert_eq!(grayed, expected);
    }

    #[test]
    fn test_pressing_on_the_directory_bar_starts_typing_in_it() {
        let mut session: Session = browsing();

        mouse(
            &mut session,
            MouseEventKind::Press(MouseButton::Left),
            0,
            50,
        );
        let fabric: Fabric = session.render(Size::new(24, 80));

        // The directory bar is yellow once it is being typed in, and the file no longer is.
        assert_eq!(background(&fabric, 0, 50), Some(Color::Highlight.into()));
        assert_ne!(background(&fabric, 1, 42), Some(Color::Highlight.into()));
    }

    #[test]
    fn test_pressing_in_a_browser_which_the_keys_do_not_go_to_goes_to_it_and_selects() {
        let mut session: Session = browsing();

        let effect: Option<Effect> =
            mouse(&mut session, MouseEventKind::Press(MouseButton::Left), 1, 1);

        assert!(matches!(
            effect,
            Some(Effect::Request(request))
                if matches!(request.params(), RequestParams::SelectPane(params) if *params.pane() == pane(1))
        ));
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

//! Contains the [`Pane`] component, which is what one of the rectangles of a window shows.

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use crate::config::Config;

    use insh_api::PaneLayout;

    use typed_builder::TypedBuilder;

    /// The properties of a pane.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// Where it is and how big it is.
        pub layout: PaneLayout,
        /// The directory to browse in it.
        pub dir: PathBuf,
        /// The configuration.
        pub config: Config,
    }
}
pub use props::Props;

/// Contains the [`Event`] enum.
mod event {
    use insh_api::{PaneScreen, Response};
    use term::TermEvent;

    /// An event which a pane handles.
    pub enum Event {
        /// Something happened to the terminal.
        TermEvent(TermEvent),
        /// Whether what is typed goes to it.
        SetFocus {
            /// Whether it does.
            focused: bool,
        },
        /// The daemon said something which is not about the screen of a pane.
        Response(Response),
        /// The daemon said what has changed about the screen of the program in it.
        Screen(PaneScreen),
    }
}
pub use event::Event;

/// Contains the [`Effect`] enum.
mod effect {
    use std::path::PathBuf;

    use insh_api::Request;

    /// Something which a pane asks for.
    pub enum Effect {
        /// Ask the daemon for something.
        Request(Request),
        /// Ask the daemon for more than one thing, in the order they are in.
        Requests(Vec<Request>),
        /// Run the program in this pane.
        Run {
            /// The program to run.
            program: String,
            /// The arguments to give it.
            args: Vec<String>,
            /// The directory to run it in.
            dir: Option<PathBuf>,
        },
        /// Make the bell sound.
        Bell,
    }
}
pub use effect::Effect;

/// Contains the [`Pane`] component.
mod pane {
    use super::super::terminal::{Props as TerminalProps, Terminal};
    use super::{Effect, Event, Props, State};
    use crate::components::browser::{Browser, BrowserEffect, BrowserProps};
    use crate::components::chat::{Chat, ChatEffect, ChatProps};
    use crate::components::file_creator::{FileCreator, FileCreatorEffect, FileCreatorProps};
    use crate::components::finder::{Finder, FinderEffect, FinderProps};
    use crate::components::history::{
        History, HistoryEffect, HistorySearcher, HistorySearcherEffect,
    };
    use crate::components::searcher::{Searcher, SearcherEffect, SearcherProps};

    use std::path::{Path, PathBuf};

    use crate::config::Config;

    use file_type::FileType;

    use insh_api::{
        FileSortOptions, GetFilesRequestParams, PaneKind, PaneLayout, Request, RequestParams,
        ResponseParams, VisitDirRequestParams,
    };
    use rend::{Fabric, Location, Size};
    use terminal_emulator::Modes;
    use til::Component;

    /// What one of the rectangles of a window shows.
    ///
    /// A pane shows the browser until a program is run in it, and shows the browser again once
    /// that program has finished. Which of the two it is showing is whatever the daemon says the
    /// pane holds, since the daemon is what runs the programs.
    pub struct Pane {
        /// The state of the pane.
        state: State,
    }

    impl Component<Props, Event, Effect> for Pane {
        fn new(props: Props) -> Self {
            Self {
                state: State::from(props),
            }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            match event {
                Event::Screen(screen) => {
                    self.state.show_screen(screen);
                    None
                }
                Event::SetFocus { focused } => self.state.set_focus(focused),
                Event::Response(response) => self.state.respond(response),
                Event::TermEvent(term_event) => self.state.term_event(term_event),
            }
        }

        fn render(&self, size: Size) -> Fabric {
            self.state.render(size)
        }
    }

    impl Pane {
        /// Take note of where it is, how big it is, and what it holds now.
        pub fn relayout(&mut self, layout: PaneLayout) {
            self.state.relayout(layout);
        }

        /// Return what has to be asked of the daemon for it to have anything to show, and take it
        /// as having been asked.
        pub fn take_asked(&mut self) -> Vec<Request> {
            self.state.take_asked()
        }

        /// Return the ways the program in it has asked its terminal to behave, which are the ones
        /// a terminal uses by default when there is no program in it.
        pub fn modes(&self) -> Modes {
            self.state.modes()
        }

        /// Return whether what is typed in it goes to a program rather than to the browser.
        pub fn is_program(&self) -> bool {
            self.state.kind() == PaneKind::Program
        }

        /// Return where the cursor is on the screen of the whole client, which is nowhere at all
        /// when nothing in it is showing one.
        pub fn cursor_at(&self) -> Option<Location> {
            let at: Location = self.state.cursor()?;
            let corner: Location = self.state.layout().at();

            Some(
                Location::builder()
                    .row(corner.row + at.row)
                    .column(corner.column + at.column)
                    .build(),
            )
        }

        /// Return where in it looking back through it starts, which is nowhere in particular when
        /// it has neither a cursor nor a selected row.
        pub fn copy_start(&self) -> Option<Location> {
            self.state.copy_start()
        }

        /// Return whether a response is any business of what it is showing.
        #[cfg(test)]
        pub fn wants(&self, params: &ResponseParams) -> bool {
            self.state.wants(params)
        }
    }

    /// What a pane is showing.
    ///
    /// A pane shows one of insh's own components or the screen of a program, and which of the
    /// components it is changes as one of them asks for another, the same way the whole of insh
    /// used to change between them before there were panes.
    pub enum Contents {
        /// The browser.
        Browser(Box<Browser>),
        /// Making a new file.
        FileCreator(Box<FileCreator>),
        /// Finding files by name.
        Finder(Box<Finder>),
        /// Searching files for a phrase.
        Searcher(Box<Searcher>),
        /// Chatting about a directory.
        Chat(Box<Chat>),
        /// Looking through the past chats.
        History(Box<History>),
        /// Searching the past chats.
        HistorySearcher(Box<HistorySearcher>),
        /// The screen of a program running in the daemon.
        Terminal(Box<Terminal>),
    }

    impl Contents {
        /// Return whether a response is any business of what is being shown.
        ///
        /// Every pane is told about every response, and a chat's are of no interest to anything
        /// but the chats, which would otherwise take a reply in another pane for something gone
        /// wrong.
        pub fn wants(&self, params: &ResponseParams) -> bool {
            match params {
                ResponseParams::AiStatus(_)
                | ResponseParams::Chat(_)
                | ResponseParams::GetChat(_) => matches!(self, Self::Chat(_)),
                ResponseParams::DeleteChat(_) => {
                    matches!(self, Self::Chat(_) | Self::History(_))
                }
                ResponseParams::ListChats(_) => matches!(self, Self::History(_)),
                ResponseParams::SearchChats(_) => matches!(self, Self::HistorySearcher(_)),
                _ => true,
            }
        }

        /// Return what a pane of the given layout shows, along with what has to be asked of the
        /// daemon for it to have anything to show.
        ///
        /// A browser has nothing in it until the files of the directory it is showing have been
        /// asked for, so the request which asks for them is made here and the browser is told
        /// which one it is waiting on.
        pub fn of(layout: &PaneLayout, dir: &Path, config: &Config) -> (Self, Vec<Request>) {
            match layout.kind() {
                PaneKind::Component => {
                    Self::showing(&Showing::Browser(dir.to_path_buf()), layout, config)
                }
                PaneKind::Program => {
                    let props = TerminalProps::builder().layout(layout.clone()).build();
                    (Self::Terminal(Box::new(Terminal::new(props))), Vec::new())
                }
            }
        }

        /// Return the component which is being shown, along with what has to be asked of the
        /// daemon for it to have anything to show.
        pub fn showing(
            showing: &Showing,
            layout: &PaneLayout,
            config: &Config,
        ) -> (Self, Vec<Request>) {
            let size: Size = layout.size();

            match showing {
                Showing::FileCreator(dir, file_type) => {
                    let props = FileCreatorProps::builder()
                        .dir(dir.clone())
                        .file_type(*file_type)
                        .build();
                    (
                        Self::FileCreator(Box::new(FileCreator::new(props))),
                        Vec::new(),
                    )
                }
                Showing::Finder(dir) => {
                    let props = FinderProps::builder()
                        .dir(dir.clone())
                        .size(size)
                        .phrase(None)
                        .build();
                    (Self::Finder(Box::new(Finder::new(props))), Vec::new())
                }
                Showing::Searcher(dir) => {
                    let props = SearcherProps::new(config.clone(), dir.clone(), size, None, None);
                    (Self::Searcher(Box::new(Searcher::new(props))), Vec::new())
                }
                Showing::Chat(dir) => {
                    let props = ChatProps::builder().dir(dir.clone()).size(size).build();
                    // A chat asks whether there is anything to chat with before anything else.
                    (
                        Self::Chat(Box::new(Chat::new(props))),
                        vec![Chat::initial_request()],
                    )
                }
                Showing::Browser(dir) => Self::browser(dir, size, config),
            }
        }

        /// Return the browser showing the directory, along with the request which asks for the
        /// files in it.
        fn browser(dir: &Path, size: Size, config: &Config) -> (Self, Vec<Request>) {
            let files = Request::builder()
                .params(RequestParams::GetFiles(
                    GetFilesRequestParams::builder()
                        .dir(dir.to_path_buf())
                        .sort(config.browser().sort().map(FileSortOptions::from))
                        .metadata(config.browser().metadata())
                        .build(),
                ))
                .build();
            let visit = Request::builder()
                .params(RequestParams::VisitDir(
                    VisitDirRequestParams::builder()
                        .dir(dir.to_path_buf())
                        .build(),
                ))
                .build();

            let props = BrowserProps::builder()
                .config(config.clone())
                .dir(dir.to_path_buf())
                .size(size)
                .pending_request(Some(*files.uuid()))
                .build();

            (
                Self::Browser(Box::new(Browser::new(props))),
                vec![visit, files],
            )
        }
    }

    /// What one of the components a pane shows has asked the pane for.
    pub enum Asked {
        /// Show a different component.
        Show(Box<Showing>),
        /// Run a program in the pane.
        Run {
            /// The program to run.
            program: String,
            /// The arguments to give it.
            args: Vec<String>,
            /// The directory to run it in.
            dir: Option<PathBuf>,
        },
        /// Ask the daemon for something.
        Request(Request),
        /// Ask the daemon for more than one thing.
        Requests(Vec<Request>),
        /// Make the bell sound.
        Bell,
        /// Look through the past chats, leaving the chat where it is.
        OpenHistory,
        /// Search the past chats.
        SearchHistory,
        /// Go back to the chat which was left.
        BackToChat,
        /// Go back to the chat which was left and show a past one in it.
        OpenChat {
            /// Which chat to show.
            id: i64,
        },
        /// Go back to the chat which was left and start a new one in it.
        NewChat,
    }

    /// Which of insh's own components a pane is being asked to show, and what it is to show of it.
    pub enum Showing {
        /// The browser, showing the given directory.
        Browser(PathBuf),
        /// Making a new file of the given type in the given directory.
        FileCreator(PathBuf, FileType),
        /// Finding files by name in the given directory.
        Finder(PathBuf),
        /// Searching the files of the given directory for a phrase.
        Searcher(PathBuf),
        /// Chatting about the given directory.
        Chat(PathBuf),
    }

    impl Asked {
        /// Return what the browser effect asks the pane for.
        pub fn of_browser(effect: BrowserEffect) -> Option<Self> {
            match effect {
                BrowserEffect::RunBash { dir } => Some(Self::Run {
                    program: String::from("bash"),
                    args: Vec::new(),
                    dir: Some(dir),
                }),
                BrowserEffect::OpenVim(args) => Some(Self::Run {
                    program: String::from("vim"),
                    args: args.strings(),
                    dir: None,
                }),
                BrowserEffect::OpenFileCreator { dir, file_type } => {
                    Some(Self::Show(Box::new(Showing::FileCreator(dir, file_type))))
                }
                BrowserEffect::OpenFinder { dir } => {
                    Some(Self::Show(Box::new(Showing::Finder(dir))))
                }
                BrowserEffect::OpenSearcher { dir } => {
                    Some(Self::Show(Box::new(Showing::Searcher(dir))))
                }
                BrowserEffect::Request(request) => Some(Self::Request(request)),
                BrowserEffect::Requests(requests) => Some(Self::Requests(requests)),
                BrowserEffect::Bell => Some(Self::Bell),
                BrowserEffect::OpenChat { dir } => Some(Self::Show(Box::new(Showing::Chat(dir)))),
            }
        }

        /// Return what the finder effect asks the pane for.
        pub fn of_finder(effect: FinderEffect) -> Option<Self> {
            match effect {
                FinderEffect::OpenVim(args) => Some(Self::Run {
                    program: String::from("vim"),
                    args: args.strings(),
                    dir: None,
                }),
                FinderEffect::Browse { dir, .. } | FinderEffect::Quit { dir } => {
                    Some(Self::Show(Box::new(Showing::Browser(dir))))
                }
                FinderEffect::Request(request) => Some(Self::Request(request)),
                FinderEffect::Bell => Some(Self::Bell),
            }
        }

        /// Return what the searcher effect asks the pane for.
        pub fn of_searcher(effect: SearcherEffect) -> Option<Self> {
            match effect {
                SearcherEffect::OpenVim(args) => Some(Self::Run {
                    program: String::from("vim"),
                    args: args.strings(),
                    dir: None,
                }),
                SearcherEffect::Goto { dir, .. } | SearcherEffect::Quit { dir } => {
                    Some(Self::Show(Box::new(Showing::Browser(dir))))
                }
                SearcherEffect::Request(request) => Some(Self::Request(request)),
                SearcherEffect::Bell => Some(Self::Bell),
            }
        }

        /// Return what the chat effect asks the pane for.
        pub fn of_chat(effect: ChatEffect, dir: &Path) -> Option<Self> {
            match effect {
                ChatEffect::Request(request) => Some(Self::Request(request)),
                ChatEffect::Quit => Some(Self::Show(Box::new(Showing::Browser(dir.to_path_buf())))),
                ChatEffect::Bell => Some(Self::Bell),
                ChatEffect::OpenHistory => Some(Self::OpenHistory),
            }
        }

        /// Return what the past chats ask the pane for.
        pub fn of_history(effect: HistoryEffect) -> Option<Self> {
            match effect {
                HistoryEffect::Open { id } => Some(Self::OpenChat { id }),
                HistoryEffect::Request(request) => Some(Self::Request(request)),
                HistoryEffect::New => Some(Self::NewChat),
                HistoryEffect::Search => Some(Self::SearchHistory),
                HistoryEffect::Quit => Some(Self::BackToChat),
                HistoryEffect::Bell => Some(Self::Bell),
            }
        }

        /// Return what searching the past chats asks the pane for.
        pub fn of_history_searcher(effect: HistorySearcherEffect) -> Option<Self> {
            match effect {
                HistorySearcherEffect::Open { id } => Some(Self::OpenChat { id }),
                HistorySearcherEffect::Request(request) => Some(Self::Request(request)),
                HistorySearcherEffect::Quit => Some(Self::OpenHistory),
                HistorySearcherEffect::Bell => Some(Self::Bell),
            }
        }

        /// Return what the file creator effect asks the pane for.
        pub fn of_file_creator(effect: FileCreatorEffect, dir: &Path) -> Option<Self> {
            match effect {
                FileCreatorEffect::Browse { dir, .. } => {
                    Some(Self::Show(Box::new(Showing::Browser(dir))))
                }
                FileCreatorEffect::Quit => {
                    Some(Self::Show(Box::new(Showing::Browser(dir.to_path_buf()))))
                }
                FileCreatorEffect::Request(request) => Some(Self::Request(request)),
                FileCreatorEffect::Bell => Some(Self::Bell),
            }
        }
    }
}
pub use pane::Pane;

/// Contains the [`State`] struct.
mod state {
    use std::path::PathBuf;

    use super::pane::{Asked, Contents, Showing};
    use super::{Effect, Props};
    use crate::components::browser::{BrowserEffect, BrowserEvent};
    use crate::components::chat::{Chat, ChatEffect};
    use crate::components::common::PaneContentsEvent;
    use crate::components::file_creator::FileCreatorEvent;
    use crate::components::history::{History, HistorySearcher};
    use crate::config::Config;

    use insh_api::{PaneKind, PaneLayout, PaneScreen, Request, Response, ResponseParams};
    use rend::{Fabric, Location, Size};
    use term::TermEvent;
    use terminal_emulator::Modes;
    use til::Component;

    /// The state of a pane.
    pub struct State {
        /// Where it is and how big it is.
        layout: PaneLayout,
        /// What it is showing.
        contents: Contents,
        /// The directory the browser in it shows.
        dir: PathBuf,
        /// The configuration.
        config: Config,
        /// What has to be asked of the daemon for it to have anything to show, which is taken
        /// away once it has been asked.
        asked: Vec<Request>,
        /// Whether what is typed goes to it.
        focused: bool,
        /// The chat which was left to look through the past ones, which goes on being told about
        /// the reply it is waiting for.
        chat_aside: Option<Box<Chat>>,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let (contents, asked): (Contents, Vec<Request>) =
                Contents::of(&props.layout, &props.dir, &props.config);

            Self {
                layout: props.layout,
                contents,
                dir: props.dir,
                config: props.config,
                asked,
                // NOTE: This is what the components themselves start out as, so that the layout
                // which follows straight after this telling a pane that what is typed does not
                // go to it is a change which reaches them.
                focused: true,
                chat_aside: None,
            }
        }
    }

    impl State {
        /// Return where it is and how big it is.
        pub fn layout(&self) -> &PaneLayout {
            &self.layout
        }

        /// Return what has to be asked of the daemon for it to have anything to show, and take it
        /// as having been asked.
        pub fn take_asked(&mut self) -> Vec<Request> {
            std::mem::take(&mut self.asked)
        }

        /// Return what it holds.
        pub fn kind(&self) -> PaneKind {
            self.layout.kind()
        }

        /// Return the ways the program in it has asked its terminal to behave.
        pub fn modes(&self) -> Modes {
            match &self.contents {
                Contents::Terminal(terminal) => terminal.modes(),
                _ => Modes::default(),
            }
        }

        /// Return where the cursor is in it.
        pub fn cursor(&self) -> Option<Location> {
            match &self.contents {
                Contents::Terminal(terminal) => terminal.cursor(),
                // The components draw their own cursor as a colored row rather than putting the
                // one the terminal shows anywhere.
                _ => None,
            }
        }

        /// Return whether a response is any business of what it is showing.
        #[cfg(test)]
        pub fn wants(&self, params: &ResponseParams) -> bool {
            self.contents.wants(params)
        }

        /// Return where in it looking back through it starts.
        pub fn copy_start(&self) -> Option<Location> {
            match &self.contents {
                Contents::Browser(browser) => browser.copy_start(),
                Contents::FileCreator(file_creator) => file_creator.copy_start(),
                Contents::Finder(finder) => finder.copy_start(),
                Contents::Searcher(searcher) => searcher.copy_start(),
                Contents::Chat(chat) => chat.copy_start(),
                Contents::History(history) => history.copy_start(),
                Contents::HistorySearcher(searcher) => searcher.copy_start(),
                Contents::Terminal(terminal) => terminal.copy_start(),
            }
        }

        /// Take note of where it is, how big it is, and what it holds now.
        ///
        /// What it shows is built again when what it holds has changed, which is what makes a pane
        /// the browser again once the program in it has finished.
        pub fn relayout(&mut self, layout: PaneLayout) {
            let changed: bool = layout.kind() != self.layout.kind();
            let resized: bool = layout.size() != self.layout.size();
            self.layout = layout;

            if changed {
                let (contents, asked) = Contents::of(&self.layout, &self.dir, &self.config);
                self.contents = contents;
                self.asked = asked;
                self.chat_aside = None;
                self.tell_new_contents_about_focus();
                return;
            }

            if let Contents::Terminal(terminal) = &mut self.contents {
                terminal.relayout(self.layout.clone());
            }

            // NOTE: Changing size is something the components only take note of, so there is
            // nothing which comes of it to be done.
            if resized {
                self.term_event(TermEvent::Resize(self.layout.size()));
            }
        }

        /// Take note of whether what is typed goes to it.
        pub fn set_focus(&mut self, focused: bool) -> Option<Effect> {
            if focused == self.focused {
                return None;
            }
            self.focused = focused;
            self.tell_contents_about_focus()
        }

        /// Tell what it has only just started showing that what is typed does not go to it, which
        /// a component takes for granted that it does until it is told otherwise.
        fn tell_new_contents_about_focus(&mut self) {
            if !self.focused {
                self.tell_contents_about_focus();
            }
        }

        /// Tell what it is showing whether what is typed goes to it.
        fn tell_contents_about_focus(&mut self) -> Option<Effect> {
            let focused: bool = self.focused;

            let event = PaneContentsEvent::SetFocus { focused };
            match &mut self.contents {
                Contents::Browser(browser) => {
                    let effect: BrowserEffect =
                        browser.handle(BrowserEvent::SetFocus { focused })?;
                    let asked: Asked = Asked::of_browser(effect)?;
                    return self.asked(asked);
                }
                Contents::FileCreator(file_creator) => {
                    file_creator.handle(FileCreatorEvent::SetFocus { focused });
                }
                Contents::Finder(finder) => {
                    finder.handle(event);
                }
                Contents::Searcher(searcher) => {
                    searcher.handle(event);
                }
                Contents::Chat(chat) => {
                    chat.handle(event);
                }
                Contents::History(history) => {
                    history.handle(event);
                }
                Contents::HistorySearcher(searcher) => {
                    searcher.handle(event);
                }
                Contents::Terminal(_) => {}
            }

            None
        }

        /// Take note of what has changed about the screen of the program in it.
        pub fn show_screen(&mut self, screen: PaneScreen) {
            if let Contents::Terminal(terminal) = &mut self.contents {
                terminal.handle(screen);
            }
        }

        /// Take note of what the daemon said.
        pub fn respond(&mut self, response: Response) -> Option<Effect> {
            // A reply goes on arriving while the past chats are being looked through, and it
            // belongs to the chat rather than to whatever is being looked at.
            if matches!(
                response.params(),
                ResponseParams::AiStatus(_) | ResponseParams::Chat(_) | ResponseParams::GetChat(_)
            ) {
                if let Some(chat) = self.chat_aside.as_mut() {
                    return match chat.handle(PaneContentsEvent::Response(response))? {
                        ChatEffect::Request(request) => Some(Effect::Request(request)),
                        ChatEffect::Bell => Some(Effect::Bell),
                        // Where the chat goes next is for whoever is looking at it to say.
                        ChatEffect::OpenHistory | ChatEffect::Quit => None,
                    };
                }
            }

            // The chat which was left to look through the past ones may be the one which was
            // deleted, and the past chats want to know about it as well.
            if matches!(response.params(), ResponseParams::DeleteChat(_)) {
                if let Some(chat) = self.chat_aside.as_mut() {
                    chat.handle(PaneContentsEvent::Response(response.clone()));
                }
            }

            if !self.contents.wants(response.params()) {
                return None;
            }

            let asked: Asked = match &mut self.contents {
                Contents::Browser(browser) => {
                    Asked::of_browser(browser.handle(BrowserEvent::Response(response))?)?
                }
                Contents::FileCreator(file_creator) => Asked::of_file_creator(
                    file_creator.handle(FileCreatorEvent::Response(response))?,
                    &self.dir,
                )?,
                Contents::Finder(finder) => {
                    Asked::of_finder(finder.handle(PaneContentsEvent::Response(response))?)?
                }
                Contents::Searcher(searcher) => {
                    Asked::of_searcher(searcher.handle(PaneContentsEvent::Response(response))?)?
                }
                Contents::Chat(chat) => Asked::of_chat(
                    chat.handle(PaneContentsEvent::Response(response))?,
                    &self.dir,
                )?,
                Contents::History(history) => {
                    Asked::of_history(history.handle(PaneContentsEvent::Response(response))?)?
                }
                Contents::HistorySearcher(searcher) => Asked::of_history_searcher(
                    searcher.handle(PaneContentsEvent::Response(response))?,
                )?,
                Contents::Terminal(_) => return None,
            };

            self.asked(asked)
        }

        /// Do what the terminal event says.
        ///
        /// Nothing is done with it when there is a program in the pane, because what is typed goes
        /// to the program and the session is what sends it there.
        pub fn term_event(&mut self, term_event: TermEvent) -> Option<Effect> {
            let asked: Asked = match &mut self.contents {
                Contents::Browser(browser) => {
                    Asked::of_browser(browser.handle(BrowserEvent::TermEvent(term_event))?)?
                }
                Contents::FileCreator(file_creator) => Asked::of_file_creator(
                    file_creator.handle(FileCreatorEvent::TermEvent(term_event))?,
                    &self.dir,
                )?,
                Contents::Finder(finder) => {
                    Asked::of_finder(finder.handle(PaneContentsEvent::TermEvent(term_event))?)?
                }
                Contents::Searcher(searcher) => {
                    Asked::of_searcher(searcher.handle(PaneContentsEvent::TermEvent(term_event))?)?
                }
                Contents::Chat(chat) => Asked::of_chat(
                    chat.handle(PaneContentsEvent::TermEvent(term_event))?,
                    &self.dir,
                )?,
                Contents::History(history) => {
                    Asked::of_history(history.handle(PaneContentsEvent::TermEvent(term_event))?)?
                }
                Contents::HistorySearcher(searcher) => Asked::of_history_searcher(
                    searcher.handle(PaneContentsEvent::TermEvent(term_event))?,
                )?,
                Contents::Terminal(_) => return None,
            };

            self.asked(asked)
        }

        /// Do what one of the components a pane shows has asked the pane for.
        fn asked(&mut self, asked: Asked) -> Option<Effect> {
            match asked {
                Asked::Show(showing) => {
                    // The directory the pane is in is wherever it was last shown, so that going
                    // back to the browser from the finder shows where the finder was left.
                    self.dir = match showing.as_ref() {
                        Showing::Browser(dir)
                        | Showing::FileCreator(dir, _)
                        | Showing::Finder(dir)
                        | Showing::Searcher(dir)
                        | Showing::Chat(dir) => dir.clone(),
                    };

                    let (contents, asked) =
                        Contents::showing(showing.as_ref(), &self.layout, &self.config);
                    self.contents = contents;
                    self.tell_new_contents_about_focus();

                    match asked.is_empty() {
                        true => None,
                        false => Some(Effect::Requests(asked)),
                    }
                }
                Asked::Run { program, args, dir } => Some(Effect::Run { program, args, dir }),
                Asked::Request(request) => Some(Effect::Request(request)),
                Asked::Requests(requests) => Some(Effect::Requests(requests)),
                Asked::Bell => Some(Effect::Bell),
                Asked::OpenHistory => {
                    let mut history: History = History::new(self.layout.size());
                    let request: Request = history.list();
                    let left: Contents =
                        std::mem::replace(&mut self.contents, Contents::History(Box::new(history)));
                    if let Contents::Chat(chat) = left {
                        self.chat_aside = Some(chat);
                    }
                    self.tell_new_contents_about_focus();

                    Some(Effect::Request(request))
                }
                Asked::SearchHistory => {
                    let searcher: HistorySearcher = HistorySearcher::new(self.layout.size());
                    self.contents = Contents::HistorySearcher(Box::new(searcher));
                    self.tell_new_contents_about_focus();
                    None
                }
                Asked::BackToChat => {
                    self.back_to_chat();
                    None
                }
                Asked::OpenChat { id } => {
                    let request: Request = self.back_to_chat()?.open(id);
                    Some(Effect::Request(request))
                }
                Asked::NewChat => {
                    self.back_to_chat()?.start_new();
                    None
                }
            }
        }

        /// Show the chat which was left to look through the past ones again.
        fn back_to_chat(&mut self) -> Option<&mut Chat> {
            let mut chat: Box<Chat> = self.chat_aside.take()?;

            // NOTE: It is not told when the pane changes size while it is not being shown, and how
            // far back what was said can be read depends on how much of it fits.
            chat.handle(PaneContentsEvent::TermEvent(TermEvent::Resize(
                self.layout.size(),
            )));
            self.contents = Contents::Chat(chat);

            // NOTE: Nor is it told whether what is typed goes to the pane.
            self.tell_contents_about_focus();

            match &mut self.contents {
                Contents::Chat(chat) => Some(chat),
                _ => None,
            }
        }

        /// Draw what it is showing.
        pub fn render(&self, _size: Size) -> Fabric {
            let size: Size = self.layout.size();

            match &self.contents {
                Contents::Browser(browser) => browser.render(size),
                Contents::FileCreator(file_creator) => file_creator.render(size),
                Contents::Finder(finder) => finder.render(size),
                Contents::Searcher(searcher) => searcher.render(size),
                Contents::Chat(chat) => chat.render(size),
                Contents::History(history) => history.render(size),
                Contents::HistorySearcher(searcher) => searcher.render(size),
                Contents::Terminal(terminal) => terminal.render(size),
            }
        }
    }
}
use state::State;

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use super::{Effect, Event, Pane, Props};
    use crate::color::Color;
    use crate::config::Config;

    use file_info::{FileInfo, FileMetadata};
    use file_type::FileType;
    use insh_api::{
        AiStatusResponseParams, Chat, ChatMessage, ChatResponseParams, ChatRole,
        DeleteChatResponseParams, FindFilesResponseParams, GetChatResponseParams,
        GetFilesResponseParams, ListChatsResponseParams, PaneKind, PaneLayout, PaneScreen, Request,
        RequestParams, Response, ResponseParams, SearchPhraseResponseParams,
    };
    use path_finder::Entry as PathEntry;
    use phrase_searcher::{FileHit, LineHit};
    use rend::{Fabric, Location, Size};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use terminal_emulator::Modes;
    use til::Component;

    use test_case::test_case;
    use uuid::Uuid;

    /// Return the layout of a pane of the given size.
    fn layout(rows: usize, columns: usize) -> PaneLayout {
        PaneLayout::builder()
            .pane(Uuid::from_u128(1))
            .at(Location::default())
            .size(Size::new(rows, columns))
            .build()
    }

    /// Return the given number of files, which have metadata when it is given.
    fn files(count: usize, metadata: Option<FileMetadata>) -> Vec<FileInfo> {
        (0..count)
            .map(|file| {
                FileInfo::builder()
                    .path(PathBuf::from(format!("/tmp/file{:02}", file)))
                    .r#type(Ok(FileType::File))
                    .metadata(metadata.clone())
                    .build()
            })
            .collect()
    }

    /// Return some metadata for a file.
    fn metadata() -> FileMetadata {
        FileMetadata::builder()
            .mode(0o100664)
            .hard_links(1)
            .uid(1000)
            .user(Some(String::from("austin")))
            .gid(1000)
            .group(Some(String::from("austin")))
            .size(123)
            .modified(0)
            .build()
    }

    /// Answer the request with the given files.
    fn answer(pane: &mut Pane, request: &Request, files: Vec<FileInfo>) {
        pane.handle(Event::Response(
            Response::builder()
                .uuid(*request.uuid())
                .params(ResponseParams::GetFiles(
                    GetFilesResponseParams::builder().result(Ok(files)).build(),
                ))
                .build(),
        ));
    }

    /// Return a pane of the given size with a browser showing the given files.
    fn browsing(rows: usize, columns: usize, files: Vec<FileInfo>) -> Pane {
        let mut pane: Pane = Pane::new(
            Props::builder()
                .layout(layout(rows, columns))
                .dir(PathBuf::from("/tmp"))
                .config(Config::default())
                .build(),
        );
        let asked: Vec<Request> = pane.take_asked();
        answer(&mut pane, asked.last().unwrap(), files);
        pane
    }

    /// Press the key in the pane.
    fn press(pane: &mut Pane, character: char) -> Option<Effect> {
        pane.handle(Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
            key: Key::Char(character),
            mods: KeyMods::NONE,
        })))
    }

    /// Return everything which is drawn in the pane.
    fn drawn(pane: &Pane) -> String {
        let fabric: Fabric = pane.render(Size::new(0, 0));
        (0..fabric.size().rows)
            .map(|row| fabric.text(row, 0..fabric.size().columns))
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// Return a pane with a chat in it which has said something, along with the request for the
    /// reply.
    fn chatting() -> (Pane, Request) {
        let mut pane: Pane = browsing(20, 60, files(1, None));

        let requests: Vec<Request> = match press(&mut pane, 'a') {
            Some(Effect::Requests(requests)) => requests,
            _ => panic!("Expected the chat to ask whether there is anything to chat with."),
        };
        pane.handle(Event::Response(
            Response::builder()
                .uuid(*requests[0].uuid())
                .params(ResponseParams::AiStatus(
                    AiStatusResponseParams::builder().configured(true).build(),
                ))
                .build(),
        ));

        press(&mut pane, 'h');
        press(&mut pane, 'i');
        let request: Request = match pane.handle(Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
            key: Key::CarriageReturn,
            mods: KeyMods::NONE,
        }))) {
            Some(Effect::Request(request)) => request,
            _ => panic!("Expected a reply to be asked for."),
        };

        (pane, request)
    }

    /// Press the key with the modifiers in the pane.
    fn press_with(pane: &mut Pane, key: Key, mods: KeyMods) -> Option<Effect> {
        pane.handle(Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
            key,
            mods,
        })))
    }

    /// Answer the request with some past chats.
    fn answer_with_chats(pane: &mut Pane, request: &Request) {
        pane.handle(Event::Response(
            Response::builder()
                .uuid(*request.uuid())
                .params(ResponseParams::ListChats(
                    ListChatsResponseParams::builder()
                        .chats(vec![Chat::builder()
                            .id(7)
                            .title("a past chat")
                            .dir("/tmp")
                            .build()])
                        .build(),
                ))
                .build(),
        ));
    }

    /// Return the pane looking through the past chats, having left a chat to do it, along with
    /// the request for the reply which the chat is waiting for.
    fn looking_through_chats() -> (Pane, Request) {
        let (mut pane, reply): (Pane, Request) = chatting();

        let list: Request = match press_with(&mut pane, Key::Char('h'), KeyMods::CONTROL) {
            Some(Effect::Request(request)) => request,
            _ => panic!("Expected the past chats to be asked for."),
        };
        answer_with_chats(&mut pane, &list);

        (pane, reply)
    }

    #[test]
    fn test_the_past_chats_can_be_looked_through_in_a_pane() {
        let (pane, _): (Pane, Request) = looking_through_chats();

        assert!(drawn(&pane).contains("a past chat"));
    }

    #[test]
    fn test_a_past_chat_can_be_opened_in_a_pane() {
        let (mut pane, _): (Pane, Request) = looking_through_chats();

        let request: Request = match press_with(&mut pane, Key::CarriageReturn, KeyMods::NONE) {
            Some(Effect::Request(request)) => request,
            _ => panic!("Expected what was said in the chat to be asked for."),
        };
        pane.handle(Event::Response(
            Response::builder()
                .uuid(*request.uuid())
                .params(ResponseParams::GetChat(
                    GetChatResponseParams::builder()
                        .messages(vec![ChatMessage::builder()
                            .role(ChatRole::Assistant)
                            .content("said back then")
                            .build()])
                        .build(),
                ))
                .build(),
        ));

        assert!(drawn(&pane).contains("said back then"));
    }

    #[test]
    fn test_a_past_chat_which_another_pane_asked_for_is_not_shown() {
        let (mut pane, _): (Pane, Request) = chatting();

        pane.handle(Event::Response(
            Response::builder()
                .uuid(Uuid::from_u128(999))
                .params(ResponseParams::GetChat(
                    GetChatResponseParams::builder()
                        .messages(vec![ChatMessage::builder()
                            .role(ChatRole::Assistant)
                            .content("not for here")
                            .build()])
                        .build(),
                ))
                .build(),
        ));

        assert!(!drawn(&pane).contains("not for here"));
    }

    #[test]
    fn test_a_reply_which_arrives_while_looking_through_the_past_chats_is_kept() {
        let (mut pane, reply): (Pane, Request) = looking_through_chats();

        pane.handle(Event::Response(
            Response::builder()
                .uuid(*reply.uuid())
                .params(ResponseParams::Chat(
                    ChatResponseParams::builder()
                        .chat_id(Some(1))
                        .delta("the late reply")
                        .build(),
                ))
                .last(true)
                .build(),
        ));
        assert!(!drawn(&pane).contains("the late reply"));

        press_with(&mut pane, Key::Char('q'), KeyMods::CONTROL);

        assert!(drawn(&pane).contains("the late reply"));
    }

    /// Say that the chat of the given number was deleted.
    fn deleted(pane: &mut Pane, request: Uuid, chat_id: i64) -> Option<Effect> {
        pane.handle(Event::Response(
            Response::builder()
                .uuid(request)
                .params(ResponseParams::DeleteChat(
                    DeleteChatResponseParams::builder()
                        .chat_id(chat_id)
                        .deleted(true)
                        .build(),
                ))
                .build(),
        ))
    }

    /// Finish the reply, which puts the chat it is in in the chat which is being had.
    fn replied(pane: &mut Pane, request: &Request, chat_id: i64) {
        pane.handle(Event::Response(
            Response::builder()
                .uuid(*request.uuid())
                .params(ResponseParams::Chat(
                    ChatResponseParams::builder()
                        .chat_id(Some(chat_id))
                        .delta("the reply")
                        .build(),
                ))
                .last(true)
                .build(),
        ));
    }

    /// Say something in the pane, and return which chat it says it is going to.
    fn said_in(pane: &mut Pane) -> Option<i64> {
        press(pane, 'x');
        match press_with(pane, Key::CarriageReturn, KeyMods::NONE) {
            Some(Effect::Request(request)) => match request.params() {
                RequestParams::Chat(params) => params.chat_id(),
                _ => panic!("Expected a reply to be asked for."),
            },
            _ => panic!("Expected a reply to be asked for."),
        }
    }

    #[test_case(7, None; "the chat which is being had")]
    #[test_case(8, Some(7); "some other chat")]
    fn test_a_chat_is_forgotten_when_it_is_deleted_from_the_past_chats(
        deleted_chat: i64,
        expected: Option<i64>,
    ) {
        let (mut pane, reply): (Pane, Request) = chatting();
        replied(&mut pane, &reply, 7);
        assert!(drawn(&pane).contains("the reply"));
        press_with(&mut pane, Key::Char('h'), KeyMods::CONTROL);

        deleted(&mut pane, Uuid::from_u128(999), deleted_chat);
        press_with(&mut pane, Key::Char('q'), KeyMods::CONTROL);

        assert_eq!(drawn(&pane).contains("the reply"), expected.is_some());
        assert_eq!(said_in(&mut pane), expected);
    }

    #[test]
    fn test_a_chat_is_forgotten_when_another_pane_deletes_it() {
        let (mut pane, reply): (Pane, Request) = chatting();
        replied(&mut pane, &reply, 7);

        deleted(&mut pane, Uuid::from_u128(999), 7);

        assert_eq!(said_in(&mut pane), None);
    }

    #[test]
    fn test_only_the_chats_are_told_about_the_responses_of_a_chat() {
        let reply: ResponseParams =
            ResponseParams::Chat(ChatResponseParams::builder().delta("a reply").build());

        let mut pane: Pane = browsing(20, 60, files(1, None));
        press(&mut pane, 'f');
        assert!(!pane.wants(&reply));

        let (pane, _): (Pane, Request) = chatting();
        assert!(pane.wants(&reply));
    }

    #[test]
    fn test_the_past_chats_can_be_searched_and_left_in_a_pane() {
        let (mut pane, _): (Pane, Request) = looking_through_chats();

        press(&mut pane, '/');
        assert!(drawn(&pane).contains("inshie history searcher"));

        let list: Request = match press_with(&mut pane, Key::Char('q'), KeyMods::CONTROL) {
            Some(Effect::Request(request)) => request,
            _ => panic!("Expected the past chats to be asked for again."),
        };
        answer_with_chats(&mut pane, &list);

        assert!(drawn(&pane).contains("a past chat"));
    }

    /// Return whether anything in the pane is the color of what has focus.
    fn any_yellow(pane: &Pane) -> bool {
        let fabric: Fabric = pane.render(Size::new(0, 0));
        (0..fabric.size().rows).any(|row| {
            (0..fabric.size().columns).any(|column| {
                fabric.spot(row, column).style().background() == Some(Color::Highlight.into())
            })
        })
    }

    #[test_case(None, ""; "the browser")]
    #[test_case(Some('f'), "abc"; "the finder")]
    #[test_case(Some('s'), "abc"; "the searcher")]
    #[test_case(Some('c'), "abc"; "the file creator")]
    #[test_case(Some('a'), "abc"; "a chat")]
    fn test_nothing_is_yellow_in_a_pane_what_is_typed_does_not_go_to(
        open: Option<char>,
        typed: &str,
    ) {
        let mut pane: Pane = browsing(20, 60, files(1, None));
        if let Some(open) = open {
            press(&mut pane, open);
        }
        type_in(&mut pane, typed);
        assert!(
            any_yellow(&pane),
            "something should be yellow to begin with"
        );

        pane.handle(Event::SetFocus { focused: false });
        assert!(!any_yellow(&pane));

        pane.handle(Event::SetFocus { focused: true });
        assert!(any_yellow(&pane));
    }

    #[test]
    fn test_nothing_is_yellow_in_the_past_chats_of_a_pane_what_is_typed_does_not_go_to() {
        let (mut pane, _): (Pane, Request) = looking_through_chats();
        assert!(any_yellow(&pane));

        pane.handle(Event::SetFocus { focused: false });
        assert!(!any_yellow(&pane));

        pane.handle(Event::SetFocus { focused: true });
        press(&mut pane, '/');
        assert!(any_yellow(&pane));

        pane.handle(Event::SetFocus { focused: false });
        assert!(!any_yellow(&pane));
    }

    /// Type the characters in the pane.
    fn type_in(pane: &mut Pane, characters: &str) {
        for character in characters.chars() {
            press(pane, character);
        }
    }

    /// Return where in the pane looking back through it starts.
    fn copy_start(pane: &Pane) -> (usize, usize) {
        let start: Location = pane.copy_start().unwrap();
        (start.row, start.column)
    }

    #[test]
    fn test_copy_mode_starts_at_the_selected_row_of_the_past_chats() {
        let mut pane: Pane = browsing(3, 60, files(1, None));
        let reply: Option<Effect> = press(&mut pane, 'a');
        assert!(reply.is_some());
        let list: Request = match press_with(&mut pane, Key::Char('h'), KeyMods::CONTROL) {
            Some(Effect::Request(request)) => request,
            _ => panic!("Expected the past chats to be asked for."),
        };
        let chats: Vec<Chat> = (0..3)
            .map(|id| Chat::builder().id(id).title("a chat").dir("/tmp").build())
            .collect();
        pane.handle(Event::Response(
            Response::builder()
                .uuid(*list.uuid())
                .params(ResponseParams::ListChats(
                    ListChatsResponseParams::builder().chats(chats).build(),
                ))
                .build(),
        ));
        assert_eq!(copy_start(&pane), (0, 0));

        // There are only two rows for the chats, so going to the last of them scrolls the list.
        press(&mut pane, 'j');
        press(&mut pane, 'j');

        assert_eq!(copy_start(&pane), (1, 0));
    }

    #[test]
    fn test_copy_mode_starts_where_the_search_of_the_past_chats_is_typed() {
        let (mut pane, _): (Pane, Request) = looking_through_chats();
        press(&mut pane, '/');
        type_in(&mut pane, "abc");

        assert_eq!(copy_start(&pane), (0, 3));
    }

    #[test]
    fn test_copy_mode_starts_where_a_chat_is_typed() {
        let (mut pane, _): (Pane, Request) = chatting();
        type_in(&mut pane, "abc");

        // The footer is on the last row, and the box is just above it.
        assert_eq!(copy_start(&pane), (18, 3));
    }

    #[test_case('f'; "the finder")]
    #[test_case('s'; "the searcher")]
    fn test_copy_mode_starts_where_what_to_look_for_is_typed(open: char) {
        let mut pane: Pane = browsing(20, 60, files(1, None));
        press(&mut pane, open);
        type_in(&mut pane, "abc");

        assert_eq!(copy_start(&pane), (1, 3));
    }

    #[test_case('f'; "the finder")]
    #[test_case('s'; "the searcher")]
    fn test_copy_mode_starts_where_the_directory_is_typed(open: char) {
        let mut pane: Pane = browsing(20, 60, files(1, None));
        press(&mut pane, open);
        press_with(&mut pane, Key::Char('d'), KeyMods::CONTROL);

        let (row, column): (usize, usize) = copy_start(&pane);

        assert_eq!(row, 0);
        assert!(column > 0);
    }

    #[test]
    fn test_copy_mode_starts_where_the_directory_of_the_browser_is_typed() {
        let mut pane: Pane = browsing(20, 60, files(1, None));
        press_with(&mut pane, Key::Char('d'), KeyMods::CONTROL);

        let (row, column): (usize, usize) = copy_start(&pane);

        assert_eq!(row, 0);
        assert!(column > 0);
    }

    #[test]
    fn test_copy_mode_starts_where_a_new_file_is_named() {
        let mut pane: Pane = browsing(20, 60, files(1, None));
        press(&mut pane, 'c');
        type_in(&mut pane, "abc");

        assert_eq!(copy_start(&pane), (1, 3));
    }

    #[test]
    fn test_copy_mode_starts_where_the_cursor_of_a_program_last_was() {
        let id: Uuid = Uuid::from_u128(1);
        let mut pane: Pane = Pane::new(
            Props::builder()
                .layout(
                    PaneLayout::builder()
                        .pane(id)
                        .at(Location::default())
                        .size(Size::new(20, 60))
                        .kind(PaneKind::Program)
                        .build(),
                )
                .dir(PathBuf::from("/tmp"))
                .config(Config::default())
                .build(),
        );

        for cursor in [Some(Location::builder().row(4).column(7).build()), None] {
            pane.handle(Event::Screen(
                PaneScreen::builder()
                    .pane(id)
                    .rows(Vec::new())
                    .cursor(cursor)
                    .modes(Modes::default())
                    .build(),
            ));
        }

        assert_eq!(copy_start(&pane), (4, 7));
    }

    /// Return the row of the pane which has the color of a selected row on it.
    fn highlighted_row(pane: &Pane) -> usize {
        let fabric: Fabric = pane.render(Size::new(0, 0));

        (0..fabric.size().rows)
            .find(|row| {
                fabric.spot(*row, 0).style().background() == Some(Color::Highlight.into())
                    && fabric.spot(*row, 1).style().background() == Some(Color::Highlight.into())
                    && !fabric
                        .text(*row, 0..fabric.size().columns)
                        .contains("finder")
            })
            .expect("Expected a row to be selected.")
    }

    #[test]
    fn test_copy_mode_starts_at_the_selected_row_of_the_finder() {
        let mut pane: Pane = browsing(20, 60, files(1, None));
        press(&mut pane, 'f');
        press(&mut pane, 'x');
        let request: Request = match press_with(&mut pane, Key::CarriageReturn, KeyMods::NONE) {
            Some(Effect::Request(request)) => request,
            _ => panic!("Expected the files to be asked for."),
        };
        let entries: Vec<PathEntry> =
            serde_yaml_ng::from_str("[{path: /tmp/aaa}, {path: /tmp/bbb}, {path: /tmp/ccc}]")
                .unwrap();
        pane.handle(Event::Response(
            Response::builder()
                .uuid(*request.uuid())
                .params(ResponseParams::FindFiles(
                    FindFilesResponseParams::builder()
                        .entries(entries)
                        .searched(3)
                        .duration(Duration::ZERO)
                        .build(),
                ))
                .last(true)
                .build(),
        ));
        press(&mut pane, 'j');

        let start: Location = pane.copy_start().unwrap();

        assert_eq!(start.column, 0);
        assert_eq!(start.row, highlighted_row(&pane));
    }

    #[test_case(0; "the path of the first file")]
    #[test_case(1; "the first line of the first file")]
    #[test_case(2; "the second line of the first file")]
    #[test_case(3; "the path of the second file")]
    #[test_case(4; "the line of the second file")]
    fn test_copy_mode_starts_at_the_selected_row_of_the_searcher(downs: usize) {
        let mut pane: Pane = browsing(20, 60, files(1, None));
        press(&mut pane, 's');
        press(&mut pane, 'x');
        let request: Request = match press_with(&mut pane, Key::CarriageReturn, KeyMods::NONE) {
            Some(Effect::Request(request)) => request,
            _ => panic!("Expected the hits to be asked for."),
        };
        pane.handle(Event::Response(
            Response::builder()
                .uuid(*request.uuid())
                .params(ResponseParams::SearchPhrase(
                    SearchPhraseResponseParams::builder()
                        .hits(vec![
                            FileHit::new(
                                Path::new("/tmp/a"),
                                vec![LineHit::new(1, "x"), LineHit::new(2, "x")],
                            ),
                            FileHit::new(Path::new("/tmp/b"), vec![LineHit::new(1, "x")]),
                        ])
                        .searched(2)
                        .duration(Duration::ZERO)
                        .build(),
                ))
                .last(true)
                .build(),
        ));
        for _ in 0..downs {
            press(&mut pane, 'j');
        }

        let start: Location = pane.copy_start().unwrap();

        assert_eq!(start.column, 0);
        assert_eq!(start.row, highlighted_row(&pane));
    }

    #[test_case("the reply", None, "the reply"; "a reply")]
    #[test_case("", Some("Failed to request a reply"), "Failed to request a reply"; "a failure")]
    fn test_what_comes_back_from_a_chat_is_shown(delta: &str, error: Option<&str>, expected: &str) {
        let (mut pane, request): (Pane, Request) = chatting();

        pane.handle(Event::Response(
            Response::builder()
                .uuid(*request.uuid())
                .params(ResponseParams::Chat(
                    ChatResponseParams::builder()
                        .chat_id(Some(1))
                        .delta(delta)
                        .error(error.map(ToString::to_string))
                        .build(),
                ))
                .last(true)
                .build(),
        ));

        assert!(drawn(&pane).contains(expected));
    }

    #[test]
    fn test_the_selected_row_stays_visible_after_the_pane_gets_smaller() {
        let mut pane: Pane = browsing(20, 40, files(20, None));

        pane.relayout(layout(6, 40));
        for _ in 0..9 {
            press(&mut pane, 'j');
        }

        assert!(drawn(&pane).contains("file09"));
    }

    #[test]
    fn test_the_metadata_is_not_shown_as_unknown_while_it_is_on_its_way() {
        let mut pane: Pane = browsing(10, 80, files(3, None));

        let request: Request = match press(&mut pane, 'm') {
            Some(Effect::Request(request)) => request,
            _ => panic!("Expected the metadata to be asked for."),
        };
        assert!(!drawn(&pane).contains('?'));

        answer(&mut pane, &request, files(3, Some(metadata())));
        assert!(drawn(&pane).contains("-rw-rw-r--"));
    }
}

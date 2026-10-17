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
    use crate::components::searcher::{Searcher, SearcherEffect, SearcherProps};

    use std::path::{Path, PathBuf};

    use crate::config::Config;

    use file_type::FileType;

    use insh_api::{
        FileSortOptions, GetFilesRequestParams, PaneKind, PaneLayout, Request, RequestParams,
        VisitDirRequestParams,
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
        /// The screen of a program running in the daemon.
        Terminal(Box<Terminal>),
    }

    impl Contents {
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
                // NOTE: The past chats are not something a pane shows yet, so asking for them
                // rings the bell rather than doing nothing at all.
                ChatEffect::OpenHistory => Some(Self::Bell),
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
    use crate::components::file_creator::FileCreatorEvent;
    use crate::config::Config;

    use insh_api::{PaneKind, PaneLayout, PaneScreen, Request, Response};
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
                // NOTE: A pane starts out as the one what is typed goes to only if the daemon says
                // so, which it does with the layout which follows straight after this.
                focused: false,
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

        /// Take note of where it is, how big it is, and what it holds now.
        ///
        /// What it shows is built again when what it holds has changed, which is what makes a pane
        /// the browser again once the program in it has finished.
        pub fn relayout(&mut self, layout: PaneLayout) {
            let changed: bool = layout.kind() != self.layout.kind();
            self.layout = layout;

            if changed {
                let (contents, asked) = Contents::of(&self.layout, &self.dir, &self.config);
                self.contents = contents;
                self.asked = asked;
                return;
            }

            if let Contents::Terminal(terminal) = &mut self.contents {
                terminal.relayout(self.layout.clone());
            }
        }

        /// Take note of whether what is typed goes to it.
        pub fn set_focus(&mut self, focused: bool) -> Option<Effect> {
            if focused == self.focused {
                return None;
            }
            self.focused = focused;

            // NOTE: Only the browser is told, because it is the only one of the components which
            // shows a row as selected whether what is typed is going to it or not.
            let browser = match &mut self.contents {
                Contents::Browser(browser) => browser,
                _ => return None,
            };

            let effect: BrowserEffect = browser.handle(BrowserEvent::SetFocus { focused })?;
            let asked: Asked = Asked::of_browser(effect)?;
            self.asked(asked)
        }

        /// Take note of what has changed about the screen of the program in it.
        pub fn show_screen(&mut self, screen: PaneScreen) {
            if let Contents::Terminal(terminal) = &mut self.contents {
                terminal.handle(screen);
            }
        }

        /// Take note of what the daemon said.
        pub fn respond(&mut self, response: Response) -> Option<Effect> {
            let asked: Asked = match &mut self.contents {
                Contents::Browser(browser) => {
                    Asked::of_browser(browser.handle(BrowserEvent::Response(response))?)?
                }
                Contents::FileCreator(file_creator) => Asked::of_file_creator(
                    file_creator.handle(FileCreatorEvent::Response(response))?,
                    &self.dir,
                )?,
                Contents::Finder(finder) => {
                    Asked::of_finder(finder.handle(til::Event::Response(response))?)?
                }
                Contents::Searcher(searcher) => {
                    Asked::of_searcher(searcher.handle(til::Event::Response(response))?)?
                }
                Contents::Chat(chat) => {
                    Asked::of_chat(chat.handle(til::Event::Response(response))?, &self.dir)?
                }
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
                    Asked::of_finder(finder.handle(til::Event::TermEvent(term_event))?)?
                }
                Contents::Searcher(searcher) => {
                    Asked::of_searcher(searcher.handle(til::Event::TermEvent(term_event))?)?
                }
                Contents::Chat(chat) => {
                    Asked::of_chat(chat.handle(til::Event::TermEvent(term_event))?, &self.dir)?
                }
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

                    match asked.is_empty() {
                        true => None,
                        false => Some(Effect::Requests(asked)),
                    }
                }
                Asked::Run { program, args, dir } => Some(Effect::Run { program, args, dir }),
                Asked::Request(request) => Some(Effect::Request(request)),
                Asked::Requests(requests) => Some(Effect::Requests(requests)),
                Asked::Bell => Some(Effect::Bell),
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
                Contents::Terminal(terminal) => terminal.render(size),
            }
        }
    }
}
use state::State;

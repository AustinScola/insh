//! Contains the [`Insh`] component.

use std::path::PathBuf;

use crate::components::browser::{Browser, BrowserEffect, BrowserEvent, BrowserProps};
use crate::components::file_creator::{
    FileCreator, FileCreatorEffect, FileCreatorEvent, FileCreatorProps,
};
use crate::components::finder::{Finder, FinderEffect, FinderProps};
use crate::components::searcher::{Searcher, SearcherEffect, SearcherProps};
use crate::config::Config;
use crate::current_dir;
use crate::programs::{Bash, Vim};
use crate::stateful::Stateful;

use file_type::FileType;
use insh_api::{
    FileSortOptions, GetFilesRequestParams, Request, RequestParams, Response, ResponseParams,
    VisitDirRequestParams,
};
use rend::{Fabric, Size};
use term::{Key, KeyEvent, KeyMods, Term, TermEvent};
use til::{Component, Event, SystemEffect};

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use crate::args::Command;
    use crate::config::Config;

    use typed_builder::TypedBuilder;
    use uuid::Uuid;

    /// The properties of insh.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// What to start in.
        start: Start,
        /// The directory to start in.
        dir: Option<PathBuf>,
        /// The pending request for the files.
        #[builder(default)]
        pending_browser_request: Option<Uuid>,
        /// The pending request for the hits.
        #[builder(default)]
        pending_search_request: Option<Uuid>,
        /// The configuration.
        config: Config,
    }

    impl Props {
        /// Return what to start in.
        pub fn start(&self) -> &Start {
            &self.start
        }

        /// Return the directory to start in.
        pub fn dir(&self) -> &Option<PathBuf> {
            &self.dir
        }

        /// Return the pending request for the files.
        pub fn pending_browser_request(&self) -> &Option<Uuid> {
            &self.pending_browser_request
        }

        /// Return the pending request for the hits.
        pub fn pending_search_request(&self) -> &Option<Uuid> {
            &self.pending_search_request
        }

        /// Return the configuration.
        pub fn config(&self) -> &Config {
            &self.config
        }
    }

    /// What insh starts in.
    pub enum Start {
        /// The browser.
        Browser,
        /// The finder.
        Finder {
            /// The pattern to start with.
            phrase: Option<String>,
        },
        /// The searcher.
        Searcher {
            /// The phrase to start with.
            phrase: Option<String>,
        },
        /// Nothing, because a program is being run instead.
        Nothing,
    }

    impl From<Option<Command>> for Start {
        fn from(command: Option<Command>) -> Self {
            match command {
                Some(Command::Browse) | None => Start::Browser,
                Some(Command::Search { phrase }) => Start::Searcher { phrase },
                Some(Command::Find { phrase }) => Start::Finder { phrase },
                Some(Command::Edit { browse, .. }) => match browse {
                    true => Start::Browser,
                    false => Start::Nothing,
                },
            }
        }
    }
}
pub use props::{Props, Start};

/// The root component.
pub struct Insh {
    /// The state of insh.
    state: State,
}

impl Component<Props, Event<Response>, SystemEffect<Request>> for Insh {
    fn new(props: Props) -> Self {
        Self {
            state: State::from(props),
        }
    }

    fn handle(&mut self, event: Event<Response>) -> Option<SystemEffect<Request>> {
        if let Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
            key: Key::Char('x'),
            mods: KeyMods::CONTROL,
        })) = event
        {
            return Some(SystemEffect::Exit);
        }

        // There is nothing for a mode to do with the response to a note that a directory was gone
        // to, so it is dropped here rather than in every one of them.
        if let Event::Response(response) = &event {
            if matches!(response.params(), ResponseParams::VisitDir(_)) {
                return None;
            }
        }

        let mut action: Option<Action> = None;

        match self.state.mode {
            Mode::Browse => {
                let event: BrowserEvent = match event {
                    Event::TermEvent(term_event) => BrowserEvent::TermEvent(term_event),
                    Event::Response(response) => BrowserEvent::Response(response),
                };

                let browser = self.state.browser.as_mut().unwrap();
                let browser_effect: Option<BrowserEffect> = browser.handle(event);
                match browser_effect {
                    Some(BrowserEffect::OpenFileCreator { dir, file_type }) => {
                        action = Some(Action::CreateFile { dir, file_type });
                    }
                    Some(BrowserEffect::OpenFinder { dir }) => {
                        action = Some(Action::Find { dir });
                    }
                    Some(BrowserEffect::OpenSearcher { dir }) => {
                        action = Some(Action::Search { dir });
                    }
                    Some(BrowserEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(BrowserEffect::RunBash { dir }) => {
                        let program = Box::new(Bash::new(dir));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(BrowserEffect::Bell) => {
                        action = Some(Action::Bell);
                    }
                    Some(BrowserEffect::Request(request)) => {
                        return Some(SystemEffect::Request(request));
                    }
                    Some(BrowserEffect::Requests(requests)) => {
                        return Some(SystemEffect::Requests(requests));
                    }
                    None => {}
                }
            }
            Mode::FileCreator => {
                let file_creator_event: FileCreatorEvent = match event {
                    Event::TermEvent(term_event) => FileCreatorEvent::TermEvent(term_event),
                    Event::Response(response) => FileCreatorEvent::Response(response),
                };

                let file_creator = self.state.file_creator.as_mut().unwrap();
                let file_creator_effect: Option<FileCreatorEffect> =
                    file_creator.handle(file_creator_event);
                match file_creator_effect {
                    Some(FileCreatorEffect::Request(request)) => {
                        return Some(SystemEffect::Request(request));
                    }
                    Some(FileCreatorEffect::Browse { dir, file }) => {
                        action = Some(Action::Browse { dir, file });
                    }
                    Some(FileCreatorEffect::Bell) => {
                        action = Some(Action::Bell);
                    }
                    Some(FileCreatorEffect::Quit) => {
                        action = Some(Action::QuitFinder);
                    }
                    None => {}
                }
            }
            Mode::Finder => {
                let finder = self.state.finder.as_mut().unwrap();
                let finder_effect: Option<FinderEffect> = finder.handle(event);
                match finder_effect {
                    Some(FinderEffect::Request(request)) => {
                        return Some(SystemEffect::Request(request));
                    }
                    Some(FinderEffect::Browse { dir, file }) => {
                        action = Some(Action::Browse { dir, file });
                    }
                    Some(FinderEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(FinderEffect::Quit) => {
                        action = Some(Action::QuitFinder);
                    }
                    Some(FinderEffect::Bell) => {
                        action = Some(Action::Bell);
                    }
                    None => {}
                }
            }
            Mode::Searcher => {
                let searcher = self.state.searcher.as_mut().unwrap();
                let searcher_effect: Option<SearcherEffect> = searcher.handle(event);
                match searcher_effect {
                    Some(SearcherEffect::Goto { dir, file }) => {
                        action = Some(Action::Browse { dir, file });
                    }
                    Some(SearcherEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(SearcherEffect::Bell) => {
                        action = Some(Action::Bell);
                    }
                    Some(SearcherEffect::Request(request)) => {
                        return Some(SystemEffect::Request(request));
                    }
                    Some(SearcherEffect::Quit) => {
                        action = Some(Action::QuitSearcher);
                    }
                    None => {}
                }
            }
            Mode::Nothing => {
                return Some(SystemEffect::Exit);
            }
        }

        if let Some(action) = action {
            let effect = self.state.perform(action);
            return effect;
        }

        None
    }

    fn render(&self, size: Size) -> Fabric {
        match self.state.mode {
            Mode::Browse => self.state.browser.as_ref().unwrap().render(size),
            Mode::FileCreator => self.state.file_creator.as_ref().unwrap().render(size),
            Mode::Finder => self.state.finder.as_ref().unwrap().render(size),
            Mode::Searcher => self.state.searcher.as_ref().unwrap().render(size),
            Mode::Nothing => Fabric::new(size),
        }
    }
}

/// The state of insh.
struct State {
    /// Which mode insh is in.
    mode: Mode,
    /// The browser.
    browser: Option<Browser>,
    /// The file creator.
    file_creator: Option<FileCreator>,
    /// The finder.
    finder: Option<Finder>,
    /// The searcher.
    searcher: Option<Searcher>,
    /// The configuration.
    config: Config,
}

impl From<Props> for State {
    fn from(props: Props) -> Self {
        let dir: PathBuf = props.dir().clone().unwrap_or_else(current_dir::current_dir);
        let size: Size = Term::size().unwrap();

        let browser_props = BrowserProps::builder()
            .config(props.config().clone())
            .dir(dir.clone())
            .size(size)
            .pending_request(*props.pending_browser_request())
            .build();
        let browser = Some(Browser::new(browser_props));
        match props.start() {
            Start::Browser => Self {
                mode: Mode::Browse,
                browser,
                file_creator: None,
                finder: None,
                searcher: None,
                config: props.config().clone(),
            },
            Start::Finder { phrase } => {
                let finder_props = FinderProps::builder()
                    .dir(dir)
                    .size(size)
                    .phrase(phrase.clone())
                    .build();
                let finder = Some(Finder::new(finder_props));
                Self {
                    mode: Mode::Finder,
                    browser,
                    file_creator: None,
                    finder,
                    searcher: None,
                    config: props.config().clone(),
                }
            }
            Start::Searcher { phrase } => {
                let searcher_props = SearcherProps::new(
                    props.config().clone(),
                    dir,
                    size,
                    phrase.clone(),
                    *props.pending_search_request(),
                );
                let searcher = Some(Searcher::new(searcher_props));
                Self {
                    mode: Mode::Searcher,
                    browser,
                    file_creator: None,
                    finder: None,
                    searcher,
                    config: props.config().clone(),
                }
            }
            Start::Nothing => Self {
                mode: Mode::Nothing,
                browser: None,
                file_creator: None,
                finder: None,
                searcher: None,
                config: props.config().clone(),
            },
        }
    }
}

impl State {
    /// Browse a directory.
    fn browse(&mut self, dir: PathBuf, file: Option<PathBuf>) -> Option<SystemEffect<Request>> {
        // Create a request for getting the files in the dir.
        let request = Request::builder()
            .params(RequestParams::GetFiles(
                GetFilesRequestParams::builder()
                    .dir(dir.clone())
                    .sort(self.config.browser().sort().map(FileSortOptions::from))
                    .metadata(self.config.browser().metadata())
                    .build(),
            ))
            .build();
        let visit_request = Request::builder()
            .params(RequestParams::VisitDir(
                VisitDirRequestParams::builder().dir(dir.clone()).build(),
            ))
            .build();

        self.mode = Mode::Browse;
        let size: Size = Term::size().unwrap();
        let browser_props = BrowserProps::builder()
            .config(self.config.clone())
            .dir(dir)
            .size(size)
            .file(file)
            .pending_request(Some(*request.uuid()))
            .build();
        self.browser = Some(Browser::new(browser_props));

        Some(SystemEffect::Requests(vec![visit_request, request]))
    }

    /// Make a new file.
    fn create_file(&mut self, dir: PathBuf, file_type: FileType) -> Option<SystemEffect<Request>> {
        self.mode = Mode::FileCreator;
        let file_creator_props = FileCreatorProps::builder()
            .dir(dir)
            .file_type(file_type)
            .build();
        self.file_creator = Some(FileCreator::new(file_creator_props));
        None
    }

    /// Find files by name.
    fn find(&mut self, dir: PathBuf) -> Option<SystemEffect<Request>> {
        self.mode = Mode::Finder;
        let size: Size = Term::size().unwrap();
        let phrase = None;
        let finder_props = FinderProps::builder()
            .dir(dir)
            .size(size)
            .phrase(phrase)
            .build();
        self.finder = Some(Finder::new(finder_props));
        None
    }

    /// Search files for a phrase.
    fn search(&mut self, dir: PathBuf) -> Option<SystemEffect<Request>> {
        self.mode = Mode::Searcher;
        let size: Size = Term::size().unwrap();
        let phrase = None;
        let searcher_props = SearcherProps::new(self.config.clone(), dir, size, phrase, None);
        self.searcher = Some(Searcher::new(searcher_props));
        None
    }

    /// Go back to the browser from the finder.
    fn quit_finder(&mut self) -> Option<SystemEffect<Request>> {
        self.mode = Mode::Browse;
        None
    }

    /// Go back to the browser from the searcher.
    fn quit_searcher(&mut self) -> Option<SystemEffect<Request>> {
        self.mode = Mode::Browse;
        None
    }

    /// If the bell sound is configured to be made, then return the effect for making the bell
    /// sound.
    fn bell(&self) -> Option<SystemEffect<Request>> {
        match self.config.general().bell() {
            true => Some(SystemEffect::Bell),
            false => None,
        }
    }
}

impl Stateful<Action, SystemEffect<Request>> for State {
    fn perform(&mut self, action: Action) -> Option<SystemEffect<Request>> {
        match action {
            Action::Browse { dir, file } => self.browse(dir, file),
            Action::CreateFile { dir, file_type } => self.create_file(dir, file_type),
            Action::Find { dir } => self.find(dir),
            Action::Search { dir } => self.search(dir),
            Action::QuitFinder => self.quit_finder(),
            Action::QuitSearcher => self.quit_searcher(),
            Action::Bell => self.bell(),
        }
    }
}

/// Which mode insh is in.
#[derive(Default)]
enum Mode {
    /// Browsing the files in a directory.
    #[default]
    Browse,
    /// Making a new file.
    FileCreator,
    /// Finding files by name.
    Finder,
    /// Searching files for a phrase.
    Searcher,
    /// Nothing, so insh exits when anything happens.
    Nothing,
}

/// An insh action.
enum Action {
    /// Browse a directory.
    Browse {
        /// The directory to browse.
        dir: PathBuf,
        /// The file to select.
        file: Option<PathBuf>,
    },
    /// Make a new file.
    CreateFile {
        /// The directory to make it in.
        dir: PathBuf,
        /// The type of file to make.
        file_type: FileType,
    },
    /// Find files by name.
    Find {
        /// The directory to look in.
        dir: PathBuf,
    },
    /// Search files for a phrase.
    Search {
        /// The directory to search in.
        dir: PathBuf,
    },
    /// Ring the bell.
    Bell,
    /// Go back to the browser from the finder.
    QuitFinder,
    /// Go back to the browser from the searcher.
    QuitSearcher,
}

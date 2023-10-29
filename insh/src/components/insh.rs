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
use insh_api::{FindFilesRequestParams, GetFilesRequestParams, Request, RequestParams, Response};
use rend::{Fabric, Size};
use term::{Key, KeyEvent, KeyMods, TermEvent};
use til::{Component, Event, SystemEffect};

use std::path::PathBuf;

use crossterm::terminal;

mod props {
    use std::path::PathBuf;

    use typed_builder::TypedBuilder;
    use uuid::Uuid;

    use crate::args::Command;
    use crate::config::Config;

    #[derive(TypedBuilder)]
    pub struct Props {
        start: Start,
        dir: Option<PathBuf>,
        #[builder(default)]
        pending_browser_request: Option<Uuid>,
        config: Config,
    }

    impl Props {
        pub fn start(&self) -> &Start {
            &self.start
        }

        pub fn dir(&self) -> &Option<PathBuf> {
            &self.dir
        }

        pub fn pending_browser_request(&self) -> &Option<Uuid> {
            &self.pending_browser_request
        }

        pub fn config(&self) -> &Config {
            &self.config
        }
    }

    pub enum Start {
        Browser,
        Finder { phrase: Option<String> },
        Searcher { phrase: Option<String> },
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

pub struct Insh {
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
                    Some(FinderEffect::SendFindFilesRequest { uuid, dir, pattern }) => {
                        let params: RequestParams = RequestParams::FindFiles(
                            FindFilesRequestParams::builder()
                                .dir(dir)
                                .pattern(pattern)
                                .build(),
                        );
                        let request: Request = Request::builder().uuid(uuid).params(params).build();
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
                let event = match event {
                    Event::TermEvent(event) => event,
                    Event::Response(_) => {
                        #[cfg(feature = "logging")]
                        log::warn!("Searcher doesn't handle responses yet.");
                        return None;
                    }
                };

                let searcher = self.state.searcher.as_mut().unwrap();
                let searcher_effect: Option<SearcherEffect> = searcher.handle(event);
                match searcher_effect {
                    Some(SearcherEffect::Goto { dir, file }) => {
                        action = Some(Action::Browse { dir, file });
                    }
                    Some(SearcherEffect::Quit) => {
                        action = Some(Action::QuitSearcher);
                    }
                    Some(SearcherEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(SearcherEffect::Bell) => {
                        action = Some(Action::Bell);
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

struct State {
    mode: Mode,
    browser: Option<Browser>,
    file_creator: Option<FileCreator>,
    finder: Option<Finder>,
    searcher: Option<Searcher>,
    config: Config,
}

impl From<Props> for State {
    fn from(props: Props) -> Self {
        let dir: PathBuf = props.dir().clone().unwrap_or_else(current_dir::current_dir);
        let size: Size = Size::from(terminal::size().unwrap());

        let browser_props = BrowserProps::builder()
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
                let searcher_props =
                    SearcherProps::new(props.config().clone(), dir, size, phrase.clone());
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
    fn browse(&mut self, dir: PathBuf, file: Option<PathBuf>) -> Option<SystemEffect<Request>> {
        // Create a request for getting the files in the dir.
        let request = Request::builder()
            .params(RequestParams::GetFiles(
                GetFilesRequestParams::builder().dir(dir.clone()).build(),
            ))
            .build();

        self.mode = Mode::Browse;
        let size: Size = Size::from(terminal::size().unwrap());
        let browser_props = BrowserProps::builder()
            .dir(dir)
            .size(size)
            .file(file)
            .pending_request(Some(*request.uuid()))
            .build();
        self.browser = Some(Browser::new(browser_props));

        Some(SystemEffect::Request(request))
    }

    fn create_file(&mut self, dir: PathBuf, file_type: FileType) -> Option<SystemEffect<Request>> {
        self.mode = Mode::FileCreator;
        let file_creator_props = FileCreatorProps::builder()
            .dir(dir)
            .file_type(file_type)
            .build();
        self.file_creator = Some(FileCreator::new(file_creator_props));
        None
    }

    fn find(&mut self, dir: PathBuf) -> Option<SystemEffect<Request>> {
        self.mode = Mode::Finder;
        let size: Size = Size::from(terminal::size().unwrap());
        let phrase = None;
        let finder_props = FinderProps::builder()
            .dir(dir)
            .size(size)
            .phrase(phrase)
            .build();
        self.finder = Some(Finder::new(finder_props));
        None
    }

    fn search(&mut self, dir: PathBuf) -> Option<SystemEffect<Request>> {
        self.mode = Mode::Searcher;
        let size: Size = Size::from(terminal::size().unwrap());
        let phrase = None;
        let searcher_props = SearcherProps::new(self.config.clone(), dir, size, phrase);
        self.searcher = Some(Searcher::new(searcher_props));
        None
    }

    fn quit_finder(&mut self) -> Option<SystemEffect<Request>> {
        self.mode = Mode::Browse;
        None
    }

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

#[derive(Default)]
enum Mode {
    #[default]
    Browse,
    FileCreator,
    Finder,
    Searcher,
    Nothing,
}

enum Action {
    Browse { dir: PathBuf, file: Option<PathBuf> },
    CreateFile { dir: PathBuf, file_type: FileType },
    Find { dir: PathBuf },
    Search { dir: PathBuf },
    Bell,
    QuitFinder,
    QuitSearcher,
}

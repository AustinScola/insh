use crate::components::browser::{Browser, BrowserEffect, BrowserProps};
use crate::components::file_creator::{FileCreator, FileCreatorEffect, FileCreatorProps};
use crate::components::finder::{Finder, FinderEffect, FinderProps};
use crate::components::searcher::{Searcher, SearcherEffect, SearcherProps};
use crate::config::Config;
use crate::current_dir;
use crate::programs::{Bash, Vim};
use crate::stateful::Stateful;

use insh_api::{FindFilesRequestParams, Request, RequestParams, Response};
use rend::{Fabric, Size};
use term::{Key, KeyEvent, KeyMods, TermEvent};
use til::{Component, Event, SystemEffect};

use std::path::PathBuf;

use crossterm::terminal;

mod props {
    use crate::args::{Args, Command};
    use crate::config::Config;
    use crate::current_dir;

    use std::path::PathBuf;

    pub struct Props {
        directory: Option<PathBuf>,
        start: Start,
        config: Config,
    }

    impl From<(Args, Config)> for Props {
        fn from(args_and_config: (Args, Config)) -> Self {
            let (args, config) = args_and_config;

            let mut directory: Option<PathBuf> =
                args.directory().as_ref().map(|path| path.to_path_buf());

            // If the directory was not passed as an argument, and we are editing a file and then
            // browsing, then the directory should be the directory of the file (if a file was
            // passed).
            if directory.is_none() {
                if let Some(Command::Edit {
                    browse,
                    file_line_column,
                }) = args.command()
                {
                    if *browse {
                        if let Some(file_line_column) = file_line_column {
                            if let Some(file) = file_line_column.file() {
                                directory = match file.parent() {
                                    Some(parent) => Some(parent.to_path_buf()),
                                    None => Some(PathBuf::from("/")),
                                }
                            }
                        }
                    }
                }
            }

            // If the directory is relative, make it absolute.
            if let Some(directory_) = &directory {
                if directory_.is_relative() {
                    let mut absolute_directory = current_dir::current_dir();
                    absolute_directory.push(directory_);
                    directory = Some(absolute_directory);
                }
            }

            Self {
                directory,
                start: Start::from(args.command().clone()),
                config,
            }
        }
    }

    impl Props {
        pub fn directory(&self) -> &Option<PathBuf> {
            &self.directory
        }

        pub fn start(&self) -> &Start {
            &self.start
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
                let event = match event {
                    Event::TermEvent(event) => event,
                    Event::Response(_) => {
                        #[cfg(feature = "logging")]
                        log::warn!("Browser doesn't handle responses yet.");
                        return None;
                    }
                };

                let browser = self.state.browser.as_mut().unwrap();
                let browser_effect: Option<BrowserEffect> = browser.handle(event);
                match browser_effect {
                    Some(BrowserEffect::OpenFileCreator { directory }) => {
                        action = Some(Action::CreateFile { directory });
                    }
                    Some(BrowserEffect::OpenFinder { directory }) => {
                        action = Some(Action::Find { directory });
                    }
                    Some(BrowserEffect::OpenSearcher { directory }) => {
                        action = Some(Action::Search { directory });
                    }
                    Some(BrowserEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(BrowserEffect::RunBash { directory }) => {
                        let program = Box::new(Bash::new(directory));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(BrowserEffect::Bell) => {
                        action = Some(Action::Bell);
                    }
                    None => {}
                }
            }
            Mode::FileCreator => {
                let event = match event {
                    Event::TermEvent(event) => event,
                    Event::Response(_) => {
                        #[cfg(feature = "logging")]
                        log::warn!("File creator doesn't handle responses yet.");
                        return None;
                    }
                };

                let file_creator = self.state.file_creator.as_mut().unwrap();
                let file_creator_effect: Option<FileCreatorEffect> = file_creator.handle(event);
                match file_creator_effect {
                    Some(FileCreatorEffect::Browse { directory, file }) => {
                        action = Some(Action::Browse { directory, file });
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
                    Some(FinderEffect::Browse { directory, file }) => {
                        action = Some(Action::Browse { directory, file });
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
                    Some(SearcherEffect::Goto { directory, file }) => {
                        action = Some(Action::Browse { directory, file });
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
        let directory: PathBuf = props
            .directory()
            .clone()
            .unwrap_or_else(current_dir::current_dir);
        let size: Size = Size::from(terminal::size().unwrap());

        let browser_props = BrowserProps::new(directory.clone(), size, None);
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
                    .dir(directory)
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
                    SearcherProps::new(props.config().clone(), directory, size, phrase.clone());
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
    fn browse(
        &mut self,
        directory: PathBuf,
        file: Option<PathBuf>,
    ) -> Option<SystemEffect<Request>> {
        self.mode = Mode::Browse;
        let size: Size = Size::from(terminal::size().unwrap());
        let browser_props = BrowserProps::new(directory, size, file);
        self.browser = Some(Browser::new(browser_props));
        None
    }

    fn create_file(&mut self, directory: PathBuf) -> Option<SystemEffect<Request>> {
        self.mode = Mode::FileCreator;
        let file_creator_props = FileCreatorProps::builder().directory(directory).build();
        self.file_creator = Some(FileCreator::new(file_creator_props));
        None
    }

    fn find(&mut self, directory: PathBuf) -> Option<SystemEffect<Request>> {
        self.mode = Mode::Finder;
        let size: Size = Size::from(terminal::size().unwrap());
        let phrase = None;
        let finder_props = FinderProps::builder()
            .dir(directory)
            .size(size)
            .phrase(phrase)
            .build();
        self.finder = Some(Finder::new(finder_props));
        None
    }

    fn search(&mut self, directory: PathBuf) -> Option<SystemEffect<Request>> {
        self.mode = Mode::Searcher;
        let size: Size = Size::from(terminal::size().unwrap());
        let phrase = None;
        let searcher_props = SearcherProps::new(self.config.clone(), directory, size, phrase);
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
            Action::Browse { directory, file } => self.browse(directory, file),
            Action::CreateFile { directory } => self.create_file(directory),
            Action::Find { directory } => self.find(directory),
            Action::Search { directory } => self.search(directory),
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
    Browse {
        directory: PathBuf,
        file: Option<PathBuf>,
    },
    CreateFile {
        directory: PathBuf,
    },
    Find {
        directory: PathBuf,
    },
    Search {
        directory: PathBuf,
    },
    Bell,
    QuitFinder,
    QuitSearcher,
}
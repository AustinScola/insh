use super::browser::{Browser, BrowserEffect, BrowserProps};
use super::finder::{Finder, FinderEffect, FinderProps};
use super::searcher::{Searcher, SearcherEffect, SearcherProps};

use crate::component::Component;
use crate::current_dir;
use crate::programs::{Bash, Vim};
use crate::rendering::{Fabric, Size};
use crate::stateful::Stateful;
use crate::system_effect::SystemEffect;

use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;

mod props {
    use crate::args::{Args, Command};
    use crate::current_dir;

    use std::path::PathBuf;

    pub struct Props {
        directory: Option<PathBuf>,
        start: Start,
    }

    impl From<Args> for Props {
        fn from(args: Args) -> Self {
            let directory: Option<PathBuf> = match args.directory() {
                Some(path) => {
                    if path.is_relative() {
                        let mut new_path = current_dir::current_dir();
                        new_path.push(path);
                        Some(new_path)
                    } else {
                        Some(path.to_path_buf())
                    }
                }
                None => None,
            };

            Self {
                directory,
                start: Start::from(args.command().clone()),
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
    }

    pub enum Start {
        Browser,
        Finder { phrase: Option<String> },
        Searcher { phrase: Option<String> },
    }

    impl From<Option<Command>> for Start {
        fn from(command: Option<Command>) -> Self {
            match command {
                Some(Command::Browse) | None => Start::Browser,
                Some(Command::Search { phrase }) => Start::Searcher { phrase },
                Some(Command::Find { phrase }) => Start::Finder { phrase },
            }
        }
    }
}
pub use props::{Props, Start};

pub struct Insh {
    state: State,
}

impl Component<Props, Event, SystemEffect> for Insh {
    fn new(props: Props) -> Self {
        Self {
            state: State::from(props),
        }
    }

    fn handle(&mut self, event: Event) -> Option<SystemEffect> {
        if let Event::Key(KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::CONTROL,
        }) = event
        {
            return Some(SystemEffect::Exit);
        }

        let mut action: Option<Action> = None;

        match self.state.mode {
            Mode::Browse => {
                let browser = self.state.browser.as_mut().unwrap();
                let browser_effect: Option<BrowserEffect> = browser.handle(event);
                match browser_effect {
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
                    None => {}
                }
            }
            Mode::Finder => {
                let finder = self.state.finder.as_mut().unwrap();
                let finder_effect: Option<FinderEffect> = finder.handle(event);
                match finder_effect {
                    Some(FinderEffect::Browse { directory }) => {
                        action = Some(Action::Browse { directory });
                    }
                    Some(FinderEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(FinderEffect::Quit) => {
                        action = Some(Action::QuitFinder);
                    }
                    None => {}
                }
            }
            Mode::Searcher => {
                let searcher = self.state.searcher.as_mut().unwrap();
                let searcher_effect: Option<SearcherEffect> = searcher.handle(event);
                match searcher_effect {
                    Some(SearcherEffect::Quit) => {
                        action = Some(Action::QuitSearcher);
                    }
                    Some(SearcherEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    None => {}
                }
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
            Mode::Finder => self.state.finder.as_ref().unwrap().render(size),
            Mode::Searcher => self.state.searcher.as_ref().unwrap().render(size),
        }
    }
}

struct State {
    mode: Mode,
    browser: Option<Browser>,
    finder: Option<Finder>,
    searcher: Option<Searcher>,
}

impl From<Props> for State {
    fn from(props: Props) -> Self {
        let directory: PathBuf = props
            .directory()
            .clone()
            .unwrap_or_else(current_dir::current_dir);
        let size: Size = Size::from(terminal::size().unwrap());

        let browser_props = BrowserProps::new(directory.clone(), size);
        let browser = Some(Browser::new(browser_props));
        match props.start() {
            Start::Browser => Self {
                mode: Mode::Browse,
                browser,
                ..Default::default()
            },
            Start::Finder { phrase } => {
                let finder_props = FinderProps::new(directory, size, phrase.clone());
                let finder = Some(Finder::new(finder_props));
                Self {
                    mode: Mode::Finder,
                    browser,
                    finder,
                    ..Default::default()
                }
            }
            Start::Searcher { phrase } => {
                let searcher_props = SearcherProps::new(directory, size, phrase.clone());
                let searcher = Some(Searcher::new(searcher_props));
                Self {
                    mode: Mode::Searcher,
                    browser,
                    searcher,
                    ..Default::default()
                }
            }
        }
    }
}

impl State {
    fn browse(&mut self, directory: PathBuf) -> Option<SystemEffect> {
        self.mode = Mode::Browse;
        let size: Size = Size::from(terminal::size().unwrap());
        let browser_props = BrowserProps::new(directory, size);
        self.browser = Some(Browser::new(browser_props));
        None
    }

    fn find(&mut self, directory: PathBuf) -> Option<SystemEffect> {
        self.mode = Mode::Finder;
        let size: Size = Size::from(terminal::size().unwrap());
        let phrase = None;
        let finder_props = FinderProps::new(directory, size, phrase);
        self.finder = Some(Finder::new(finder_props));
        None
    }

    fn search(&mut self, directory: PathBuf) -> Option<SystemEffect> {
        self.mode = Mode::Searcher;
        let size: Size = Size::from(terminal::size().unwrap());
        let phrase = None;
        let searcher_props = SearcherProps::new(directory, size, phrase);
        self.searcher = Some(Searcher::new(searcher_props));
        None
    }

    fn quit_finder(&mut self) -> Option<SystemEffect> {
        self.mode = Mode::Browse;
        None
    }

    fn quit_searcher(&mut self) -> Option<SystemEffect> {
        self.mode = Mode::Browse;
        None
    }
}

impl Default for State {
    fn default() -> Self {
        let mode: Mode = Mode::default();

        let size: Size = Size::from(terminal::size().unwrap());

        let directory: PathBuf = current_dir::current_dir();
        let browser: Option<Browser> = Some(Browser::new(BrowserProps::new(directory, size)));
        let finder: Option<Finder> = None;
        let searcher: Option<Searcher> = None;

        State {
            mode,
            browser,
            finder,
            searcher,
        }
    }
}

impl Stateful<Action, SystemEffect> for State {
    fn perform(&mut self, action: Action) -> Option<SystemEffect> {
        match action {
            Action::Browse { directory } => self.browse(directory),
            Action::Find { directory } => self.find(directory),
            Action::Search { directory } => self.search(directory),
            Action::QuitFinder => self.quit_finder(),
            Action::QuitSearcher => self.quit_searcher(),
        }
    }
}

enum Mode {
    Browse,
    Finder,
    Searcher,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Browse
    }
}

enum Action {
    Browse { directory: PathBuf },
    Find { directory: PathBuf },
    Search { directory: PathBuf },
    QuitFinder,
    QuitSearcher,
}

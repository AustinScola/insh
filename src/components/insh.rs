use crate::component::Component;
use crate::current_dir::current_dir;
use crate::programs::{Bash, Vim};
use crate::rendering::{Fabric, Size};
use crate::stateful::Stateful;
use crate::system_effect::SystemEffect;

use super::browse::{Browse, BrowseEffect, BrowseProps};
use super::finder::{Finder, FinderEffect, FinderProps};
use super::searcher::{Searcher, SearcherEffect, SearcherProps};

use std::env;
use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;

pub struct Props {}

pub struct Insh {
    state: State,
}

impl Component<Props, Event, SystemEffect> for Insh {
    fn new(_props: Props) -> Self {
        let state = State::default();
        Self { state }
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
                let browse_effect: Option<BrowseEffect> = self.state.browse.handle(event);
                match browse_effect {
                    Some(BrowseEffect::OpenFinder { directory }) => {
                        action = Some(Action::OpenFinder { directory });
                    }
                    Some(BrowseEffect::OpenSearcher { directory }) => {
                        action = Some(Action::OpenSearcher { directory });
                    }
                    Some(BrowseEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(BrowseEffect::RunBash { directory }) => {
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
                        action = Some(Action::OpenBrowse { directory });
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
            Mode::Home => {}
        }

        if let Some(action) = action {
            let effect = self.state.perform(action);
            return effect;
        }

        None
    }

    fn render(&self, size: Size) -> Fabric {
        match self.state.mode {
            Mode::Home => Fabric::new(size),
            Mode::Browse => self.state.browse.render(size),
            Mode::Finder => self.state.finder.as_ref().unwrap().render(size),
            Mode::Searcher => self.state.searcher.as_ref().unwrap().render(size),
        }
    }
}

struct State {
    mode: Mode,
    browse: Browse,
    finder: Option<Finder>,
    searcher: Option<Searcher>,
}

impl State {
    fn open_browser(&mut self, directory: PathBuf) -> Option<SystemEffect> {
        self.mode = Mode::Browse;
        let size: Size = Size::from(terminal::size().unwrap());
        let browse_props = BrowseProps::new(directory, size);
        self.browse = Browse::new(browse_props);
        None
    }

    fn open_finder(&mut self, directory: PathBuf) -> Option<SystemEffect> {
        self.mode = Mode::Finder;
        let size: Size = Size::from(terminal::size().unwrap());
        let finder_props = FinderProps::new(directory, size);
        self.finder = Some(Finder::new(finder_props));
        None
    }

    fn open_searcher(&mut self, directory: PathBuf) -> Option<SystemEffect> {
        self.mode = Mode::Searcher;
        let size: Size = Size::from(terminal::size().unwrap());
        let searcher_props = SearcherProps::new(directory, size);
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

        let directory: PathBuf = current_dir();
        let browse: Browse = Browse::new(BrowseProps::new(directory, size));
        let finder: Option<Finder> = None;
        let searcher: Option<Searcher> = None;

        State {
            mode,
            browse,
            finder,
            searcher,
        }
    }
}

impl Stateful<Action, SystemEffect> for State {
    fn perform(&mut self, action: Action) -> Option<SystemEffect> {
        match action {
            Action::OpenBrowse { directory } => self.open_browser(directory),
            Action::OpenFinder { directory } => self.open_finder(directory),
            Action::OpenSearcher { directory } => self.open_searcher(directory),
            Action::QuitFinder => self.quit_finder(),
            Action::QuitSearcher => self.quit_searcher(),
        }
    }
}

enum Mode {
    Home,
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
    OpenBrowse { directory: PathBuf },
    OpenFinder { directory: PathBuf },
    OpenSearcher { directory: PathBuf },
    QuitFinder,
    QuitSearcher,
}

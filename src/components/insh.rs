use crate::component::Component;
use crate::programs::{Bash, Vim};
use crate::rendering::{Fabric, Size};
use crate::stateful::Stateful;
use crate::system_effect::SystemEffect;

use super::browse::{Browse, BrowseEffect, BrowseProps};
use super::finder::{Finder, FinderEffect, FinderProps};

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
                    Some(BrowseEffect::OpenVim { file }) => {
                        let program = Box::new(Vim::new(file));
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
                    Some(FinderEffect::OpenVim { file }) => {
                        let program = Box::new(Vim::new(file));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(FinderEffect::Quit) => {
                        action = Some(Action::QuitFinder);
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
        }
    }
}

struct State {
    mode: Mode,
    browse: Browse,
    finder: Option<Finder>,
}

impl State {
    fn open_finder(&mut self, directory: PathBuf) -> Option<SystemEffect> {
        self.mode = Mode::Finder;
        let size: Size = Size::from(terminal::size().unwrap());
        let finder_props = FinderProps::new(directory, size);
        self.finder = Some(Finder::new(finder_props));
        None
    }

    fn quit_finder(&mut self) -> Option<SystemEffect> {
        self.mode = Mode::Browse;
        None
    }

    fn open_browser(&mut self, directory: PathBuf) -> Option<SystemEffect> {
        self.mode = Mode::Browse;
        let size: Size = Size::from(terminal::size().unwrap());
        let browse_props = BrowseProps::new(directory, size);
        self.browse = Browse::new(browse_props);
        None
    }
}

impl Default for State {
    fn default() -> Self {
        let mode: Mode = Mode::default();

        let size: Size = Size::from(terminal::size().unwrap());

        let directory: PathBuf = env::current_dir().unwrap();
        let browse: Browse = Browse::new(BrowseProps::new(directory, size));
        let finder: Option<Finder> = None;

        State {
            mode,
            browse,
            finder,
        }
    }
}

impl Stateful<Action, SystemEffect> for State {
    fn perform(&mut self, action: Action) -> Option<SystemEffect> {
        match action {
            Action::OpenFinder { directory } => self.open_finder(directory),
            Action::OpenBrowse { directory } => self.open_browser(directory),
            Action::QuitFinder => self.quit_finder(),
        }
    }
}

enum Mode {
    Home,
    Browse,
    Finder,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Browse
    }
}

enum Action {
    OpenFinder { directory: PathBuf },
    OpenBrowse { directory: PathBuf },
    QuitFinder,
}

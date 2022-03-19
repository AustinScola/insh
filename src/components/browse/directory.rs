use crate::color::Color;
use crate::component::Component;
use crate::rendering::{Fabric, Size, Yarn};
use crate::stateful::Stateful;

use std::env;
use std::path::PathBuf;

pub use crossterm::event::Event as CrosstermEvent;

pub struct Props {}

pub struct Directory {
    state: State,
}

impl Component<Props, Event, Effect> for Directory {
    fn new(_props: Props) -> Self {
        let state = State::default();
        Self { state }
    }

    fn handle(&mut self, event: Event) -> Option<Effect> {
        if let Some(action) = self.map(event) {
            return self.state.perform(action);
        }
        None
    }

    fn render(&self, size: Size) -> Fabric {
        let string = self.state.directory_string();
        let mut yarn = Yarn::from(string);
        yarn.resize(size.columns);
        yarn.color(Color::InvertedText.into());
        yarn.background(Color::InvertedBackground.into());

        Fabric::from(yarn)
    }
}

impl Directory {
    fn map(&self, event: Event) -> Option<Action> {
        match event {
            Event::SetDirectory { directory } => Some(Action::SetDirectory { directory }),
            Event::CrosstermEvent { event: _ } => None,
        }
    }
}

impl Default for Directory {
    fn default() -> Self {
        let state = State::default();
        Self { state }
    }
}

pub enum Event {
    SetDirectory { directory: PathBuf },
    CrosstermEvent { event: CrosstermEvent },
}

struct State {
    directory: PathBuf,
}

impl State {
    pub fn directory_string(&self) -> String {
        self.directory.to_str().unwrap().to_string()
    }

    fn set_directory(&mut self, directory: PathBuf) {
        self.directory = directory;
    }

    fn pop_directory(&mut self) {
        self.directory.pop();
    }
}

impl Default for State {
    fn default() -> Self {
        let directory: PathBuf = env::current_dir().unwrap();
        State { directory }
    }
}

impl Stateful<Action, Effect> for State {
    fn perform(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::SetDirectory { directory } => {
                self.set_directory(directory);
            }
            Action::PopDirectory => {
                self.pop_directory();
            }
        }
        None
    }
}

enum Action {
    SetDirectory { directory: PathBuf },
    PopDirectory,
}

pub enum Effect {}

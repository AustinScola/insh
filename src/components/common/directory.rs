mod props {
    use std::path::PathBuf;

    pub struct Props {
        pub directory: PathBuf,
    }

    impl Props {
        pub fn new(directory: PathBuf) -> Self {
            Self { directory }
        }
    }
}
pub use props::Props;

mod directory {
    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::component::Component;
    use crate::rendering::{Fabric, Size, Yarn};
    use crate::stateful::Stateful;

    pub struct Directory {
        state: State,
    }

    impl Component<Props, Event, Effect> for Directory {
        fn new(props: Props) -> Self {
            let state = State::from(props);
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
}
pub use directory::Directory;

mod event {
    use crossterm::event::Event as CrosstermEvent;
    use std::path::PathBuf;

    pub enum Event {
        SetDirectory { directory: PathBuf },
        CrosstermEvent { event: CrosstermEvent },
    }
}
pub use event::Event;

mod state {
    use super::{Action, Effect, Props};
    use crate::stateful::Stateful;

    use std::env;
    use std::path::{PathBuf, MAIN_SEPARATOR as PATH_SEPARATOR};

    pub struct State {
        directory: PathBuf,
        home: Option<PathBuf>,
    }

    impl State {
        pub fn directory_string(&self) -> String {
            let mut string: String;
            if let Some(home) = &self.home {
                if let Ok(path) = self.directory.strip_prefix(home) {
                    let mut string = String::from("~");
                    string.push(PATH_SEPARATOR);

                    let path_string = path.to_str().unwrap();
                    if path_string.len() > 0 {
                        string.push_str(path.to_str().unwrap());
                        string.push(PATH_SEPARATOR);
                    }

                    return string;
                }
            }

            let mut string = self.directory.to_str().unwrap().to_string();
            if self.directory.parent().is_some() {
                string.push(PATH_SEPARATOR);
            }
            string
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
            let home: Option<PathBuf> = dirs::home_dir();
            State { directory, home }
        }
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                directory: props.directory,
                ..Default::default()
            }
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
}
use state::State;

mod action {
    use std::path::PathBuf;

    pub enum Action {
        SetDirectory { directory: PathBuf },
        PopDirectory,
    }
}
use action::Action;

mod effect {
    pub enum Effect {}
}
pub use effect::Effect;

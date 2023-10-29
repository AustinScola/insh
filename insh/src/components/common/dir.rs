mod props {
    use std::path::PathBuf;

    pub struct Props {
        pub dir: PathBuf,
    }

    impl Props {
        pub fn new(dir: PathBuf) -> Self {
            Self { dir }
        }
    }
}
pub use props::Props;

mod dir {
    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::stateful::Stateful;

    use rend::{Fabric, Size, Yarn};
    use til::Component;

    pub struct Dir {
        state: State,
    }

    impl Component<Props, Event, Effect> for Dir {
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
            let string = self.state.dir_string();
            let mut yarn = Yarn::from(string);
            yarn.resize(size.columns);
            yarn.color(Color::InvertedText.into());
            yarn.background(Color::InvertedBackground.into());

            Fabric::from(yarn)
        }
    }

    impl Dir {
        fn map(&self, event: Event) -> Option<Action> {
            match event {
                Event::SetDir { dir } => Some(Action::SetDir { dir }),
                Event::PopDir => Some(Action::PopDir),
            }
        }
    }

    impl Default for Dir {
        fn default() -> Self {
            let state = State::default();
            Self { state }
        }
    }
}
pub use dir::Dir;

mod event {
    use std::path::PathBuf;

    pub enum Event {
        SetDir { dir: PathBuf },
        PopDir,
    }
}
pub use event::Event;

mod state {
    use super::{Action, Effect, Props};
    use crate::current_dir;
    use crate::stateful::Stateful;

    use std::path::{PathBuf, MAIN_SEPARATOR as PATH_SEPARATOR};

    pub struct State {
        dir: PathBuf,
        home: Option<PathBuf>,
    }

    impl State {
        pub fn dir_string(&self) -> String {
            if let Some(home) = &self.home {
                if let Ok(path) = self.dir.strip_prefix(home) {
                    let mut string = String::from("~");
                    string.push(PATH_SEPARATOR);

                    let path_string = path.to_str().unwrap();
                    if !path_string.is_empty() {
                        string.push_str(path.to_str().unwrap());
                        string.push(PATH_SEPARATOR);
                    }

                    return string;
                }
            }

            let mut string = self.dir.to_str().unwrap().to_string();
            if self.dir.parent().is_some() {
                string.push(PATH_SEPARATOR);
            }
            string
        }

        fn set_dir(&mut self, dir: PathBuf) {
            self.dir = dir;
        }

        fn pop_dir(&mut self) {
            self.dir.pop();
        }
    }

    impl Default for State {
        fn default() -> Self {
            let dir: PathBuf = current_dir::current_dir();
            let home: Option<PathBuf> = dirs::home_dir();
            State { dir, home }
        }
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                dir: props.dir,
                ..Default::default()
            }
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::SetDir { dir } => {
                    self.set_dir(dir);
                }
                Action::PopDir => {
                    self.pop_dir();
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
        SetDir { dir: PathBuf },
        PopDir,
    }
}
use action::Action;

mod effect {
    pub enum Effect {}
}
pub use effect::Effect;

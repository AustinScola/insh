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
    use super::{Effect, Event, Props, State};
    use crate::color::Color;
    use crate::rendering::{Fabric, Size, Yarn};
    use crate::Component;

    pub struct Directory {
        state: State,
    }

    impl Component<Props, Event, Effect> for Directory {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Self { state }
        }

        fn handle(&mut self, _event: Event) -> Option<Effect> {
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
}
pub use directory::Directory;

mod event {
    pub enum Event {}
}
pub use event::Event;

mod state {
    use super::{Action, Effect, Props};
    use crate::stateful::Stateful;

    use std::path::PathBuf;

    pub struct State {
        directory: PathBuf,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                directory: props.directory,
            }
        }
    }

    impl State {
        pub fn directory_string(&self) -> String {
            self.directory.to_str().unwrap().to_string()
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, _action: Action) -> Option<Effect> {
            None
        }
    }
}
use state::State;

mod action {
    pub enum Action {}
}
use action::Action;

mod effect {
    pub enum Effect {}
}
pub use effect::Effect;

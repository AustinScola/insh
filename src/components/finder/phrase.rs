mod props {
    pub struct Props {}
}
pub use props::Props;

mod phrase {
    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::component::Component;
    use crate::rendering::{Fabric, Size, Yarn};
    use crate::stateful::Stateful;
    use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

    pub struct Phrase {
        state: State,
    }

    impl Default for Phrase {
        fn default() -> Self {
            let state = State::default();
            Self { state }
        }
    }

    impl Component<Props, Event, Effect> for Phrase {
        fn new(_props: Props) -> Self {
            Phrase::default()
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Option<Action> = match event {
                Event::Focus => Some(Action::Focus),
                Event::CrosstermEvent { event } => match { event } {
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }) => Some(Action::Quit),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    }) => Some(Action::Pop),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => Some(Action::Find),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char(character),
                        ..
                    }) => Some(Action::Push { character }),
                    _ => None,
                },
            };

            if let Some(action) = action {
                self.state.perform(action)
            } else {
                None
            }
        }

        fn render(&self, size: Size) -> Fabric {
            let string = self.state.value();
            let mut yarn = Yarn::from(string);
            yarn.resize(size.columns);
            yarn.color(Color::InvertedText.into());
            let background_color = Color::focus_or_important(self.state.is_focused());
            yarn.background(background_color.into());

            Fabric::from(yarn)
        }
    }
}
pub use phrase::Phrase;

mod event {
    use crossterm::event::Event as CrosstermEvent;

    pub enum Event {
        Focus,
        CrosstermEvent { event: CrosstermEvent },
    }
}
pub use event::Event;

mod state {
    use super::{Action, Effect};
    use crate::stateful::Stateful;

    pub struct State {
        value: String,
        focus: bool,
    }

    impl Default for State {
        fn default() -> Self {
            let value = String::new();
            let focus = true;
            Self { value, focus }
        }
    }

    impl State {
        pub fn value(&self) -> &str {
            &self.value
        }

        pub fn is_focused(&self) -> bool {
            self.focus
        }

        pub fn focus(&mut self) -> Option<Effect> {
            self.focus = true;
            None
        }

        fn push(&mut self, character: char) -> Option<Effect> {
            self.value.push(character);
            None
        }

        fn pop(&mut self) -> Option<Effect> {
            self.value.pop();
            None
        }

        fn find(&mut self) -> Option<Effect> {
            self.focus = false;
            Some(Effect::Find {
                phrase: self.value.clone(),
            })
        }

        fn quit(&mut self) -> Option<Effect> {
            Some(Effect::Quit)
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Focus => self.focus(),
                Action::Push { character } => self.push(character),
                Action::Pop => self.pop(),
                Action::Find => self.find(),
                Action::Quit => self.quit(),
            }
        }
    }
}
pub use state::State;

mod action {
    pub enum Action {
        Focus,
        Push { character: char },
        Pop,
        Find,
        Quit,
    }
}
pub use action::Action;

mod effect {
    pub enum Effect {
        Find { phrase: String },
        Quit,
    }
}
pub use effect::Effect;
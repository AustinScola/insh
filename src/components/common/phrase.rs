mod props {
    use crate::auto_completer::AutoCompleter;

    pub struct Props {
        pub auto_completer: Option<Box<dyn AutoCompleter<String, String>>>,
    }

    impl Props {
        pub fn new(auto_completer: Option<Box<dyn AutoCompleter<String, String>>>) -> Self {
            Self { auto_completer }
        }
    }
}
pub use props::Props;

mod phrase {
    use super::{Action, Effect, Event, Props, State};
    use crate::auto_completer::AutoCompleter;
    use crate::color::Color;
    use crate::component::Component;
    use crate::rendering::{Fabric, Size, Yarn};
    use crate::stateful::Stateful;

    use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

    #[derive(Default)]
    pub struct Phrase {
        state: State,
        auto_completer: Option<Box<dyn AutoCompleter<String, String>>>,
    }

    impl Component<Props, Event, Effect> for Phrase {
        fn new(props: Props) -> Self {
            Self {
                auto_completer: props.auto_completer,
                ..Default::default()
            }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Option<Action> = match event {
                Event::Focus => Some(Action::Focus),
                Event::Unfocus => Some(Action::Unfocus),
                Event::Set { phrase } => Some(Action::Set { phrase }),
                Event::CrosstermEvent { event } => match { event } {
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }) => Some(Action::Quit),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    }) => Some(Action::Pop {
                        auto_completer: &mut self.auto_completer,
                    }),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Tab,
                        modifiers: KeyModifiers::NONE,
                    }) => Some(Action::Complete),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => Some(Action::Enter),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char(character),
                        modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                    }) => Some(Action::Push {
                        character,
                        auto_completer: &mut self.auto_completer,
                    }),
                    _ => None,
                },
            };

            if let Some(action) = action {
                self.state.perform(action)
            } else {
                Some(Effect::Bell)
            }
        }

        fn render(&self, size: Size) -> Fabric {
            let string = self.state.value();
            let mut yarn = Yarn::from(string);
            yarn.color(Color::InvertedText.into());

            if self.state.is_focused() {
                if let Some(completion) = self.state.completion() {
                    if let Some(rest) = completion.strip_prefix(self.state.value()) {
                        let mut rest_yarn: Yarn = Yarn::from(rest);
                        rest_yarn.color(Color::InvertedGrayedText.into());
                        yarn = yarn.concat(rest_yarn);
                    }
                }
            }

            yarn.resize(size.columns);
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
        Unfocus,
        Set { phrase: String },
        CrosstermEvent { event: CrosstermEvent },
    }
}
pub use event::Event;

mod state {
    use super::{Action, Effect};
    use crate::auto_completer::AutoCompleter;
    use crate::stateful::Stateful;

    pub struct State {
        value: String,
        completion: Option<String>,
        focus: bool,
    }

    impl Default for State {
        fn default() -> Self {
            Self {
                value: String::new(),
                completion: None,
                focus: true,
            }
        }
    }

    impl State {
        pub fn value(&self) -> &str {
            &self.value
        }

        pub fn completion(&self) -> &Option<String> {
            &self.completion
        }

        pub fn is_focused(&self) -> bool {
            self.focus
        }

        pub fn focus(&mut self) -> Option<Effect> {
            self.focus = true;
            None
        }

        pub fn unfocus(&mut self) -> Option<Effect> {
            self.focus = false;
            None
        }

        pub fn set(&mut self, value: String) -> Option<Effect> {
            self.value = value;
            None
        }

        fn push(
            &mut self,
            character: char,
            auto_completer: &mut Option<Box<dyn AutoCompleter<String, String>>>,
        ) -> Option<Effect> {
            self.value.push(character);

            if let Some(auto_completer) = auto_completer {
                // TODO: Make auto completion non-blocking.
                self.completion = auto_completer.complete(self.value.clone());
            }

            None
        }

        fn pop(
            &mut self,
            auto_completer: &mut Option<Box<dyn AutoCompleter<String, String>>>,
        ) -> Option<Effect> {
            self.value.pop();

            if let Some(auto_completer) = auto_completer {
                self.completion = match self.value.is_empty() {
                    // TODO: Make auto completion non-blocking.
                    false => auto_completer.complete(self.value.clone()),
                    true => None,
                };
            }

            None
        }

        fn complete(&mut self) -> Option<Effect> {
            if let Some(completion) = &self.completion {
                self.value = completion.to_string();
                self.completion = None;
            }
            None
        }

        fn find(&mut self) -> Option<Effect> {
            self.focus = false;
            Some(Effect::Enter {
                phrase: self.value.clone(),
            })
        }

        fn quit(&mut self) -> Option<Effect> {
            Some(Effect::Quit)
        }
    }

    impl Stateful<Action<'_>, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Focus => self.focus(),
                Action::Unfocus => self.unfocus(),
                Action::Set { phrase } => self.set(phrase),
                Action::Push {
                    character,
                    auto_completer,
                } => self.push(character, auto_completer),
                Action::Pop { auto_completer } => self.pop(auto_completer),
                Action::Complete => self.complete(),
                Action::Enter => self.find(),
                Action::Quit => self.quit(),
            }
        }
    }
}
pub use state::State;

mod action {
    use crate::auto_completer::AutoCompleter;

    pub enum Action<'a> {
        Focus,
        Unfocus,
        Set {
            phrase: String,
        },
        Push {
            character: char,
            auto_completer: &'a mut Option<Box<dyn AutoCompleter<String, String>>>,
        },
        Pop {
            auto_completer: &'a mut Option<Box<dyn AutoCompleter<String, String>>>,
        },
        Complete,
        Enter,
        Quit,
    }
}
pub use action::Action;

mod effect {
    pub enum Effect {
        Enter { phrase: String },
        Bell,
        Quit,
    }
}
pub use effect::Effect;

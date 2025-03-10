mod props {
    use crate::auto_completer::AutoCompleter;

    use typed_builder::TypedBuilder;

    #[derive(TypedBuilder)]
    pub struct Props {
        #[builder(default, setter(into))]
        pub auto_completer: Option<Box<dyn AutoCompleter<String, String>>>,
        #[builder(default, setter(into))]
        pub value: Option<String>,
    }
}
pub use props::Props;

mod phrase {
    use super::{Action, Effect, Event, Props, State};
    use crate::auto_completer::AutoCompleter;
    use crate::color::Color;
    use crate::stateful::Stateful;

    use rend::{Fabric, Size, Yarn};
    use til::Component;

    use term::{Key, KeyEvent, KeyMods, TermEvent};

    #[derive(Default)]
    pub struct Phrase {
        state: State,
        auto_completer: Option<Box<dyn AutoCompleter<String, String>>>,
    }

    impl Component<Props, Event, Effect> for Phrase {
        fn new(props: Props) -> Self {
            Self {
                state: State::builder()
                    .value(props.value.unwrap_or_default())
                    .build(),
                auto_completer: props.auto_completer,
            }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Option<Action> = match event {
                Event::Focus => Some(Action::Focus),
                Event::Unfocus => Some(Action::Unfocus),
                Event::Set { phrase } => Some(Action::Set { phrase }),
                Event::TermEvent(key_event) => match key_event {
                    TermEvent::KeyEvent(KeyEvent {
                        key: Key::Char('q'),
                        mods: KeyMods::CONTROL,
                        ..
                    }) => Some(Action::Quit),
                    TermEvent::KeyEvent(KeyEvent {
                        key: Key::Delete, ..
                    }) => Some(Action::Pop {
                        auto_completer: &mut self.auto_completer,
                    }),
                    TermEvent::KeyEvent(KeyEvent {
                        key: Key::HorizontalTab,
                        mods: KeyMods::NONE,
                    }) => Some(Action::Complete),
                    TermEvent::KeyEvent(KeyEvent {
                        key: Key::CarriageReturn,
                        ..
                    }) => Some(Action::Enter),
                    TermEvent::KeyEvent(KeyEvent {
                        key: Key::Char(character),
                        mods: KeyMods::NONE | KeyMods::SHIFT,
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
    use term::TermEvent;

    #[allow(clippy::enum_variant_names)]
    pub enum Event {
        Focus,
        Unfocus,
        Set { phrase: String },
        TermEvent(TermEvent),
    }
}
pub use event::Event;

mod state {
    use super::{Action, Effect};
    use crate::auto_completer::AutoCompleter;
    use crate::stateful::Stateful;

    use typed_builder::TypedBuilder;

    #[derive(TypedBuilder)]
    pub struct State {
        #[builder(default, setter(into))]
        value: String,
        #[builder(default, setter(into))]
        completion: Option<String>,
        #[builder(default = true, setter(into))]
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

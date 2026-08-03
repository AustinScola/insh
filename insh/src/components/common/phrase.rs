mod props {
    use typed_builder::TypedBuilder;

    #[derive(TypedBuilder)]
    pub struct Props {
        /// Whether or not completions should be requested for the phrase as it is typed.
        #[builder(default)]
        pub completable: bool,
        #[builder(default, setter(into))]
        pub value: Option<String>,
    }
}
pub use props::Props;

mod phrase {
    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::stateful::Stateful;

    use rend::{Fabric, Size, Yarn};
    use til::Component;

    use term::{Key, KeyEvent, KeyMods, TermEvent};

    #[derive(Default)]
    pub struct Phrase {
        state: State,
    }

    impl Component<Props, Event, Effect> for Phrase {
        fn new(props: Props) -> Self {
            Self {
                state: State::builder()
                    .value(props.value.unwrap_or_default())
                    .completable(props.completable)
                    .build(),
            }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Option<Action> = match event {
                Event::Focus => Some(Action::Focus),
                Event::Unfocus => Some(Action::Unfocus),
                Event::TermEvent(key_event) => match key_event {
                    TermEvent::KeyEvent(KeyEvent {
                        key: Key::Char('q'),
                        mods: KeyMods::CONTROL,
                        ..
                    }) => Some(Action::Quit),
                    TermEvent::KeyEvent(KeyEvent {
                        key: Key::Delete, ..
                    }) => Some(Action::Pop),
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
                    }) => Some(Action::Push { character }),
                    _ => None,
                },
                Event::Completion { uuid, completion } => {
                    Some(Action::SetCompletion { uuid, completion })
                }
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
    use uuid::Uuid;

    #[allow(clippy::enum_variant_names)]
    pub enum Event {
        Focus,
        Unfocus,
        TermEvent(TermEvent),
        /// A completion (or lack thereof) requested via `Effect::RequestCompletion` has arrived.
        Completion {
            uuid: Uuid,
            completion: Option<String>,
        },
    }
}
pub use event::Event;

mod state {
    use super::{Action, Effect};
    use crate::stateful::Stateful;

    use typed_builder::TypedBuilder;
    use uuid::Uuid;

    #[derive(TypedBuilder)]
    pub struct State {
        #[builder(default, setter(into))]
        value: String,
        #[builder(default, setter(into))]
        completion: Option<String>,
        #[builder(default = true, setter(into))]
        focus: bool,
        #[builder(default)]
        completable: bool,
        #[builder(default)]
        pending_completion_request: Option<Uuid>,
    }

    impl Default for State {
        fn default() -> Self {
            Self {
                value: String::new(),
                completion: None,
                focus: true,
                completable: false,
                pending_completion_request: None,
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

        /// If completable, request a completion for the current value.
        fn request_completion(&mut self) -> Option<Effect> {
            if !self.completable {
                return None;
            }

            let uuid: Uuid = Uuid::new_v4();
            self.pending_completion_request = Some(uuid);
            Some(Effect::RequestCompletion {
                uuid,
                partial: self.value.clone(),
            })
        }

        fn push(&mut self, character: char) -> Option<Effect> {
            self.value.push(character);
            self.request_completion()
        }

        fn pop(&mut self) -> Option<Effect> {
            self.value.pop();

            if self.value.is_empty() {
                self.completion = None;
                self.pending_completion_request = None;
                return None;
            }

            self.request_completion()
        }

        fn set_completion(&mut self, uuid: Uuid, completion: Option<String>) -> Option<Effect> {
            if self.pending_completion_request != Some(uuid) {
                return None;
            }

            self.completion = completion;
            self.pending_completion_request = None;

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

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Focus => self.focus(),
                Action::Unfocus => self.unfocus(),
                Action::Push { character } => self.push(character),
                Action::Pop => self.pop(),
                Action::SetCompletion { uuid, completion } => self.set_completion(uuid, completion),
                Action::Complete => self.complete(),
                Action::Enter => self.find(),
                Action::Quit => self.quit(),
            }
        }
    }
}
pub use state::State;

mod action {
    use uuid::Uuid;

    pub enum Action {
        Focus,
        Unfocus,
        Push {
            character: char,
        },
        Pop,
        SetCompletion {
            uuid: Uuid,
            completion: Option<String>,
        },
        Complete,
        Enter,
        Quit,
    }
}
pub use action::Action;

mod effect {
    use uuid::Uuid;

    pub enum Effect {
        RequestCompletion { uuid: Uuid, partial: String },
        Enter { phrase: String },
        Bell,
        Quit,
    }
}
pub use effect::Effect;

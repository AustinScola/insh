//! Contains the [`Phrase`] component.

/// Contains the [`Props`] struct.
mod props {
    use typed_builder::TypedBuilder;

    /// The properties of a phrase.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// Whether completions should be requested for the phrase as it is typed.
        #[builder(default)]
        pub completable: bool,
        /// The phrase to start with.
        #[builder(default, setter(into))]
        pub value: Option<String>,
        /// The character which the parts of the phrase are separated by, if it has parts.
        #[builder(default, setter(into))]
        pub part_separator: Option<char>,
    }
}
pub use props::Props;

/// Contains the [`Phrase`] component.
mod phrase {
    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::stateful::Stateful;

    use rend::{Fabric, Size, Yarn};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::Component;

    /// A phrase.
    #[derive(Default)]
    pub struct Phrase {
        /// The state of the phrase.
        state: State,
    }

    impl Component<Props, Event, Effect> for Phrase {
        fn new(props: Props) -> Self {
            Self {
                state: State::builder()
                    .value(props.value.unwrap_or_default())
                    .completable(props.completable)
                    .part_separator(props.part_separator)
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
                        key: Key::Backspace,
                        ..
                    }) => Some(Action::Pop),
                    TermEvent::KeyEvent(KeyEvent {
                        key: Key::Char('w'),
                        mods: KeyMods::CONTROL,
                        ..
                    }) => Some(Action::PopPart),
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
                    TermEvent::Paste(text) => Some(Action::Paste { text }),
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
            let background_color = Color::highlight(self.state.is_focused());
            yarn.background(background_color.into());
            Fabric::from(yarn)
        }
    }

    impl Phrase {
        /// Return what has been typed in.
        pub fn value(&self) -> &str {
            self.state.value()
        }
    }
}
pub use phrase::Phrase;

/// Contains the [`Event`] enum.
mod event {
    use term::TermEvent;

    use uuid::Uuid;

    /// A phrase event.
    #[allow(clippy::enum_variant_names)]
    pub enum Event {
        /// The phrase is being typed in.
        Focus,
        /// The phrase is no longer being typed in.
        Unfocus,
        /// A terminal event.
        TermEvent(TermEvent),
        /// A completion arrived.
        Completion {
            /// The unique identifier of the request.
            uuid: Uuid,
            /// The completion.
            completion: Option<String>,
        },
    }
}
pub use event::Event;

/// Contains the [`State`] struct.
mod state {
    use super::{Action, Effect};
    use crate::stateful::Stateful;

    use typed_builder::TypedBuilder;
    use uuid::Uuid;

    /// The state of a phrase.
    #[derive(TypedBuilder)]
    pub struct State {
        /// What has been typed in.
        #[builder(default, setter(into))]
        value: String,
        /// The completion.
        #[builder(default, setter(into))]
        completion: Option<String>,
        /// Whether the phrase is being typed in.
        #[builder(default = true, setter(into))]
        focus: bool,
        /// Whether completions should be requested for the phrase as it is typed.
        #[builder(default)]
        completable: bool,
        /// The character which the parts of the phrase are separated by, if it has parts.
        #[builder(default, setter(into))]
        part_separator: Option<char>,
        /// The pending request for a completion.
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
                part_separator: None,
                pending_completion_request: None,
            }
        }
    }

    impl State {
        /// Return what has been typed in.
        pub fn value(&self) -> &str {
            &self.value
        }

        /// Return the completion.
        pub fn completion(&self) -> &Option<String> {
            &self.completion
        }

        /// Return whether the phrase is being typed in.
        pub fn is_focused(&self) -> bool {
            self.focus
        }

        /// Start typing in the phrase.
        pub fn focus(&mut self) -> Option<Effect> {
            self.focus = true;
            None
        }

        /// Stop typing in the phrase.
        pub fn unfocus(&mut self) -> Option<Effect> {
            self.focus = false;
            None
        }

        /// If completable, request a completion for the current value.
        fn request_completion(&mut self) -> Option<Effect> {
            if !self.completable {
                return None;
            }

            // A value which ends with a separator has no part typed in to complete yet, so asking
            // would only waste a request. The completion which is held on to is from before the
            // separator, so it goes rather than being shown for a part it is not for.
            if let Some(separator) = self.part_separator {
                if self.value.ends_with(separator) {
                    self.completion = None;
                    self.pending_completion_request = None;
                    return None;
                }
            }

            let uuid: Uuid = Uuid::new_v4();
            self.pending_completion_request = Some(uuid);
            Some(Effect::RequestCompletion {
                uuid,
                partial: self.value.clone(),
            })
        }

        /// Add a character to the end of the phrase.
        fn push(&mut self, character: char) -> Option<Effect> {
            self.value.push(character);
            self.request_completion()
        }

        /// Append pasted text to the value.
        ///
        /// Only the first line of it is taken, because the value is a single line. The rest is
        /// dropped rather than run together with it or treated as an enter, either of which would
        /// do something surprising with a paste which spans lines.
        fn paste(&mut self, text: String) -> Option<Effect> {
            let line: &str = text.lines().next().unwrap_or_default();

            if line.is_empty() {
                return Some(Effect::Bell);
            }

            self.value.push_str(line);
            self.request_completion()
        }

        /// Take the last character off the phrase.
        fn pop(&mut self) -> Option<Effect> {
            self.value.pop();

            if self.value.is_empty() {
                self.completion = None;
                self.pending_completion_request = None;
                return None;
            }

            self.request_completion()
        }

        /// Take the last part off the phrase.
        ///
        /// A trailing separator belongs to the part before it, so it goes too. Otherwise pressing
        /// this once on a phrase which ends with one would only take that separator off.
        fn pop_part(&mut self) -> Option<Effect> {
            let separator: char = match self.part_separator {
                Some(separator) => separator,
                None => {
                    return Some(Effect::Bell);
                }
            };

            if self.value.is_empty() {
                return Some(Effect::Bell);
            }

            let end: usize = self.value.trim_end_matches(separator).len();
            let keep: usize = match self.value[..end].rfind(separator) {
                Some(index) => index + separator.len_utf8(),
                None => 0,
            };
            self.value.truncate(keep);

            if self.value.is_empty() {
                self.completion = None;
                self.pending_completion_request = None;
                return None;
            }

            self.request_completion()
        }

        /// Take note of a completion.
        fn set_completion(&mut self, uuid: Uuid, completion: Option<String>) -> Option<Effect> {
            if self.pending_completion_request != Some(uuid) {
                return None;
            }

            self.completion = completion;
            self.pending_completion_request = None;

            None
        }

        /// Take the completion as the phrase.
        fn complete(&mut self) -> Option<Effect> {
            if let Some(completion) = &self.completion {
                self.value = completion.to_string();
                self.completion = None;
            }
            None
        }

        /// Enter the phrase.
        fn find(&mut self) -> Option<Effect> {
            self.focus = false;
            Some(Effect::Enter {
                phrase: self.value.clone(),
            })
        }

        /// Quit.
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
                Action::Paste { text } => self.paste(text),
                Action::Pop => self.pop(),
                Action::PopPart => self.pop_part(),
                Action::SetCompletion { uuid, completion } => self.set_completion(uuid, completion),
                Action::Complete => self.complete(),
                Action::Enter => self.find(),
                Action::Quit => self.quit(),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use test_case::test_case;

        #[test_case("~/foo/bar/", "~/foo/"; "a trailing separator")]
        #[test_case("~/foo/bar", "~/foo/"; "no trailing separator")]
        #[test_case("~/foo/ba", "~/foo/"; "part of a part")]
        #[test_case("/home/", "/"; "the part above the root")]
        #[test_case("~/", ""; "the only part")]
        #[test_case("foo", ""; "no separator at all")]
        #[test_case("/", ""; "the root")]
        #[test_case("~/foo//", "~/"; "more than one trailing separator")]
        fn test_pop_part(value: &str, expected: &str) {
            let mut state: State = State::builder().value(value).part_separator('/').build();

            state.pop_part();

            assert_eq!(state.value(), expected);
        }

        #[test]
        fn test_pop_part_does_not_ask_for_a_completion() {
            let mut state: State = State::builder()
                .value("~/foo/bar")
                .completion(String::from("~/foo/barn/"))
                .completable(true)
                .part_separator('/')
                .build();

            let effect: Option<Effect> = state.pop_part();

            assert!(effect.is_none());
            assert_eq!(state.value(), "~/foo/");
            assert_eq!(state.completion(), &None);
        }

        #[test]
        fn test_pop_part_without_a_separator() {
            let mut state: State = State::builder().value("~/foo/bar").build();

            let effect: Option<Effect> = state.pop_part();

            assert!(matches!(effect, Some(Effect::Bell)));
            assert_eq!(state.value(), "~/foo/bar");
        }

        #[test]
        fn test_pop_part_of_nothing() {
            let mut state: State = State::builder().part_separator('/').build();

            let effect: Option<Effect> = state.pop_part();

            assert!(matches!(effect, Some(Effect::Bell)));
            assert_eq!(state.value(), "");
        }
    }
}
pub use state::State;

/// Contains the [`Action`] enum.
mod action {
    use uuid::Uuid;

    /// A phrase action.
    pub enum Action {
        /// Start typing in the phrase.
        Focus,
        /// Stop typing in the phrase.
        Unfocus,
        /// Add a character to the end of the phrase.
        Push {
            /// The character to add.
            character: char,
        },
        /// Add pasted text to the end of the phrase.
        Paste {
            /// The pasted text.
            text: String,
        },
        /// Take the last character off the phrase.
        Pop,
        /// Take the last part off the phrase.
        PopPart,
        /// Take note of a completion.
        SetCompletion {
            /// The unique identifier of the request.
            uuid: Uuid,
            /// The completion.
            completion: Option<String>,
        },
        /// Take the completion as the phrase.
        Complete,
        /// Enter the phrase.
        Enter,
        /// Quit.
        Quit,
    }
}
pub use action::Action;

/// Contains the [`Effect`] enum.
mod effect {
    use uuid::Uuid;

    /// A phrase effect.
    pub enum Effect {
        /// Ask for a completion for what has been typed in so far.
        RequestCompletion {
            /// The unique identifier to ask for it with.
            uuid: Uuid,
            /// What has been typed in so far.
            partial: String,
        },
        /// The phrase was entered.
        Enter {
            /// The phrase.
            phrase: String,
        },
        /// Ring the bell.
        Bell,
        /// Quit.
        Quit,
    }
}
pub use effect::Effect;

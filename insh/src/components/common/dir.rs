//! Contains the [`Dir`] component.

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use typed_builder::TypedBuilder;

    /// The properties of the directory.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// The directory to show.
        pub dir: PathBuf,
    }
}
pub use props::Props;

/// Contains the [`Dir`] component.
mod dir {
    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::stateful::Stateful;

    use rend::{Fabric, Size, Yarn};
    use til::Component;

    /// The directory bar.
    pub struct Dir {
        /// The state of the directory.
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
            // The phrase paints itself as focused, which is what shows that the directory is being
            // typed in.
            if let Some(phrase) = self.state.phrase() {
                return phrase.render(size);
            }

            let string = self.state.dir_string();
            let mut yarn = Yarn::from(string);
            yarn.resize(size.columns);
            yarn.color(Color::InvertedText.into());
            yarn.background(Color::InvertedBackground.into());

            Fabric::from(yarn)
        }
    }

    impl Dir {
        /// Return the action to perform for an event.
        fn map(&self, event: Event) -> Option<Action> {
            match event {
                Event::SetDir { dir } => Some(Action::SetDir { dir }),
                Event::PopDir => Some(Action::PopDir),
                Event::Focus => Some(Action::Focus),
                Event::TermEvent(event) => Some(Action::HandleTermEvent { event }),
                Event::Completion { uuid, completion } => {
                    Some(Action::SetCompletion { uuid, completion })
                }
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

/// Contains the [`Event`] enum.
mod event {
    use std::path::PathBuf;

    use term::TermEvent;

    use uuid::Uuid;

    /// A directory event.
    pub enum Event {
        /// Show a different directory.
        SetDir {
            /// The directory to show.
            dir: PathBuf,
        },
        /// Show the directory above.
        PopDir,
        /// The directory is being typed in.
        Focus,
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
    use std::path::{Path, PathBuf, MAIN_SEPARATOR as PATH_SEPARATOR};

    use super::{Action, Effect, Props};
    use crate::components::common::{Phrase, PhraseEffect, PhraseEvent, PhraseProps};
    use crate::current_dir;
    use crate::stateful::Stateful;

    use term::TermEvent;
    use til::Component;

    use uuid::Uuid;

    /// The state of the directory.
    pub struct State {
        /// The directory which is shown.
        dir: PathBuf,
        /// The home directory, which is shown as a tilde.
        home: Option<PathBuf>,
        /// What is being typed in, if the directory is being typed in.
        phrase: Option<Phrase>,
    }

    impl State {
        /// Return what is being typed in, if the directory is being typed in.
        pub fn phrase(&self) -> &Option<Phrase> {
            &self.phrase
        }

        /// Return the directory as it is shown.
        pub fn dir_string(&self) -> String {
            self.string(&self.dir)
        }

        /// Return a path as it is shown, which is with the home directory as a tilde and with a
        /// trailing path separator.
        fn string(&self, path: &Path) -> String {
            let mut string: String = match self
                .home
                .as_ref()
                .and_then(|home| path.strip_prefix(home).ok())
            {
                Some(rest) => {
                    let mut string = String::from("~");
                    string.push(PATH_SEPARATOR);
                    string.push_str(rest.to_str().unwrap());
                    string
                }
                None => path.to_str().unwrap().to_string(),
            };

            if !string.ends_with(PATH_SEPARATOR) {
                string.push(PATH_SEPARATOR);
            }

            string
        }

        /// Return the path which a string shows.
        fn path(&self, string: &str) -> PathBuf {
            let path: PathBuf = match &self.home {
                Some(home) => match string.strip_prefix('~') {
                    Some("") => home.clone(),
                    Some(rest) => match rest.strip_prefix(PATH_SEPARATOR) {
                        Some(rest) => home.join(rest),
                        None => PathBuf::from(string),
                    },
                    None => PathBuf::from(string),
                },
                None => PathBuf::from(string),
            };

            // A relative path is from the directory which is shown. It cannot be left relative,
            // because inshd runs from the root rather than from wherever insh was started, so the
            // two would not agree on what it means.
            let path: PathBuf = match path.is_absolute() {
                true => path,
                false => self.dir.join(path),
            };

            // Collecting the components is what takes off the trailing separator which the bar
            // shows and the completions come back with.
            return path.components().collect();
        }

        /// Show a different directory.
        fn set_dir(&mut self, dir: PathBuf) -> Option<Effect> {
            self.dir = dir;
            None
        }

        /// Show the directory above.
        fn pop_dir(&mut self) -> Option<Effect> {
            self.dir.pop();
            None
        }

        /// Start typing in the directory.
        fn focus(&mut self) -> Option<Effect> {
            let props = PhraseProps::builder()
                .value(self.dir_string())
                .completable(true)
                .part_separator(PATH_SEPARATOR)
                .build();
            self.phrase = Some(Phrase::new(props));
            None
        }

        /// Type in the directory.
        fn handle_term_event(&mut self, event: TermEvent) -> Option<Effect> {
            let phrase_effect: Option<PhraseEffect> = match &mut self.phrase {
                Some(phrase) => phrase.handle(PhraseEvent::TermEvent(event)),
                None => {
                    return Some(Effect::Bell);
                }
            };

            match phrase_effect {
                Some(PhraseEffect::RequestCompletion { uuid, partial }) => {
                    let partial: String = self.path(&partial).to_string_lossy().into_owned();
                    Some(Effect::RequestCompletion { uuid, partial })
                }
                Some(PhraseEffect::Enter { phrase }) => {
                    let dir: PathBuf = self.path(&phrase);

                    if !dir.is_dir() {
                        // Entering the phrase stops it from being typed in, so it has to be given
                        // back for the directory which is not one to be fixed.
                        if let Some(phrase) = &mut self.phrase {
                            phrase.handle(PhraseEvent::Focus);
                        }
                        return Some(Effect::Bell);
                    }

                    self.phrase = None;
                    self.dir = dir.clone();
                    Some(Effect::Enter { dir })
                }
                Some(PhraseEffect::Quit) => {
                    self.phrase = None;
                    Some(Effect::Unfocus)
                }
                Some(PhraseEffect::Bell) => Some(Effect::Bell),
                None => None,
            }
        }

        /// Take note of a completion.
        fn set_completion(&mut self, uuid: Uuid, completion: Option<String>) -> Option<Effect> {
            // Suggestions come back as absolute paths, which is how directories are stored, but
            // they are shown in the form which is being typed in. The ghost text is the part of a
            // suggestion past what has been typed and only lines up when the two agree, so a path
            // which is being typed out in full is left as one and every other one gets the tilde.
            let absolute: bool = match &self.phrase {
                Some(phrase) => phrase.value().starts_with(PATH_SEPARATOR),
                None => {
                    return None;
                }
            };

            let completion: Option<String> = match absolute {
                true => completion,
                false => completion.map(|path| self.string(&PathBuf::from(path))),
            };

            if let Some(phrase) = &mut self.phrase {
                phrase.handle(PhraseEvent::Completion { uuid, completion });
            }

            None
        }
    }

    impl Default for State {
        fn default() -> Self {
            let dir: PathBuf = current_dir::current_dir();
            let home: Option<PathBuf> = dirs::home_dir();
            State {
                dir,
                home,
                phrase: None,
            }
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
                Action::SetDir { dir } => self.set_dir(dir),
                Action::PopDir => self.pop_dir(),
                Action::Focus => self.focus(),
                Action::HandleTermEvent { event } => self.handle_term_event(event),
                Action::SetCompletion { uuid, completion } => self.set_completion(uuid, completion),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use test_case::test_case;

        /// Return a state with a known home directory, so that the tests do not depend on whose
        /// home directory they are run from.
        fn state() -> State {
            State {
                dir: PathBuf::from("/home/pat"),
                home: Some(PathBuf::from("/home/pat")),
                phrase: None,
            }
        }

        #[test_case("/home/pat", "~/"; "the home directory")]
        #[test_case("/home/pat/foo", "~/foo/"; "under the home directory")]
        #[test_case("/home/pat/foo/", "~/foo/"; "already separated")]
        #[test_case("/data", "/data/"; "outside the home directory")]
        #[test_case("/", "/"; "the root")]
        fn test_string(path: &str, expected: &str) {
            assert_eq!(state().string(&PathBuf::from(path)), expected);
        }

        #[test_case("~/", "/home/pat"; "the home directory")]
        #[test_case("~", "/home/pat"; "the home directory without a separator")]
        #[test_case("~/foo/", "/home/pat/foo"; "under the home directory")]
        #[test_case("/data/foo/", "/data/foo"; "outside the home directory")]
        #[test_case("~foo", "/home/pat/~foo"; "a tilde which is not the home directory")]
        #[test_case("foo/bar", "/home/pat/foo/bar"; "a relative path")]
        #[test_case("/", "/"; "the root")]
        fn test_path(string: &str, expected: &str) {
            assert_eq!(state().path(string), PathBuf::from(expected));
        }
    }
}
use state::State;

/// Contains the [`Action`] enum.
mod action {
    use std::path::PathBuf;

    use term::TermEvent;

    use uuid::Uuid;

    /// A directory action.
    pub enum Action {
        /// Show a different directory.
        SetDir {
            /// The directory to show.
            dir: PathBuf,
        },
        /// Show the directory above.
        PopDir,
        /// Start typing in the directory.
        Focus,
        /// Type in the directory.
        HandleTermEvent {
            /// The terminal event.
            event: TermEvent,
        },
        /// Take note of a completion.
        SetCompletion {
            /// The unique identifier of the request.
            uuid: Uuid,
            /// The completion.
            completion: Option<String>,
        },
    }
}
use action::Action;

/// Contains the [`Effect`] enum.
mod effect {
    use std::path::PathBuf;

    use uuid::Uuid;

    /// A directory effect.
    pub enum Effect {
        /// Ask for a completion for what has been typed in so far.
        RequestCompletion {
            /// The unique identifier to ask for it with.
            uuid: Uuid,
            /// What has been typed in so far.
            partial: String,
        },
        /// A directory was entered.
        Enter {
            /// The directory.
            dir: PathBuf,
        },
        /// The directory is no longer being typed in.
        Unfocus,
        /// Ring the bell.
        Bell,
    }
}
pub use effect::Effect;

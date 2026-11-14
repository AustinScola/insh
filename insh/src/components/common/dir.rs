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
    use std::path::Path;

    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::stateful::Stateful;

    use rend::{Cell, Fabric, Size, Yarn};
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
        /// Return the column which what is shown ends at.
        pub fn end(&self) -> usize {
            match self.state.phrase() {
                Some(phrase) => phrase.end(),
                None => Cell::columns(&self.state.dir_string()),
            }
        }

        /// Return the directory which is shown.
        pub fn dir(&self) -> &Path {
            self.state.dir()
        }

        /// Return the action to perform for an event.
        fn map(&self, event: Event) -> Option<Action> {
            match event {
                Event::SetDir { dir } => Some(Action::SetDir { dir }),
                Event::PopDir => Some(Action::PopDir),
                Event::Focus => Some(Action::Focus),
                Event::Unfocus => Some(Action::Unfocus),
                Event::SetPaneFocus { focused } => Some(Action::SetPaneFocus { focused }),
                Event::Press { column } => Some(Action::Press { column }),
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
        /// The directory is no longer being typed in, and what was typed is given up on.
        Unfocus,
        /// Whether what is typed goes to the pane the directory is in has changed.
        SetPaneFocus {
            /// Whether it does.
            focused: bool,
        },
        /// The directory was pressed on with the mouse.
        Press {
            /// The column it was pressed on.
            column: usize,
        },
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
    use crate::typed_path::TypedPath;

    use rend::Cell;
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
        /// Return the directory which is shown.
        pub fn dir(&self) -> &Path {
            &self.dir
        }

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
            return TypedPath::builder()
                .home(self.home.clone())
                .from(self.dir.clone())
                .build()
                .resolve(string);
        }

        /// Return the directory which the part of what is shown at the column is, which is nothing
        /// at all past the end of it.
        ///
        /// A separator goes with the part before it, so that pressing on the slash after the
        /// tilde is going home.
        fn dir_at(&self, shown: &str, column: usize) -> Option<PathBuf> {
            let mut end: usize = 0;
            for (at, character) in shown.char_indices() {
                end += Cell::columns(&shown[at..at + character.len_utf8()]);
                if character == PATH_SEPARATOR && end > column {
                    return Some(self.path(&shown[..=at]));
                }
            }

            None
        }

        /// Go to the directory whose part of what is shown was pressed on, and type in it from
        /// there, so that going up and then typing a way back down is one thing rather than two.
        ///
        /// While the directory is being typed in, what is shown is what has been typed, and a part
        /// of it is only gone to when it is a directory. Otherwise nothing is done when what was
        /// pressed on is the directory which is shown already, which is left to whatever the
        /// directory is in to decide about.
        fn press(&mut self, column: usize) -> Option<Effect> {
            let dir: PathBuf = match &self.phrase {
                Some(phrase) => self
                    .dir_at(phrase.value(), column)
                    .filter(|dir| dir.is_dir())?,
                None => self
                    .dir_at(&self.dir_string(), column)
                    .filter(|dir| *dir != self.dir)?,
            };

            self.dir = dir.clone();
            self.focus();
            Some(Effect::Enter { dir, typing: true })
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
                    Some(Effect::Enter { dir, typing: false })
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
                Action::Unfocus => {
                    self.phrase = None;
                    None
                }
                Action::SetPaneFocus { focused } => {
                    // Only what is being typed is ever colored as being typed in.
                    if let Some(phrase) = self.phrase.as_mut() {
                        phrase.handle(PhraseEvent::SetPaneFocus { focused });
                    }
                    None
                }
                Action::Press { column } => self.press(column),
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
        #[test_case("..", "/home"; "the directory above")]
        #[test_case("~/foo/../bar", "/home/pat/bar"; "going up partway through")]
        #[test_case("/..", "/"; "above the root")]
        #[test_case(".", "/home/pat"; "the directory itself")]
        #[test_case("/", "/"; "the root")]
        fn test_path(string: &str, expected: &str) {
            assert_eq!(state().path(string), PathBuf::from(expected));
        }

        #[test]
        fn test_pressing_on_a_part_of_what_is_being_typed_goes_to_it() {
            let mut state: State = state();
            state.dir = PathBuf::from("/tmp");
            state.focus();
            // Everything typed is rubbed out, and the root and a directory which is not there are
            // typed instead.
            for _ in 0.."/tmp/".len() {
                state.handle_term_event(TermEvent::KeyEvent(term::KeyEvent {
                    key: term::Key::Backspace,
                    mods: term::KeyMods::NONE,
                }));
            }
            for character in "/nowhere-at-all/".chars() {
                state.handle_term_event(TermEvent::KeyEvent(term::KeyEvent {
                    key: term::Key::Char(character),
                    mods: term::KeyMods::NONE,
                }));
            }

            // The part which is not a directory is not gone to, and the root is, which goes on
            // being typed in from there.
            assert!(state.press(5).is_none());
            assert!(matches!(
                state.press(0),
                Some(Effect::Enter { dir, typing: true }) if dir == PathBuf::from("/")
            ));
            assert_eq!(state.phrase().as_ref().map(Phrase::value), Some("/"));
        }

        #[test_case("/home/pat/foo/bar", 0, Some("/home/pat"); "the tilde")]
        #[test_case("/home/pat/foo/bar", 1, Some("/home/pat"); "the separator after the tilde")]
        #[test_case("/home/pat/foo/bar", 3, Some("/home/pat/foo"); "a part partway through")]
        #[test_case("/home/pat/foo/bar", 5, Some("/home/pat/foo"); "the separator after a part")]
        #[test_case("/home/pat/foo/bar", 7, None; "the directory which is shown already")]
        #[test_case("/home/pat/foo/bar", 20, None; "past the end")]
        #[test_case("/data/foo", 0, Some("/"); "the root")]
        #[test_case("/home/pat/😀/bar", 4, Some("/home/pat/😀"); "a part two columns wide")]
        fn test_press(dir: &str, column: usize, expected: Option<&str>) {
            let mut state: State = state();
            state.dir = PathBuf::from(dir);

            let entered: Option<PathBuf> = match state.press(column) {
                Some(Effect::Enter { dir, typing: true }) => Some(dir),
                _ => None,
            };

            assert_eq!(entered, expected.map(PathBuf::from));
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
        /// Stop typing in the directory, giving up on what was typed.
        Unfocus,
        /// Take note of whether what is typed goes to the pane the directory is in.
        SetPaneFocus {
            /// Whether it does.
            focused: bool,
        },
        /// Go to the directory whose part of what is shown was pressed on.
        Press {
            /// The column it was pressed on.
            column: usize,
        },
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
            /// Whether the directory is still being typed in, which it is when part of it was
            /// pressed on.
            typing: bool,
        },
        /// The directory is no longer being typed in.
        Unfocus,
        /// Ring the bell.
        Bell,
    }
}
pub use effect::Effect;

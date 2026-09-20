//! Contains the [`Input`] component.

/// Contains the [`Input`] component.
mod input {
    use super::{Action, Effect, Event, State};
    use crate::color::Color;
    use crate::stateful::Stateful;

    use rend::{Cell, Fabric, Size, Yarn};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::Component;

    /// The box which what to say is typed into.
    #[derive(Default)]
    pub struct Input {
        /// The state of the input.
        state: State,
    }

    impl Component<(), Event, Effect> for Input {
        fn new(_props: ()) -> Self {
            Self::default()
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Option<Action> = match event {
                Event::Clear => Some(Action::Clear),
                Event::TermEvent(TermEvent::Paste(text)) => Some(Action::Paste { text }),
                Event::TermEvent(TermEvent::KeyEvent(KeyEvent { key, mods })) => {
                    match (key, mods) {
                        (Key::Char('q'), KeyMods::CONTROL) => Some(Action::Leave),
                        (Key::CarriageReturn, KeyMods::NONE) => Some(Action::Send),
                        // Enter sends, so a new line is put in with this. Shift and enter cannot be
                        // told apart from enter on its own: a terminal sends the same byte for
                        // both unless it is asked to do otherwise, and asking changes what the
                        // other keys send.
                        (Key::Char('o'), KeyMods::CONTROL) => {
                            Some(Action::Push { character: '\n' })
                        }
                        (Key::Backspace, _) => Some(Action::Pop),
                        (Key::Char(character), KeyMods::NONE | KeyMods::SHIFT) => {
                            Some(Action::Push { character })
                        }
                        _ => None,
                    }
                }
                Event::TermEvent(_) => None,
            };

            match action {
                Some(action) => self.state.perform(action),
                None => None,
            }
        }

        fn render(&self, size: Size) -> Fabric {
            if size.rows == 0 || size.columns == 0 {
                return Fabric::new(size);
            }

            let lines: Vec<String> = Self::wrap(self.state.value(), size.columns);

            // The end of what is being typed is what is being looked at, so that is what stays in
            // view when there is more of it than there is room for.
            let shown: &[String] = match lines.len() > size.rows {
                true => &lines[lines.len() - size.rows..],
                false => &lines,
            };

            // The cursor is always here, so this is always the color for whatever has focus.
            let background: ansi::Color = Color::Highlight.into();

            let mut yarns: Vec<Yarn> = Vec::with_capacity(size.rows);
            for line in shown {
                let mut yarn: Yarn = Yarn::from(line.as_str());
                yarn.resize(size.columns);
                yarn.color(Color::InvertedText.into());
                yarn.background(background);
                yarns.push(yarn);
            }

            // The box keeps its shape when there is nothing in it, so the rows which are not
            // typed in yet are part of it rather than the transcript above.
            while yarns.len() < size.rows {
                let mut yarn: Yarn = Yarn::blank(size.columns);
                yarn.background(background);
                yarns.insert(0, yarn);
            }

            Fabric::from(yarns)
        }
    }

    impl Input {
        /// Return how many rows what is being typed needs at the given width.
        ///
        /// This is one to start with and grows as what is typed wraps, so the box is only as big
        /// as it has to be and the rest of the room goes to what has been said.
        pub fn rows(&self, columns: usize) -> usize {
            return Self::wrap(self.state.value(), columns).len().max(1);
        }

        /// Return what is being typed broken into lines which fit in a width.
        ///
        /// A line which is exactly as long as the width does not get an empty one after it, so the
        /// box only grows once there is something on the next row to show.
        fn wrap(value: &str, columns: usize) -> Vec<String> {
            if columns == 0 {
                return Vec::new();
            }

            let mut lines: Vec<String> = Vec::new();

            for paragraph in value.split('\n') {
                let mut line: String = String::new();

                for grapheme in Cell::all(paragraph) {
                    let grapheme: String = grapheme.to_string();
                    if Cell::columns(&line) + Cell::columns(&grapheme) > columns {
                        lines.push(std::mem::take(&mut line));
                    }
                    line.push_str(&grapheme);
                }

                lines.push(line);
            }

            lines
        }
    }
}
pub use input::Input;

/// Contains the [`Event`] enum.
mod event {
    use term::TermEvent;

    /// An input event.
    pub enum Event {
        /// A terminal event.
        TermEvent(TermEvent),
        /// Throw away what is being typed.
        Clear,
    }
}
pub use event::Event;

/// Contains the [`Effect`] enum.
mod effect {
    /// An input effect.
    pub enum Effect {
        /// What was typed should be said.
        Send {
            /// What was typed.
            message: String,
        },
        /// The input is no longer being typed in.
        Unfocus,
        /// Ring the bell.
        Bell,
    }
}
pub use effect::Effect;

/// Contains the [`Action`] enum.
mod action {
    /// An input action.
    pub enum Action {
        /// Leave the chat.
        Leave,
        /// Add a character to what is being typed.
        Push {
            /// The character to add.
            character: char,
        },
        /// Remove the last character of what is being typed.
        Pop,
        /// Add some text to what is being typed.
        Paste {
            /// The text to add.
            text: String,
        },
        /// Say what was typed.
        Send,
        /// Throw away what is being typed.
        Clear,
    }
}
use action::Action;

/// Contains the [`State`] struct.
mod state {
    use super::{Action, Effect};
    use crate::stateful::Stateful;

    /// The state of the input.
    #[derive(Default)]
    pub struct State {
        /// What is being typed.
        value: String,
    }

    impl State {
        /// Return what is being typed.
        pub fn value(&self) -> &str {
            &self.value
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Leave => Some(Effect::Unfocus),
                Action::Push { character } => {
                    self.value.push(character);
                    None
                }
                Action::Pop => {
                    match self.value.pop() {
                        Some(_) => None,
                        // There is nothing left to remove, so say so rather than doing nothing.
                        None => Some(Effect::Bell),
                    }
                }
                Action::Paste { text } => {
                    self.value.push_str(&text);
                    None
                }
                Action::Clear => {
                    self.value.clear();
                    None
                }
                Action::Send => {
                    let message: String = self.value.trim().to_string();
                    if message.is_empty() {
                        return Some(Effect::Bell);
                    }
                    self.value.clear();
                    Some(Effect::Send { message })
                }
            }
        }
    }
}
use state::State;

#[cfg(test)]
mod tests {
    use super::{Event, Input};

    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::Component;

    use test_case::test_case;

    /// Return an input with some text typed into it.
    fn typed(text: &str) -> Input {
        let mut input = Input::new(());

        for character in text.chars() {
            // A new line is put in with control and o, since enter sends what was typed.
            let (key, mods): (Key, KeyMods) = match character {
                '\n' => (Key::Char('o'), KeyMods::CONTROL),
                character => (Key::Char(character), KeyMods::NONE),
            };
            input.handle(Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
                key,
                mods,
            })));
        }

        input
    }

    #[test_case("", 10, 1; "nothing typed is still one row")]
    #[test_case("abc", 10, 1; "a short message is one row")]
    #[test_case("abcdefghij", 10, 1; "a message which exactly fills the width does not grow yet")]
    #[test_case("abcdefghijk", 10, 2; "a message which is one over wraps")]
    #[test_case("abcdefghijabcdefghijk", 10, 3; "a longer message wraps again")]
    #[test_case("a\nb", 10, 2; "a new line is a row")]
    #[test_case("a\nb\nc", 10, 3; "two new lines are two rows")]
    fn test_rows_grow_with_what_is_typed(text: &str, columns: usize, expected: usize) {
        assert_eq!(typed(text).rows(columns), expected);
    }
}

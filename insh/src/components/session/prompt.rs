//! Contains the [`Prompt`] component, which is where a command for the multiplexer is typed.

/// Contains the [`Asking`] enum.
mod asking {
    /// What the prompt is asking for.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Asking {
        /// A command for the multiplexer.
        Command,
        /// What to call the session.
        SessionName,
        /// What to call the window.
        WindowName,
    }

    impl Asking {
        /// Return what is shown before what has been typed.
        pub fn marker(&self) -> &'static str {
            return match self {
                Self::Command => ":",
                Self::SessionName => "(rename-session) ",
                Self::WindowName => "(rename-window) ",
            };
        }
    }
}
pub use asking::Asking;

/// Contains the [`Complaint`] struct.
mod complaint {
    /// What is said about what was typed when it could not be done.
    #[derive(Debug, PartialEq, Eq)]
    pub struct Complaint {
        /// What to say about it.
        pub message: String,
    }

    impl Complaint {
        /// Return a complaint which says the given thing.
        pub fn saying(message: impl Into<String>) -> Self {
            return Self {
                message: message.into(),
            };
        }
    }
}
pub use complaint::Complaint;

/// Contains the [`Words`] struct.
mod words {
    use super::Complaint;

    /// The words of a line typed at the prompt.
    ///
    /// A line is split the way a shell splits one rather than on whitespace alone, so that a name
    /// with a space in it can be given as one word by quoting it.
    #[derive(Debug, PartialEq, Eq)]
    pub struct Words {
        /// The words themselves.
        words: Vec<String>,
    }

    impl Words {
        /// Return the words of the line, which is nothing to complain about when it has none.
        pub fn of(line: &str) -> Result<Self, Complaint> {
            let mut words: Vec<String> = Vec::new();
            let mut word = String::new();
            // Whether a word has been begun, which quotes do without putting anything in it.
            let mut begun: bool = false;
            // Which quote the word is inside of, when it is inside one.
            let mut quote: Option<char> = None;
            // Whether the last character was the one which takes away what the next one means.
            let mut escaped: bool = false;

            for character in line.chars() {
                if escaped {
                    word.push(character);
                    escaped = false;
                    continue;
                }

                match (quote, character) {
                    // NOTE: A single quote takes nothing as meaning anything else, which is what
                    // makes it the one to quote something with a backslash in it with.
                    (Some('\''), '\\') => word.push('\\'),
                    (None | Some('"'), '\\') => {
                        escaped = true;
                        begun = true;
                    }
                    (None, '\'' | '"') => {
                        quote = Some(character);
                        begun = true;
                    }
                    (Some(open), character) if character == open => quote = None,
                    (None, character) if character.is_whitespace() => {
                        if begun {
                            words.push(std::mem::take(&mut word));
                            begun = false;
                        }
                    }
                    (_, character) => {
                        word.push(character);
                        begun = true;
                    }
                }
            }

            if escaped {
                return Err(Complaint::saying("Nothing follows the backslash"));
            }
            if let Some(open) = quote {
                return Err(Complaint::saying(format!("Unmatched {}", open)));
            }
            if begun {
                words.push(word);
            }

            return Ok(Self { words });
        }

        /// Return the words themselves.
        pub fn words(&self) -> &[String] {
            &self.words
        }

        /// Return whether the line had no words in it at all.
        pub fn is_empty(&self) -> bool {
            self.words.is_empty()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use test_case::test_case;

        #[test_case("one two", Ok(vec!["one", "two"]); "words apart")]
        #[test_case("  one   two  ", Ok(vec!["one", "two"]); "words apart with spaces around them")]
        #[test_case("'one two'", Ok(vec!["one two"]); "a word in single quotes")]
        #[test_case("\"one two\"", Ok(vec!["one two"]); "a word in double quotes")]
        #[test_case("one' 'two", Ok(vec!["one two"]); "quotes partway through a word")]
        #[test_case("one\\ two", Ok(vec!["one two"]); "a space taken as part of a word")]
        #[test_case("'\\'", Ok(vec!["\\"]); "a backslash in single quotes, which means only itself")]
        #[test_case("''", Ok(vec![""]); "nothing in quotes, which is a word all the same")]
        #[test_case("", Ok(vec![]); "nothing at all")]
        #[test_case("'one", Err(Complaint::saying("Unmatched '")); "a quote which is never closed")]
        #[test_case("one\\", Err(Complaint::saying("Nothing follows the backslash")); "a backslash with nothing after it")]
        fn test_of(line: &str, expected: Result<Vec<&str>, Complaint>) {
            let result = Words::of(line).map(|words| words.words().to_vec());

            assert_eq!(
                result,
                expected.map(|words| words
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>())
            );
        }
    }
}
pub use words::Words;

/// Contains the [`Command`] enum.
mod command {
    use super::{Complaint, Words};
    use crate::string::CapitalizeFirstLetterExt;

    use clap::error::{ContextKind, ContextValue, ErrorKind};
    use clap::{Error as ClapError, Parser, Subcommand, ValueEnum};

    /// What was typed at the prompt, which is a command along with whatever was given to it.
    ///
    /// NOTE: This is what clap is given the words to fill in. The prompt is not a command line, so
    /// nothing about it is laid out for a terminal: there is no help, since there is nowhere in a
    /// one row prompt to put it, and what is complained about is cut down to the one line there is
    /// room for.
    #[derive(Parser, Debug)]
    #[command(
        no_binary_name = true,
        disable_help_flag = true,
        disable_help_subcommand = true,
        disable_version_flag = true
    )]
    struct Line {
        /// The command which was typed.
        #[command(subcommand)]
        command: Command,
    }

    /// What a pane shows other than the panes themselves.
    #[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Mode {
        /// Looking back through a pane and taking from it.
        Copy,
        /// Typing a command for the multiplexer.
        Prompt,
    }

    /// A command for the multiplexer.
    ///
    /// Every key the multiplexer takes does one of these, so that there is nothing which can be
    /// done with a key which cannot be typed at the prompt, and nothing at the prompt which has to
    /// be written twice to be bound to a key as well.
    #[derive(Subcommand, Clone, Debug, PartialEq, Eq)]
    pub enum Command {
        /// Split the pane what is typed goes to, side by side.
        SplitVertically,
        /// Split the pane what is typed goes to, one above the other.
        SplitHorizontally,
        /// Close the pane what is typed goes to.
        KillPane,
        /// Send what is typed to the pane below.
        FocusDown,
        /// Send what is typed to the pane above.
        FocusUp,
        /// Send what is typed to the pane on the left.
        FocusLeft,
        /// Send what is typed to the pane on the right.
        FocusRight,
        /// Move the line below the pane what is typed goes to, a row down.
        ResizePaneDown,
        /// Move the line below the pane what is typed goes to, a row up.
        ResizePaneUp,
        /// Move the line beside the pane what is typed goes to, a column left.
        ResizePaneLeft,
        /// Move the line beside the pane what is typed goes to, a column right.
        ResizePaneRight,
        /// Send what is typed to every pane of the window at once, or stop.
        SynchronizePanes,
        /// Make the pane what is typed goes to fill the window, or put the window back.
        ToggleZoom,
        /// Swap the pane what is typed goes to with the one before it.
        SwitchPanePrevious,
        /// Swap the pane what is typed goes to with the one after it.
        SwitchPaneNext,
        /// Say what the session is called.
        RenameSession {
            /// What to call it, which is asked for when it is not given.
            name: Option<String>,
        },
        /// Say what the window is called.
        RenameWindow {
            /// What to call it, which is asked for when it is not given.
            name: Option<String>,
        },
        /// Make a new session.
        NewSession {
            /// What to call it, which is a name made up from the ones already taken when there is
            /// none.
            name: Option<String>,
        },
        /// Make a new window.
        NewWindow {
            /// What to call it, which is a name made up from the ones already taken when there is
            /// none.
            name: Option<String>,
        },
        /// Show something other than the panes.
        ChangeMode {
            /// What to show.
            mode: Mode,
        },
        /// List the sessions and the windows in them.
        #[command(alias = "ls")]
        ListSessions,
        /// Stop showing the session.
        Detach,
    }

    impl Command {
        /// Return the command which the text is, which is nothing at all when nothing was typed.
        pub fn of(text: &str) -> Result<Option<Self>, Complaint> {
            let words: Words = Words::of(text)?;
            // Nothing was typed at all, which is not something to complain about.
            if words.is_empty() {
                return Ok(None);
            }

            return match Line::try_parse_from(words.words()) {
                Ok(line) => Ok(Some(line.command)),
                Err(error) => Err(Self::complaint(&error)),
            };
        }

        /// Return what to say about what clap could not take.
        ///
        /// What clap says is written for a terminal, so it runs to several lines and ends by
        /// saying to ask for help. The first line of it is the one which says what was wrong, and
        /// the ones it words poorly for a prompt are worded again here.
        fn complaint(error: &ClapError) -> Complaint {
            let typed: Option<&str> = match error.get(ContextKind::InvalidSubcommand) {
                Some(ContextValue::String(typed)) => Some(typed.as_str()),
                _ => None,
            };

            return match (error.kind(), typed) {
                (ErrorKind::InvalidSubcommand, Some(typed)) => {
                    Complaint::saying(format!("Unknown command: {}", typed))
                }
                (ErrorKind::InvalidSubcommand | ErrorKind::MissingSubcommand, None) => {
                    Complaint::saying("Unknown command")
                }
                _ => Complaint::saying(Self::first_line(error)),
            };
        }

        /// Return the first line of what clap said, without the word it puts before it.
        ///
        /// It is capitalized because clap does not capitalize what it says and everything else
        /// which is complained about here is.
        fn first_line(error: &ClapError) -> String {
            let said: String = error.render().to_string();
            let line: &str = said.lines().next().unwrap_or_default();

            return line
                .trim_start_matches("error: ")
                .to_string()
                .capitalize_first_letter();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use test_case::test_case;

        #[test_case("split-vertically", Ok(Some(Command::SplitVertically)); "splitting side by side")]
        #[test_case("split-horizontally", Ok(Some(Command::SplitHorizontally)); "splitting one above the other")]
        #[test_case("kill-pane", Ok(Some(Command::KillPane)); "closing a pane")]
        #[test_case("focus-down", Ok(Some(Command::FocusDown)); "the pane below")]
        #[test_case("focus-up", Ok(Some(Command::FocusUp)); "the pane above")]
        #[test_case("focus-left", Ok(Some(Command::FocusLeft)); "the pane on the left")]
        #[test_case("focus-right", Ok(Some(Command::FocusRight)); "the pane on the right")]
        #[test_case("resize-pane-down", Ok(Some(Command::ResizePaneDown)); "the line below, down")]
        #[test_case("resize-pane-up", Ok(Some(Command::ResizePaneUp)); "the line below, up")]
        #[test_case("resize-pane-left", Ok(Some(Command::ResizePaneLeft)); "the line beside, left")]
        #[test_case("resize-pane-right", Ok(Some(Command::ResizePaneRight)); "the line beside, right")]
        #[test_case("synchronize-panes", Ok(Some(Command::SynchronizePanes)); "typing in every pane at once")]
        #[test_case("toggle-zoom", Ok(Some(Command::ToggleZoom)); "filling the window with a pane")]
        #[test_case("switch-pane-previous", Ok(Some(Command::SwitchPanePrevious)); "swapping with the pane before")]
        #[test_case("switch-pane-next", Ok(Some(Command::SwitchPaneNext)); "swapping with the pane after")]
        #[test_case("rename-session", Ok(Some(Command::RenameSession { name: None })); "asking what to call the session")]
        #[test_case("rename-session work", Ok(Some(Command::RenameSession { name: Some(String::from("work")) })); "saying what to call the session")]
        #[test_case("rename-window", Ok(Some(Command::RenameWindow { name: None })); "asking what to call the window")]
        #[test_case("rename-window notes", Ok(Some(Command::RenameWindow { name: Some(String::from("notes")) })); "saying what to call the window")]
        #[test_case("new-session", Ok(Some(Command::NewSession { name: None })); "a session")]
        #[test_case("new-session work", Ok(Some(Command::NewSession { name: Some(String::from("work")) })); "a session with a name")]
        #[test_case("new-session 'the work one'", Ok(Some(Command::NewSession { name: Some(String::from("the work one")) })); "a session with a name with a space in it")]
        #[test_case("new-window", Ok(Some(Command::NewWindow { name: None })); "a window")]
        #[test_case("new-window notes", Ok(Some(Command::NewWindow { name: Some(String::from("notes")) })); "a window with a name")]
        #[test_case("change-mode copy", Ok(Some(Command::ChangeMode { mode: Mode::Copy })); "looking back through a pane")]
        #[test_case("change-mode prompt", Ok(Some(Command::ChangeMode { mode: Mode::Prompt })); "typing a command")]
        #[test_case("list-sessions", Ok(Some(Command::ListSessions)); "the sessions")]
        #[test_case("ls", Ok(Some(Command::ListSessions)); "the sessions, said the short way")]
        #[test_case("detach", Ok(Some(Command::Detach)); "detaching")]
        #[test_case("   new-window  ", Ok(Some(Command::NewWindow { name: None })); "with spaces around it")]
        #[test_case("", Ok(None); "nothing at all")]
        #[test_case("   ", Ok(None); "nothing but spaces")]
        #[test_case("wibble", Err(Complaint::saying("Unknown command: wibble")); "a word which is not a command")]
        #[test_case("change-mode", Err(Complaint::saying("The following required arguments were not provided:")); "a mode is asked for but not given")]
        #[test_case("change-mode wibble", Err(Complaint::saying("Invalid value 'wibble' for '<MODE>'")); "a mode which there is none of")]
        #[test_case("toggle-zoom wibble", Err(Complaint::saying("Unexpected argument 'wibble' found")); "a word which the command has no use for")]
        #[test_case("new-session 'work", Err(Complaint::saying("Unmatched '")); "a quote which is never closed")]
        fn test_of(typed: &str, expected: Result<Option<Command>, Complaint>) {
            assert_eq!(Command::of(typed), expected);
        }
    }
}
pub use command::{Command, Mode};

/// Contains the [`Prompt`] component.
mod prompt {
    use super::{Asking, Command, Complaint};
    use crate::color::Color;

    use insh_api::{Request, RequestParams, Response, ResponseParams, SuggestCommandRequestParams};
    use rend::{Fabric, Size, Yarn};
    use term::{Key, KeyEvent, KeyMods, TermEvent};

    use uuid::Uuid;

    /// What the prompt has been asked to do.
    pub enum Effect {
        /// Ask the daemon for something.
        Request(Request),
        /// Do the command which was typed, and take note that it was run.
        Run {
            /// The command itself.
            command: Command,
            /// What was typed to run it, which is what is taken note of.
            typed: String,
        },
        /// Call what was being named what was typed.
        Name {
            /// What was being named.
            what: Asking,
            /// What to call it.
            name: String,
        },
        /// Stop showing the prompt.
        Close,
    }

    /// Where a command for the multiplexer, or what to call something, is typed.
    pub struct Prompt {
        /// What it is asking for.
        asking: Asking,
        /// What has been typed so far.
        typed: String,
        /// What was said about the last thing which was typed, when it was not a command.
        complaint: Option<String>,
        /// What the daemon suggested the rest of what has been typed is.
        suggestion: Option<String>,
        /// The request which the suggestion being shown is for.
        ///
        /// NOTE: A suggestion is asked for on every key, so the one which is shown is the one the
        /// last request was answered with and whatever answers an older one is let go of.
        suggesting: Option<Uuid>,
    }

    impl Prompt {
        /// Return a prompt which asks for the given thing, starting with what is already typed.
        pub fn asking(asking: Asking, typed: String) -> Self {
            return Self {
                asking,
                typed,
                complaint: None,
                suggestion: None,
                suggesting: None,
            };
        }

        /// Take note of what the daemon suggested, which is ignored unless it was asked for last.
        pub fn handle_response(&mut self, response: &Response) {
            if Some(*response.uuid()) != self.suggesting {
                return;
            }

            if let ResponseParams::SuggestCommand(params) = response.params() {
                self.suggestion = params.suggestion().clone();
            }
        }

        /// Do what the key says.
        pub fn handle(&mut self, term_event: TermEvent) -> Option<Effect> {
            let key_event: KeyEvent = match term_event {
                TermEvent::KeyEvent(key_event) => key_event,
                _ => return None,
            };

            match (key_event.key, key_event.mods) {
                (Key::Escape, _) => return Some(Effect::Close),
                (Key::Char('c'), KeyMods::CONTROL) => return Some(Effect::Close),
                (Key::CarriageReturn, _) => return self.run(),
                (Key::Char('u'), KeyMods::CONTROL) => {
                    self.typed.clear();
                    self.complaint = None;
                }
                (Key::Char('w'), KeyMods::CONTROL) => {
                    self.rub_out_word();
                    self.complaint = None;
                }
                (Key::Backspace | Key::Delete, _) => {
                    // NOTE: Rubbing out what is not there does nothing at all rather than giving
                    // up on the prompt, so that holding the key down is safe. Escape is what
                    // gives up on it.
                    self.typed.pop();
                    self.complaint = None;
                }
                (Key::HorizontalTab, _) => {
                    if let Some(completion) = self.completion() {
                        self.typed = completion;
                    }
                }
                // NOTE: A character is only typed when nothing but shift is held with it, since
                // shift is what a character like `%` is typed with. A chord which means nothing
                // here does nothing rather than putting the letter of it in what was typed.
                (Key::Char(character), mods)
                    if !mods.intersects(KeyMods::CONTROL | KeyMods::ALT | KeyMods::SUPER) =>
                {
                    self.typed.push(character);
                    self.complaint = None;
                }
                _ => return None,
            }

            self.suggest()
        }

        /// Return what asking the daemon what the rest of what has been typed is asks for.
        ///
        /// Nothing is asked for a prompt which is asking what to call something, since what to
        /// call something is not something there is any suggesting.
        fn suggest(&mut self) -> Option<Effect> {
            if self.asking != Asking::Command {
                return None;
            }

            // The suggestion which is up is for less than has been typed now, so it is let go of
            // rather than left showing until the daemon answers.
            self.suggestion = None;

            let request = Request::builder()
                .params(RequestParams::SuggestCommand(
                    SuggestCommandRequestParams::builder()
                        .partial(self.typed.clone())
                        .build(),
                ))
                .build();
            self.suggesting = Some(*request.uuid());

            return Some(Effect::Request(request));
        }

        /// Return what has been typed so far.
        #[cfg(test)]
        pub fn typed(&self) -> &str {
            &self.typed
        }

        /// Rub out the last word of what was typed, along with the spaces before it.
        fn rub_out_word(&mut self) {
            let kept: usize = self
                .typed
                .trim_end()
                .rfind(char::is_whitespace)
                .map_or(0, |at| at + 1);
            self.typed.truncate(kept);
        }

        /// Return the whole of what the daemon suggested what has been typed is the start of, which
        /// is nothing at all when it suggested nothing or when all of it has been typed already.
        fn completion(&self) -> Option<String> {
            let suggestion: &String = self.suggestion.as_ref()?;
            if suggestion.len() <= self.typed.trim_start().len() {
                return None;
            }

            return Some(suggestion.clone());
        }

        /// Return what doing what was typed asks for.
        fn run(&mut self) -> Option<Effect> {
            if self.asking != Asking::Command {
                // Nothing is called nothing at all, so an empty name only closes the prompt.
                let name: String = self.typed.trim().to_string();
                return match name.is_empty() {
                    true => Some(Effect::Close),
                    false => Some(Effect::Name {
                        what: self.asking,
                        name,
                    }),
                };
            }

            match Command::of(&self.typed) {
                Ok(Some(command)) => Some(Effect::Run {
                    command,
                    typed: self.typed.trim().to_string(),
                }),
                // Nothing was typed, so there is nothing to do but stop.
                Ok(None) => Some(Effect::Close),
                Err(Complaint { message }) => {
                    self.complaint = Some(message);
                    self.typed.clear();
                    self.suggestion = None;
                    self.suggesting = None;
                    None
                }
            }
        }

        /// Draw the prompt, which is one row along the bottom where the footer usually is.
        ///
        /// It is written on the color which everything which stands out is written on rather than
        /// the one the footer is, so that it is plain that what is typed is going to the prompt
        /// and not to a pane.
        pub fn render(&self, size: Size) -> Fabric {
            if let Some(complaint) = &self.complaint {
                let mut yarn = Yarn::from(complaint.clone());
                yarn.resize(size.columns);
                yarn.color(Color::InvertedText.into());
                yarn.background(Color::Highlight.into());

                return Fabric::from(yarn);
            }

            let mut yarn = Yarn::from(format!("{}{}", self.asking.marker(), self.typed));
            yarn.color(Color::InvertedText.into());

            // The rest of the command which has been started is shown greyed ahead of what has
            // been typed, which tab takes. It is the only one it could be, so there is never more
            // than one of them to choose between.
            if let Some(completion) = self.completion() {
                if let Some(rest) = completion.strip_prefix(self.typed.trim_start()) {
                    let mut rest_yarn = Yarn::from(rest);
                    rest_yarn.color(Color::InvertedGrayedText.into());
                    yarn = yarn.concat(rest_yarn);
                }
            }

            yarn.resize(size.columns);
            yarn.background(Color::Highlight.into());

            return Fabric::from(yarn);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use test_case::test_case;

        #[test_case(Key::Char('u'), KeyMods::CONTROL, ""; "clearing the line")]
        #[test_case(Key::Char('w'), KeyMods::CONTROL, "new-session "; "rubbing out the last word, which leaves the space before it")]
        #[test_case(Key::Backspace, KeyMods::NONE, "new-session wor"; "rubbing out a character")]
        #[test_case(Key::Char('%'), KeyMods::SHIFT, "new-session work%"; "a character typed with shift")]
        #[test_case(Key::Char('a'), KeyMods::CONTROL, "new-session work"; "a chord which means nothing here")]
        #[test_case(Key::Char('a'), KeyMods::ALT, "new-session work"; "a chord held with alt")]
        fn test_handle(key: Key, mods: KeyMods, expected: &str) {
            let mut prompt = Prompt::asking(Asking::Command, String::from("new-session work"));

            prompt.handle(TermEvent::KeyEvent(KeyEvent { key, mods }));

            assert_eq!(prompt.typed(), expected);
        }

        #[test_case(Key::Escape, KeyMods::NONE; "escape")]
        #[test_case(Key::Char('c'), KeyMods::CONTROL; "control and c")]
        fn test_handle_closes(key: Key, mods: KeyMods) {
            let mut prompt = Prompt::asking(Asking::Command, String::from("new-session work"));

            let effect = prompt.handle(TermEvent::KeyEvent(KeyEvent { key, mods }));

            assert!(matches!(effect, Some(Effect::Close)));
        }
    }
}
pub use prompt::{Effect, Prompt};

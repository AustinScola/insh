//! Contains the [`Transcript`] component.

/// Contains the [`Transcript`] component.
mod transcript {
    use super::{Action, Effect, Event, Message, State};
    use crate::color::Color;
    use crate::stateful::Stateful;

    use crate::markdown::Markdown;

    use rend::{Cell, Fabric, Size, Yarn};

    use std::cell::RefCell;
    use til::Component;

    /// What has been said in a chat.
    #[derive(Default)]
    pub struct Transcript {
        /// The state of the transcript.
        state: State,
        /// Reads the markdown which replies are written in.
        ///
        /// Drawing is done from what is there rather than by being told to, so this is only
        /// reachable through a shared reference. It holds the grammars, which are slow to read and
        /// worth keeping.
        markdown: RefCell<Markdown>,
    }

    impl Component<(), Event, Effect> for Transcript {
        fn new(_props: ()) -> Self {
            Self::default()
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Option<Action> = match event {
                Event::ScrollUp => Some(Action::Up {
                    max: self.max_offset(),
                }),
                Event::ScrollDown => Some(Action::Down),
                Event::Show { messages, error } => Some(Action::Show { messages, error }),
                Event::Said { message } => Some(Action::Said { message }),
                Event::Delta { uuid, text, error } => Some(Action::Delta { uuid, text, error }),
                Event::Expect { uuid } => Some(Action::Expect { uuid }),
                Event::Ended { uuid } => Some(Action::Ended { uuid }),
                Event::Thinking { uuid, word } => Some(Action::Thinking { uuid, word }),
                Event::Resize { size } => Some(Action::Resize { size }),
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

            let lines: Vec<Yarn> = self.lines(size.columns);

            // The newest is what matters, so the view sits at the bottom unless it was scrolled.
            //
            // How far back it has been scrolled is held to what there is to scroll, which is not
            // the same as what it was when the scrolling happened: the box which what to say is
            // typed into grows as it is typed in, and takes these rows as it does. Without this
            // the view would be asked to start past the oldest line and would show nothing.
            let offset: usize = self
                .state
                .offset()
                .min(lines.len().saturating_sub(size.rows));
            let end: usize = lines.len().saturating_sub(offset);
            let start: usize = end.saturating_sub(size.rows);

            let mut fabric: Fabric = Fabric::from(lines[start..end].to_vec());
            fabric.pad_bottom(size.rows);
            fabric
        }
    }

    impl Transcript {
        /// Return how far back the view is allowed to go.
        ///
        /// This is however many lines there are beyond the ones which fit, so that scrolling stops
        /// with the oldest line at the top rather than carrying on into nothing.
        fn max_offset(&self) -> usize {
            let size: Size = self.state.size();
            if size.columns == 0 {
                return 0;
            }

            self.lines(size.columns).len().saturating_sub(size.rows)
        }

        /// Return the lines of every message, wrapped to a width.
        fn lines(&self, columns: usize) -> Vec<Yarn> {
            let mut lines: Vec<Yarn> = Vec::new();

            let mut markdown = self.markdown.borrow_mut();

            for message in self.state.messages() {
                // A reply is put in place empty as soon as one is asked for, so that the pieces
                // have somewhere to go. There is nothing to show for it until the first of them
                // arrives, and a blank row under what was just said reads as an answer of nothing.
                //
                // A reply which ends in a new line is trimmed for the same reason.
                let text: &str = message.text().trim_end_matches('\n');
                if text.is_empty() {
                    continue;
                }

                match message {
                    // What you typed is shown as you typed it. Reading it as markdown would take
                    // apart something which was never meant as markdown.
                    Message::Said(_) => {
                        for line in Self::wrap(text, columns) {
                            let mut yarn: Yarn = Yarn::from(line);
                            yarn.resize(columns);
                            // What you said is set apart by what it is on.
                            yarn.color(Color::InvertedText.into());
                            yarn.background(Color::SaidBackground.into());
                            lines.push(yarn);
                        }
                    }
                    Message::Heard(_) => lines.extend(markdown.render(text, columns)),
                }
            }

            // What the engine is up to sits at the end of what has been said while it is up to it,
            // and goes away once there is an answer.
            if let Some(word) = self.state.thinking() {
                let mut yarn: Yarn = Yarn::from(format!("{}…", word));
                yarn.resize(columns);
                yarn.color(Color::GrayedText.into());
                lines.push(yarn);
            }

            if let Some(error) = self.state.error() {
                let mut yarn: Yarn = Yarn::from(error.clone());
                yarn.resize(columns);
                yarn.color(Color::BadRegex.into());
                lines.push(yarn);
            }

            lines
        }

        /// Return some text broken into lines which fit in a width.
        fn wrap(text: &str, columns: usize) -> Vec<String> {
            if columns == 0 {
                return Vec::new();
            }

            let mut lines: Vec<String> = Vec::new();

            for paragraph in text.split('\n') {
                let mut line: String = String::new();

                for word in paragraph.split(' ') {
                    // A word with no space in it which is wider than the line has nowhere to
                    // wrap, so it is broken where it runs out of room rather than losing
                    // everything past the edge.
                    for piece in Self::pieces(word, columns) {
                        let width: usize = Cell::columns(&piece);
                        let taken: usize = Cell::columns(&line);

                        if !line.is_empty() && taken + 1 + width > columns {
                            lines.push(std::mem::take(&mut line));
                        }

                        if !line.is_empty() {
                            line.push(' ');
                        }
                        line.push_str(&piece);
                    }
                }

                lines.push(line);
            }

            lines
        }

        /// Break a word into the pieces which fit in a width.
        ///
        /// A word which fits is one piece, which is the usual case. One which does not is cut at
        /// the column it runs out of room at, counting the columns each cluster is written in so
        /// that a wide one is not split down the middle.
        fn pieces(word: &str, columns: usize) -> Vec<String> {
            if columns == 0 || Cell::columns(word) <= columns {
                return vec![word.to_string()];
            }

            let mut pieces: Vec<String> = Vec::new();
            let mut piece: String = String::new();
            let mut taken: usize = 0;

            for cell in Cell::all(word) {
                let cluster: String = cell.to_string();
                let width: usize = Cell::columns(&cluster);

                if taken + width > columns {
                    pieces.push(std::mem::take(&mut piece));
                    taken = 0;
                }

                piece.push_str(&cluster);
                taken += width;
            }

            if !piece.is_empty() {
                pieces.push(piece);
            }

            pieces
        }
    }
}
pub use transcript::Transcript;

/// Contains the [`Message`] enum.
mod message {
    /// Something which was said in a chat.
    pub enum Message {
        /// What the person using insh said.
        Said(String),
        /// What the inference engine said.
        Heard(String),
    }

    impl Message {
        /// Return what was said.
        pub fn text(&self) -> &str {
            match self {
                Self::Said(text) => text,
                Self::Heard(text) => text,
            }
        }

        /// Add to what was said.
        pub fn push(&mut self, text: &str) {
            match self {
                Self::Said(said) => said.push_str(text),
                Self::Heard(heard) => heard.push_str(text),
            }
        }
    }
}
pub use message::Message;

/// Contains the [`Event`] enum.
mod event {
    use rend::Size;
    use uuid::Uuid;

    /// A transcript event.
    pub enum Event {
        /// Show what was said before what is shown.
        ScrollUp,
        /// Show what was said after what is shown.
        ScrollDown,
        /// Show the messages of a chat which was opened.
        Show {
            /// The messages, the oldest first.
            messages: Vec<(bool, String)>,
            /// What went wrong reading the chat, if anything did.
            error: Option<String>,
        },
        /// Something was said.
        Said {
            /// What was said.
            message: String,
        },
        /// A reply is coming.
        Expect {
            /// The unique identifier of the request the reply is to.
            uuid: Uuid,
        },
        /// A piece of a reply arrived.
        Delta {
            /// The unique identifier of the request the reply is to.
            uuid: Uuid,
            /// The text which arrived.
            text: String,
            /// What went wrong, if anything did.
            error: Option<String>,
        },
        /// A reply is over.
        Ended {
            /// The unique identifier of the request the reply was to.
            uuid: Uuid,
        },
        /// The engine started or stopped thinking before answering.
        Thinking {
            /// The unique identifier of the request the reply is to.
            uuid: Uuid,
            /// What to call what it is doing, or nothing when it is no longer doing it.
            word: Option<String>,
        },
        /// The transcript was resized.
        Resize {
            /// The new size.
            size: Size,
        },
    }
}
pub use event::Event;

/// Contains the [`Effect`] enum.
mod effect {
    /// A transcript effect.
    pub enum Effect {
        /// Ring the bell.
        Bell,
    }
}
pub use effect::Effect;

/// Contains the [`Action`] enum.
mod action {
    use rend::Size;
    use uuid::Uuid;

    /// A transcript action.
    pub enum Action {
        /// Show the messages of a chat which was opened.
        Show {
            /// The messages, the oldest first.
            messages: Vec<(bool, String)>,
            /// What went wrong reading the chat, if anything did.
            error: Option<String>,
        },
        /// Record something which was said.
        Said {
            /// What was said.
            message: String,
        },
        /// Expect a reply.
        Expect {
            /// The unique identifier of the request the reply is to.
            uuid: Uuid,
        },
        /// Add a piece of a reply.
        Delta {
            /// The unique identifier of the request the reply is to.
            uuid: Uuid,
            /// The text which arrived.
            text: String,
            /// What went wrong, if anything did.
            error: Option<String>,
        },
        /// Finish a reply.
        Ended {
            /// The unique identifier of the request the reply was to.
            uuid: Uuid,
        },
        /// Take note of the engine starting or stopping thinking.
        Thinking {
            /// The unique identifier of the request the reply is to.
            uuid: Uuid,
            /// What to call what it is doing, or nothing when it is no longer doing it.
            word: Option<String>,
        },
        /// Scroll back through the transcript.
        Up {
            /// How far back the view is allowed to go.
            max: usize,
        },
        /// Scroll forward through the transcript.
        Down,
        /// The transcript was resized.
        Resize {
            /// The new size.
            size: Size,
        },
    }
}
use action::Action;

/// Contains the [`State`] struct.
mod state {
    use super::{Action, Effect, Message};
    use crate::stateful::Stateful;

    use rend::Size;
    use uuid::Uuid;

    /// The state of the transcript.
    #[derive(Default)]
    pub struct State {
        /// What has been said, the oldest first.
        messages: Vec<Message>,
        /// The request whose reply is being waited for.
        pending_request: Option<Uuid>,
        /// What went wrong, if anything did.
        error: Option<String>,
        /// What the engine is up to before it answers, while it is up to it.
        thinking: Option<String>,
        /// How many lines back from the newest the view is.
        offset: usize,
        /// The size the transcript was last drawn at, which is what bounds the scrolling.
        size: Size,
    }

    impl State {
        /// Return the size the transcript was last drawn at.
        pub fn size(&self) -> Size {
            self.size
        }

        /// Return what has been said.
        pub fn messages(&self) -> &Vec<Message> {
            &self.messages
        }

        /// Return what went wrong, if anything did.
        pub fn error(&self) -> Option<&String> {
            self.error.as_ref()
        }

        /// Return what the engine is up to before it answers, while it is up to it.
        pub fn thinking(&self) -> Option<&String> {
            self.thinking.as_ref()
        }

        /// Return how many lines back from the newest the view is.
        pub fn offset(&self) -> usize {
            self.offset
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Show { messages, error } => {
                    self.thinking = None;
                    self.messages = messages
                        .into_iter()
                        .map(|(mine, text)| match mine {
                            true => Message::Said(text),
                            false => Message::Heard(text),
                        })
                        .collect();
                    self.pending_request = None;
                    self.error = error;
                    self.offset = 0;
                    None
                }
                Action::Said { message } => {
                    self.messages.push(Message::Said(message));
                    self.error = None;
                    self.offset = 0;
                    None
                }
                Action::Expect { uuid } => {
                    self.pending_request = Some(uuid);
                    self.thinking = None;
                    // The reply is added to as it arrives, so there has to be something to add to.
                    self.messages.push(Message::Heard(String::new()));
                    self.offset = 0;
                    None
                }
                Action::Delta { uuid, text, error } => {
                    // A reply to a request which was replaced is no longer wanted. Matching on the
                    // request rather than on what has focus is what lets the person keep typing
                    // while a reply arrives.
                    if self.pending_request != Some(uuid) {
                        return None;
                    }

                    if let Some(message) = self.messages.last_mut() {
                        message.push(&text);
                    }
                    if error.is_some() {
                        self.error = error;
                    }
                    self.offset = 0;
                    None
                }
                Action::Ended { uuid } => {
                    if self.pending_request == Some(uuid) {
                        self.pending_request = None;
                        // Whatever it was up to, it is not up to it any more.
                        self.thinking = None;
                    }
                    None
                }
                Action::Thinking { uuid, word } => {
                    if self.pending_request == Some(uuid) {
                        self.thinking = word;
                    }
                    None
                }
                Action::Up { max } => {
                    if self.offset >= max {
                        return Some(Effect::Bell);
                    }

                    self.offset += 1;
                    None
                }
                Action::Resize { size } => {
                    self.size = size;
                    None
                }
                Action::Down => match self.offset {
                    0 => Some(Effect::Bell),
                    _ => {
                        self.offset -= 1;
                        None
                    }
                },
            }
        }
    }
}
use state::State;

#[cfg(test)]
mod tests {
    use super::{Event, Transcript};

    use rend::Size;
    use til::Component;

    /// Return a transcript of the given size holding one reply of the given number of lines.
    fn transcript(lines: usize, size: Size) -> Transcript {
        let mut transcript = Transcript::new(());
        transcript.handle(Event::Resize { size });
        transcript.handle(Event::Show {
            error: None,
            messages: vec![(false, "line\n\n".repeat(lines))],
        });
        transcript
    }

    /// Return how many times the transcript scrolled back before it would not go further.
    fn scrolls(transcript: &mut Transcript, tries: usize) -> usize {
        (0..tries)
            .take_while(|_| transcript.handle(Event::ScrollUp).is_none())
            .count()
    }

    /// Scrolling back stops with the oldest line at the top, not past it.
    ///
    /// It used to be bounded by a guess at how many lines the messages would come to, which was
    /// larger than the truth and let the view run off above everything into blank rows.
    #[test]
    fn test_scrolling_back_stops_at_the_oldest_line() {
        let size: Size = Size::new(5, 40);
        let mut transcript = transcript(20, size);

        let scrolled: usize = scrolls(&mut transcript, 500);
        assert!(scrolled > 0, "it would not scroll at all");
        assert!(scrolled < 500, "it never stopped");

        // Stopping in the right place means the oldest line is the one at the top. A blank row
        // there is the view having gone up past everything there is.
        let fabric = transcript.render(size);
        let top: String = fabric.cells()[0].iter().map(ToString::to_string).collect();

        assert_eq!(
            top.trim(),
            "line",
            "the view went past the oldest line and left a blank row at the top"
        );
    }

    /// What was said never goes missing, however stale what the transcript was told is.
    ///
    /// How far back it can be scrolled is worked out from the size it was last told, and it is
    /// drawn at whatever size it is given. When the told size has fewer rows than the drawn one,
    /// which is what happens when the box below it gives rows back after being typed in and then
    /// emptied, the view could be left starting past the oldest line with nothing on the screen.
    #[test]
    fn test_what_was_said_does_not_go_missing_when_the_transcript_grows() {
        let told: Size = Size::new(0, 40);
        let drawn: Size = Size::new(10, 40);
        let mut transcript = transcript(20, told);

        scrolls(&mut transcript, 500);
        let fabric = transcript.render(drawn);

        let rows: Vec<String> = fabric
            .cells()
            .iter()
            .map(|row| row.iter().map(ToString::to_string).collect::<String>())
            .collect();

        assert!(
            rows.iter().any(|row| !row.trim().is_empty()),
            "every row was blank: {:?}",
            rows
        );
    }

    #[test]
    fn test_a_transcript_which_fits_does_not_scroll() {
        let size: Size = Size::new(40, 40);
        let mut transcript = transcript(2, size);

        assert_eq!(scrolls(&mut transcript, 10), 0);
    }

    #[test]
    fn test_an_empty_transcript_does_not_scroll() {
        let mut transcript = Transcript::new(());
        transcript.handle(Event::Resize {
            size: Size::new(10, 40),
        });

        assert_eq!(scrolls(&mut transcript, 10), 0);
    }
}

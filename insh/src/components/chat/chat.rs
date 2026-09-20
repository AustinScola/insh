//! Contains the [`Chat`] component.

/// The words for a model which is thinking before it answers.
///
/// Which one a reply gets is picked from the request it is to, so it is the same word for the whole
/// of that reply and a different one for the next.
const THINKING_WORDS: [&str; 12] = [
    "Cogitating",
    "Pondering",
    "Ruminating",
    "Musing",
    "Deliberating",
    "Percolating",
    "Mulling",
    "Noodling",
    "Puzzling",
    "Chewing",
    "Brooding",
    "Reckoning",
];

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use rend::Size;
    use typed_builder::TypedBuilder;

    /// The properties of the chat.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// The directory the chat is about.
        pub dir: PathBuf,
        /// The size of the chat.
        pub size: Size,
        /// What to say to start with.
        #[builder(default)]
        pub prompt: Option<String>,
    }
}
pub use props::Props;

/// Contains the [`Chat`] component.
mod chat {
    use super::super::{
        Input, InputEffect, InputEvent, InputProps, Transcript, TranscriptEffect, TranscriptEvent,
    };
    use super::{Action, Effect, Props, State};
    use crate::components::common::{Footer, FooterProps};
    use crate::stateful::Stateful;

    use insh_api::{
        AiStatusRequestParams, ChatRequestParams, ChatRole, GetChatRequestParams, Request,
        RequestParams, Response, ResponseParams,
    };
    use rend::{Fabric, Size};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::{Component, Event};

    /// The most of the room which what is being typed is allowed to take.
    ///
    /// The box grows as a long message wraps, but what has been said is the reason to be here, so
    /// it never gives up more than this much of the screen to it.
    const MAX_INPUT_SHARE: usize = 2;

    /// What is shown when there is nothing to talk to.
    const NOT_CONFIGURED: &str = "Inshie has no inference engine configured.";

    /// A chat with an AI inference engine.
    pub struct Chat {
        /// The state of the chat.
        state: State,
        /// What has been said.
        transcript: Transcript,
        /// The box which what to say is typed into.
        input: Input,
    }

    impl Component<Props, Event<Response>, Effect> for Chat {
        fn new(props: Props) -> Self {
            let size: Size = props.size;
            // A chat which was started with a prompt has it typed in already, so that the
            // carriage return which insh starts with says it.
            let input = Input::new(InputProps::builder().value(props.prompt.clone()).build());

            let mut transcript = Transcript::new(());
            // How far back the reading can go depends on how much of it fits, which only the size
            // says, so the transcript is told it up front and again whenever it changes.
            transcript.handle(TranscriptEvent::Resize {
                size: Self::transcript_size(size, input.rows(size.columns)),
            });

            Self {
                state: State::from(props),
                transcript,
                input,
            }
        }

        fn name(&self) -> String {
            String::from("inshie")
        }

        fn handle(&mut self, event: Event<Response>) -> Option<Effect> {
            match event {
                Event::Response(response) => self.handle_response(response),
                Event::TermEvent(TermEvent::Resize(size)) => {
                    let input_rows: usize = self.input.rows(size.columns);
                    self.transcript.handle(TranscriptEvent::Resize {
                        size: Self::transcript_size(size, input_rows),
                    });
                    self.state.perform(Action::Resize { size })
                }
                Event::TermEvent(term_event) => self.handle_term_event(term_event),
            }
        }

        fn render(&self, size: Size) -> Fabric {
            if size.rows == 0 || size.columns == 0 {
                return Fabric::new(size);
            }

            // Until the daemon says, it is not known whether there is anything to talk to, and
            // showing nothing is better than showing the wrong thing.
            if self.state.configured() == Some(false) {
                return Fabric::center(NOT_CONFIGURED, size);
            }

            if size.rows == 1 {
                return self.footer(size);
            }

            let transcript_size: Size = Self::transcript_size(size, self.input.rows(size.columns));
            let input_rows: usize = size.rows - 1 - transcript_size.rows;

            let mut fabric: Fabric = self.transcript.render(transcript_size);
            fabric = fabric.quilt_bottom(self.input.render(Size::new(input_rows, size.columns)));
            fabric.quilt_bottom(self.footer(Size::new(1, size.columns)))
        }
    }

    impl Chat {
        /// Return how much room what has been said gets, out of the whole of the chat.
        ///
        /// This is everything the footer and the box which what to say is typed into do not take.
        fn transcript_size(size: Size, input_rows: usize) -> Size {
            if size.rows <= 1 {
                return Size::new(0, size.columns);
            }

            let body_rows: usize = size.rows - 1;
            let most: usize = (body_rows / MAX_INPUT_SHARE).max(1);
            let input_rows: usize = input_rows.clamp(1, most.min(body_rows));

            Size::new(body_rows - input_rows, size.columns)
        }

        /// Return the request to make when a chat is opened.
        ///
        /// A component only gets to return one effect at a time and nothing calls `on_created`, so
        /// whoever opens the chat makes this request.
        pub fn initial_request() -> Request {
            Request::builder()
                .params(RequestParams::AiStatus(
                    AiStatusRequestParams::builder().build(),
                ))
                .build()
        }

        /// Show a chat from the history, and return the request for what was said in it.
        pub fn open(&mut self, chat_id: i64) -> Request {
            self.state.perform(Action::Opened { chat_id });

            Request::builder()
                .params(RequestParams::GetChat(
                    GetChatRequestParams::builder().chat_id(chat_id).build(),
                ))
                .build()
        }

        /// Start a new chat.
        pub fn start_new(&mut self) {
            self.transcript.handle(TranscriptEvent::Show {
                messages: vec![],
                error: None,
            });
            // A new chat starts with nothing said and nothing typed, so a half written message
            // does not follow along into it.
            self.input.handle(InputEvent::Clear);
            self.state.perform(Action::New);
        }

        /// Return the bar which is shown along the bottom.
        fn footer(&self, size: Size) -> Fabric {
            let props = FooterProps::builder()
                .name(self.name())
                .info(&self.state)
                .build();
            Footer::new(props).render(size)
        }

        /// Handle something which was typed.
        fn handle_term_event(&mut self, term_event: TermEvent) -> Option<Effect> {
            // Going to the past chats and moving between what has been said and what is being
            // typed work from anywhere, so they are looked for before whatever has focus gets a
            // chance at them.
            if let TermEvent::KeyEvent(KeyEvent {
                key: Key::Char(character),
                mods: KeyMods::CONTROL,
            }) = &term_event
            {
                match character {
                    'h' => return Some(Effect::OpenHistory),
                    'n' => {
                        self.start_new();
                        return None;
                    }
                    // Reading back through what was said does not take the cursor out of the box
                    // which what to say is typed into, so anything else typed goes straight there
                    // rather than being swallowed by the reading.
                    'y' => return self.scroll(TranscriptEvent::ScrollUp),
                    'e' => return self.scroll(TranscriptEvent::ScrollDown),
                    _ => {}
                }
            }

            let effect = self.input.handle(InputEvent::TermEvent(term_event));
            self.handle_input_effect(effect)
        }

        /// Read back through what was said.
        fn scroll(&mut self, event: TranscriptEvent) -> Option<Effect> {
            // How far back the reading can go depends on how much of it is on the screen, and the
            // box which what to say is typed into has been taking rows from it as it was typed in.
            let size: Size = self.state.size();
            self.transcript.handle(TranscriptEvent::Resize {
                size: Self::transcript_size(size, self.input.rows(size.columns)),
            });

            self.transcript
                .handle(event)
                .map(|TranscriptEffect::Bell| Effect::Bell)
        }

        /// Handle what the box which what to say is typed into wants.
        fn handle_input_effect(&mut self, effect: Option<InputEffect>) -> Option<Effect> {
            match effect {
                Some(InputEffect::Send { message }) => {
                    let request = Request::builder()
                        .params(RequestParams::Chat(
                            ChatRequestParams::builder()
                                .chat_id(self.state.chat_id())
                                .dir(self.state.dir().to_path_buf())
                                .message(message.clone())
                                .build(),
                        ))
                        .build();
                    let uuid = *request.uuid();

                    self.transcript.handle(TranscriptEvent::Said { message });
                    self.transcript.handle(TranscriptEvent::Expect { uuid });
                    self.state.perform(Action::Sent { uuid });

                    Some(Effect::Request(request))
                }
                Some(InputEffect::Unfocus) => Some(Effect::Quit),
                Some(InputEffect::Bell) => Some(Effect::Bell),
                None => None,
            }
        }

        /// Handle a response from the daemon.
        fn handle_response(&mut self, response: Response) -> Option<Effect> {
            let uuid = *response.uuid();
            let last: bool = response.last();

            match response.params() {
                ResponseParams::AiStatus(params) => self.state.perform(Action::Configured {
                    configured: params.configured(),
                }),
                ResponseParams::GetChat(params) => {
                    let messages: Vec<(bool, String)> = params
                        .messages()
                        .iter()
                        .map(|message| {
                            (
                                matches!(message.role(), ChatRole::User),
                                message.content().to_string(),
                            )
                        })
                        .collect();
                    self.transcript.handle(TranscriptEvent::Show {
                        messages,
                        error: params.error().cloned(),
                    });
                    None
                }
                ResponseParams::Chat(params) => {
                    self.transcript.handle(TranscriptEvent::Delta {
                        uuid,
                        text: params.delta().to_string(),
                        error: params.error().cloned(),
                    });
                    self.state.perform(Action::Replying {
                        uuid,
                        tokens: params.tokens(),
                        thinking_tokens: params.thinking_tokens(),
                        thinking: params.thinking(),
                        duration: params.duration(),
                    });
                    self.transcript.handle(TranscriptEvent::Thinking {
                        uuid,
                        word: match params.thinking() {
                            true => Some(self.state.thinking_word().to_string()),
                            false => None,
                        },
                    });

                    if last {
                        self.transcript.handle(TranscriptEvent::Ended { uuid });
                        // The chat which this started is the one the next message goes in.
                        return self.state.perform(Action::Replied {
                            uuid,
                            chat_id: params.chat_id(),
                        });
                    }

                    None
                }
                _ => {
                    #[cfg(feature = "logging")]
                    log::error!("Unexpected response parameters.");
                    None
                }
            }
        }
    }
}
pub use chat::Chat;

/// Contains the [`Effect`] enum.
mod effect {
    use insh_api::Request;

    /// A chat effect.
    pub enum Effect {
        /// Make a request.
        Request(Request),
        /// Show the past chats.
        OpenHistory,
        /// Leave the chat.
        Quit,
        /// Ring the bell.
        Bell,
    }
}
pub use effect::Effect;

/// Contains the [`Action`] enum.
mod action {
    use std::time::Duration;

    use rend::Size;
    use uuid::Uuid;

    /// A chat action.
    pub enum Action {
        /// The chat was resized.
        Resize {
            /// The new size.
            size: Size,
        },
        /// Whether there is an inference engine to talk to was learned.
        Configured {
            /// Whether there is an inference engine to talk to.
            configured: bool,
        },
        /// Something was said.
        Sent {
            /// The unique identifier of the request it was said with.
            uuid: Uuid,
        },
        /// A piece of a reply arrived.
        Replying {
            /// The unique identifier of the request the reply is to.
            uuid: Uuid,
            /// How many tokens the reply came to, once the inference engine has said.
            tokens: Option<usize>,
            /// How many of them were the engine thinking rather than answering.
            thinking_tokens: Option<usize>,
            /// Whether the engine is thinking rather than answering.
            thinking: bool,
            /// How long the reply has been coming.
            duration: Duration,
        },
        /// A reply finished.
        Replied {
            /// The unique identifier of the request the reply was to.
            uuid: Uuid,
            /// Which chat the reply was in, if it was in one at all.
            chat_id: Option<i64>,
        },
        /// A past chat was opened.
        Opened {
            /// Which chat was opened.
            chat_id: i64,
        },
        /// A new chat was started.
        New,
    }
}
use action::Action;

/// Contains the [`State`] struct.
mod state {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use super::THINKING_WORDS;

    use super::{Action, Effect, Props};
    use crate::components::common::FooterInfo;
    use crate::stateful::Stateful;

    use rend::Size;
    use uuid::Uuid;

    /// The state of the chat.
    pub struct State {
        /// The directory the chat is about.
        dir: PathBuf,
        /// The size of the chat.
        size: Size,
        /// Whether there is an inference engine to talk to, once it is known.
        configured: Option<bool>,
        /// Which chat is being had, once there is one.
        chat_id: Option<i64>,
        /// The request whose reply is being waited for.
        pending_request: Option<Uuid>,
        /// Whether anything has been replied yet, so that nothing is said before the first one.
        replied: bool,
        /// How many tokens the reply came to, once the inference engine has said.
        tokens: Option<usize>,
        /// How many of them were the engine thinking rather than answering.
        thinking_tokens: Option<usize>,
        /// Whether the engine is thinking rather than answering.
        thinking: bool,
        /// Which word is used for the thinking of the reply being waited for.
        thinking_word: &'static str,
        /// How long the reply took, or has been taking.
        duration: Duration,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                dir: props.dir,
                size: props.size,
                configured: None,
                chat_id: None,
                pending_request: None,
                replied: false,
                tokens: None,
                thinking_tokens: None,
                thinking: false,
                thinking_word: THINKING_WORDS[0],
                duration: Duration::ZERO,
            }
        }
    }

    impl State {
        /// Return the directory the chat is about.
        pub fn dir(&self) -> &Path {
            &self.dir
        }

        /// Return the size of the chat.
        pub fn size(&self) -> Size {
            self.size
        }

        /// Return what the thinking of the reply being waited for is called.
        pub fn thinking_word(&self) -> &'static str {
            self.thinking_word
        }

        /// Return whether there is an inference engine to talk to, once it is known.
        pub fn configured(&self) -> Option<bool> {
            self.configured
        }

        /// Return which chat is being had, once there is one.
        pub fn chat_id(&self) -> Option<i64> {
            self.chat_id
        }

        /// Forget how the last reply came along, since it was to another chat.
        ///
        /// What the footer says is about the reply it is next to, so carrying it into a chat which
        /// has not been replied in yet would say it of the wrong one.
        fn forget_reply(&mut self) {
            self.replied = false;
            self.tokens = None;
            self.thinking_tokens = None;
            self.thinking = false;
            self.duration = Duration::ZERO;
        }
    }

    impl FooterInfo for State {
        fn in_progress(&self) -> bool {
            self.pending_request.is_some()
        }

        /// Return how the reply is coming along, the same way the searcher says how the searching
        /// is.
        fn text(&self) -> String {
            if !self.replied {
                return String::new();
            }

            let seconds: String = format!("({:.2}s)", self.duration.as_secs_f64());

            // How many tokens a reply came to is only known once the engine says, which it does
            // once, near the end. It is not counted here in the meantime: there is no tokenizer
            // for these models to count with, so anything shown before then would be a guess.
            let tokens: String = match self.tokens {
                None => String::new(),
                Some(tokens) => {
                    let thinking: String = match self.thinking_tokens {
                        // The thinking is counted in the total, which is why a short answer can
                        // come to far more tokens than the words in it.
                        Some(thought) if thought > 0 => format!(" | {} thinking", thought),
                        _ => String::new(),
                    };

                    format!(
                        "{} token{}{} ",
                        tokens,
                        match tokens {
                            1 => "",
                            _ => "s",
                        },
                        thinking
                    )
                }
            };

            format!("{}{}", tokens, seconds)
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Resize { size } => {
                    self.size = size;
                    None
                }
                Action::Configured { configured } => {
                    self.configured = Some(configured);
                    None
                }
                Action::Sent { uuid } => {
                    self.pending_request = Some(uuid);
                    self.replied = true;
                    self.tokens = None;
                    self.thinking_tokens = None;
                    self.thinking = false;
                    self.duration = Duration::ZERO;
                    // Which word this reply thinks in comes from the request, so that it stays put
                    // for the whole of it rather than changing with every piece which arrives.
                    self.thinking_word =
                        THINKING_WORDS[(uuid.as_u128() % THINKING_WORDS.len() as u128) as usize];
                    None
                }
                Action::Replying {
                    uuid,
                    tokens,
                    thinking_tokens,
                    thinking,
                    duration,
                } => {
                    // A reply to a request which was replaced says nothing about this one.
                    if self.pending_request != Some(uuid) {
                        return None;
                    }

                    if tokens.is_some() {
                        self.tokens = tokens;
                        self.thinking_tokens = thinking_tokens;
                    }
                    self.thinking = thinking;
                    self.duration = duration;
                    None
                }
                Action::Replied { uuid, chat_id } => {
                    if self.pending_request == Some(uuid) {
                        self.pending_request = None;
                        // A reply which never got as far as a chat leaves this alone, so that the
                        // next thing said starts one rather than going to a chat which is not
                        // there.
                        if let Some(chat_id) = chat_id {
                            self.chat_id = Some(chat_id);
                        }
                    }
                    None
                }
                Action::Opened { chat_id } => {
                    self.chat_id = Some(chat_id);
                    self.pending_request = None;
                    self.forget_reply();
                    None
                }
                Action::New => {
                    self.chat_id = None;
                    self.pending_request = None;
                    self.forget_reply();
                    None
                }
            }
        }
    }
}
use state::State;

#[cfg(test)]
mod tests {
    use super::{Chat, Effect, Props};

    use insh_api::{ChatResponseParams, Request, RequestParams, Response, ResponseParams};
    use rend::{Fabric, Size};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::{Component, Event};

    use std::path::PathBuf;

    use test_case::test_case;

    /// Return a chat which has just been opened.
    fn chat(size: Size) -> Chat {
        Chat::new(
            Props::builder()
                .dir(PathBuf::from("/tmp"))
                .size(size)
                .build(),
        )
    }

    /// Return a chat which has just been opened with something to say.
    fn chat_with_prompt(size: Size, prompt: &str) -> Chat {
        Chat::new(
            Props::builder()
                .dir(PathBuf::from("/tmp"))
                .size(size)
                .prompt(Some(prompt.to_string()))
                .build(),
        )
    }

    /// Return the event for pressing a key.
    fn press(key: Key, mods: KeyMods) -> Event<Response> {
        Event::TermEvent(TermEvent::KeyEvent(KeyEvent { key, mods }))
    }

    /// Whatever it is asked for, the chat has to give back a fabric of exactly that size.
    ///
    /// Anything smaller leaves the rows it did not reach showing whatever was on the screen
    /// before, because the renderer only paints what it is given.
    #[test_case(24, 80; "a normal terminal")]
    #[test_case(24, 100; "a wide terminal")]
    #[test_case(24, 40; "a narrow terminal")]
    #[test_case(5, 80; "room for the transcript and what is being typed")]
    #[test_case(4, 80; "only room for what is being typed")]
    #[test_case(3, 80; "not quite room for what is being typed")]
    #[test_case(2, 80; "two rows")]
    #[test_case(1, 80; "one row")]
    #[test_case(0, 80; "no rows")]
    #[test_case(24, 0; "no columns")]
    fn test_render_fills_the_size_it_is_given(rows: usize, columns: usize) {
        let size: Size = Size::new(rows, columns);

        let fabric: Fabric = chat(size).render(size);

        assert_eq!(fabric.size(), size);
    }

    /// Starting a new conversation leaves the chat looking like one which was just opened.
    #[test]
    fn test_a_new_conversation_puts_back_an_empty_chat() {
        let size: Size = Size::new(24, 80);
        let mut chat: Chat = chat(size);
        let empty: Fabric = chat.render(size);

        chat.handle(press(Key::Char('h'), KeyMods::NONE));
        chat.handle(press(Key::Char('i'), KeyMods::NONE));
        chat.handle(press(Key::CarriageReturn, KeyMods::NONE));
        assert_ne!(
            chat.render(size),
            empty,
            "What was said was never shown in the first place."
        );

        chat.handle(press(Key::Char('n'), KeyMods::CONTROL));

        assert_eq!(chat.render(size), empty);
    }

    /// A reply which fails before a chat is started does not leave one behind to talk into.
    ///
    /// The daemon says which chat a reply is in, and it used to say `0` for one which never got
    /// as far as being started. Taking that for a chat sent every later message to a row which is
    /// not there, which the database refuses, wedging the chat until a new one was started.
    #[test]
    fn test_a_reply_which_never_started_a_chat_leaves_no_chat_behind() {
        let size: Size = Size::new(24, 80);
        let mut chat: Chat = chat(size);

        chat.handle(press(Key::Char('h'), KeyMods::NONE));
        let request: Request = match chat.handle(press(Key::CarriageReturn, KeyMods::NONE)) {
            Some(Effect::Request(request)) => request,
            _ => panic!("Saying something has to ask the daemon for a reply."),
        };

        chat.handle(Event::Response(
            Response::builder()
                .uuid(*request.uuid())
                .last(true)
                .params(ResponseParams::Chat(
                    ChatResponseParams::builder()
                        .error(Some("No AI inference engine is configured.".to_string()))
                        .build(),
                ))
                .build(),
        ));

        // The next thing said starts a chat rather than going into one which is not there.
        chat.handle(press(Key::Char('h'), KeyMods::NONE));
        let next: Request = match chat.handle(press(Key::CarriageReturn, KeyMods::NONE)) {
            Some(Effect::Request(request)) => request,
            _ => panic!("Saying something again has to ask the daemon for a reply."),
        };
        match next.params() {
            RequestParams::Chat(params) => assert_eq!(params.chat_id(), None),
            _ => panic!("The request has to be one for a reply."),
        }
    }

    /// A chat which was started with a prompt says it when insh presses enter for it.
    #[test]
    fn test_a_prompt_is_said_as_it_was_passed() {
        let size: Size = Size::new(24, 80);
        let mut chat: Chat = chat_with_prompt(size, "hi");

        let request: Request = match chat.handle(press(Key::CarriageReturn, KeyMods::NONE)) {
            Some(Effect::Request(request)) => request,
            _ => panic!("The prompt has to be said as soon as enter is pressed."),
        };

        match request.params() {
            RequestParams::Chat(params) => assert_eq!(params.message(), "hi"),
            _ => panic!("The request has to be one for a reply."),
        }
    }

    /// A message which was being written is not carried into a new conversation.
    #[test]
    fn test_a_new_conversation_throws_away_what_was_being_typed() {
        let size: Size = Size::new(24, 80);
        let mut chat: Chat = chat(size);
        let empty: Fabric = chat.render(size);

        chat.handle(press(Key::Char('h'), KeyMods::NONE));
        chat.handle(press(Key::Char('i'), KeyMods::NONE));
        assert_ne!(
            chat.render(size),
            empty,
            "What was being typed was never shown in the first place."
        );

        chat.handle(press(Key::Char('n'), KeyMods::CONTROL));

        assert_eq!(chat.render(size), empty);
    }
}

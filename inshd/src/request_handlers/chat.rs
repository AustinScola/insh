//! Handles requests to say something in a chat.
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::ai_streamer::{AiStreamer, Streamed};
use crate::config::Config;

use ai_client::{AiClient, ChatOptions, Message, Role};
use embedder::{Embedder, MODEL_ID};
use insh_api::{ChatRequestParams, ChatResponseParams, ResponseParams, ResponseParamsAndLast};
use insh_db::{chats, DbConnPool};

use crossbeam::channel::{self, Receiver, Sender};

/// The role recorded for what the person using insh says.
const USER: &str = "user";

/// The role recorded for what the inference engine says.
const ASSISTANT: &str = "assistant";

/// The most characters of the first message to name a chat after.
const TITLE_LENGTH: usize = 60;

/// The most tokens a name is allowed to be.
///
/// A name is a handful of words, and a cap this low keeps a model which would rather explain
/// itself from writing a paragraph.
const NAME_MAX_TOKENS: usize = 24;

/// The most characters of a name to keep.
const NAME_LENGTH: usize = 60;

/// What the inference engine is told when it is asked to name a chat.
const NAME_INSTRUCTIONS: &str = "You name conversations. Reply with a short title of at most six \
     words for the conversation so far. Reply with the title and nothing else: no quotes, no \
     punctuation at the end, and no explanation.";

/// What the inference engine is asked in order to name a chat.
const NAME_REQUEST: &str = "Give this conversation a title.";

/// Handles a request to say something in a chat.
pub struct Chat {
    /// Which chat the reply is in, once there is one.
    ///
    /// A chat which could not be started has none, and saying so is what keeps a failure from
    /// being taken for a chat which the next thing said could go in.
    chat_id: Option<i64>,
    /// A receiver for the pieces of the reply.
    results_rx: Option<Receiver<Streamed>>,
    /// A handle to the thread which streams the reply.
    ai_streamer_handle: Option<JoinHandle<()>>,
    /// What went wrong before the reply could be asked for, if anything did.
    failure: Option<String>,
    /// The reply so far, kept so that it can be recorded once it is whole.
    reply: String,
    /// If the reply is over.
    done: bool,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// Turns text into vectors.
    embedder: Arc<Embedder>,
    /// A client to ask for a name with, once there is a whole exchange to name.
    ///
    /// This is only set for a chat which was started by this request, since one which was already
    /// going has a name.
    namer: Option<AiClient>,
}

impl Chat {
    /// Say something in a chat and start streaming the reply.
    pub fn run(
        params: &ChatRequestParams,
        config: Config,
        db_conn_pool: &DbConnPool,
        embedder: Arc<Embedder>,
    ) -> Self {
        let mut chat = Self {
            chat_id: params.chat_id(),
            results_rx: None,
            ai_streamer_handle: None,
            failure: None,
            reply: String::new(),
            done: false,
            db_conn_pool: db_conn_pool.clone(),
            embedder,
            namer: None,
        };

        if let Err(failure) = chat.start(params, config) {
            chat.failure = Some(failure);
        }

        return chat;
    }

    /// Record what was said, then ask for a reply.
    fn start(&mut self, params: &ChatRequestParams, config: Config) -> Result<(), String> {
        if !config.ai().configured() {
            return Err("No AI inference engine is configured.".to_string());
        }

        // A chat is started by the first thing said in it. It is named after that to begin with so
        // that it always has a name, and the engine is asked for a better one once it has answered.
        let new: bool = params.chat_id().is_none();
        if new {
            let title: String = Self::title_of(params.message());
            let dir: String = params.dir().to_string_lossy().to_string();
            self.chat_id = Some(
                chats::create(&self.db_conn_pool, &title, &dir)
                    .map_err(|error| format!("Failed to start the chat: {}", error))?,
            );
        }
        let chat_id: i64 = self
            .chat_id
            .ok_or_else(|| "There is no chat to say it in.".to_string())?;

        // What was said is recorded before the reply is asked for, so that it is not lost if the
        // inference engine never answers.
        let said: i64 = chats::add_message(&self.db_conn_pool, chat_id, USER, params.message())
            .map_err(|error| format!("Failed to record what you said: {}", error))?;
        self.embed(said, params.message());

        let messages: Vec<Message> = self.messages()?;
        let instructions: Option<String> = config.ai().instructions();

        let client: AiClient = AiClient::builder()
            .base_url(config.ai().base_url().cloned().unwrap_or_default())
            .api_key(config.ai().api_key().cloned().unwrap_or_default())
            .model(config.ai().model())
            .api(config.ai().api_type())
            .build();

        let namer: AiClient = client.with_max_tokens(NAME_MAX_TOKENS);

        let (results_tx, results_rx): (Sender<Streamed>, Receiver<Streamed>) = channel::unbounded();
        let mut ai_streamer: AiStreamer = AiStreamer::builder().results_tx(results_tx).build();
        let handle: JoinHandle<()> = thread::Builder::new()
            .name("ai-streamer".to_string())
            .spawn(move || ai_streamer.run(client, messages, instructions))
            .unwrap();

        self.results_rx = Some(results_rx);
        self.ai_streamer_handle = Some(handle);
        if new {
            self.namer = Some(namer);
        }

        return Ok(());
    }

    /// Return the conversation so far, for the inference engine to reply to.
    fn messages(&self) -> Result<Vec<Message>, String> {
        let chat_id: i64 = self
            .chat_id
            .ok_or_else(|| "There is no chat to read.".to_string())?;

        let messages = chats::messages(&self.db_conn_pool, chat_id)
            .map_err(|error| format!("Failed to read the chat: {}", error))?;

        return Ok(messages
            .into_iter()
            .map(|message| {
                let role: Role = match message.role.as_str() {
                    ASSISTANT => Role::Assistant,
                    _ => Role::User,
                };
                Message::builder()
                    .role(role)
                    .content(message.content)
                    .build()
            })
            .collect());
    }

    /// Record the vector of a message.
    ///
    /// Failing to do this only costs the message being findable by meaning, so it is logged rather
    /// than passed on.
    fn embed(&self, message_id: i64, content: &str) {
        let embedding: Vec<f32> = self.embedder.embed_one(content);

        if let Err(error) =
            chats::set_embedding(&self.db_conn_pool, message_id, &embedding, MODEL_ID)
        {
            log::error!("Failed to record the vector of a message: {}", error);
        }
    }

    /// Record the reply now that it is whole.
    fn record_reply(&mut self) {
        let chat_id: i64 = match self.chat_id {
            Some(chat_id) => chat_id,
            None => return,
        };
        if self.reply.is_empty() {
            return;
        }

        let reply: String = std::mem::take(&mut self.reply);
        match chats::add_message(&self.db_conn_pool, chat_id, ASSISTANT, &reply) {
            Ok(id) => self.embed(id, &reply),
            Err(error) => log::error!("Failed to record the reply: {}", error),
        }
    }

    /// Ask the inference engine what the chat should be called, and record what it says.
    ///
    /// This happens after the reply has been sent on, so it costs the person nothing but the
    /// tokens. Failing only leaves the chat named after the first thing said in it, which is what
    /// it was called already, so it is logged rather than passed on.
    fn name_chat(&mut self) {
        let namer: AiClient = match self.namer.take() {
            Some(namer) => namer,
            None => return,
        };
        let chat_id: i64 = match self.chat_id {
            Some(chat_id) => chat_id,
            None => return,
        };

        let mut messages: Vec<Message> = match self.messages() {
            Ok(messages) => messages,
            Err(error) => {
                log::error!("Failed to read the chat to name it: {}", error);
                return;
            }
        };
        messages.push(
            Message::builder()
                .role(Role::User)
                .content(NAME_REQUEST)
                .build(),
        );

        let options: ChatOptions = ChatOptions::builder()
            .messages(messages)
            .instructions(Some(NAME_INSTRUCTIONS.to_string()))
            .build();

        let name: String = match namer.complete(options) {
            Ok(name) => name,
            Err(error) => {
                log::error!("Failed to ask for a name for the chat: {}", error);
                return;
            }
        };

        // A model which was asked for a title and wrote a sentence anyway is not allowed to make
        // the list of chats unreadable.
        let name: String = name.trim().trim_matches('"').trim().to_string();
        let name: String = match name.lines().next() {
            Some(line) if !line.is_empty() => line.chars().take(NAME_LENGTH).collect(),
            _ => return,
        };

        if let Err(error) = chats::rename(&self.db_conn_pool, chat_id, &name) {
            log::error!("Failed to name the chat: {}", error);
        }
    }

    /// Return what to call a chat which starts with a message.
    fn title_of(message: &str) -> String {
        let line: &str = message.lines().next().unwrap_or_default().trim();

        if line.is_empty() {
            return "New chat".to_string();
        }

        // Counting characters rather than bytes keeps this from splitting one in half.
        if line.chars().count() <= TITLE_LENGTH {
            return line.to_string();
        }

        return line.chars().take(TITLE_LENGTH).collect();
    }

    /// Return the parameters of a response carrying a piece of the reply.
    fn response_params(
        &self,
        delta: String,
        tokens: Option<(usize, usize)>,
        duration: Duration,
        thinking: bool,
        error: Option<String>,
    ) -> ResponseParams {
        return ResponseParams::Chat(
            ChatResponseParams::builder()
                .chat_id(self.chat_id)
                .delta(delta)
                .tokens(tokens.map(|(total, _)| total))
                .thinking_tokens(tokens.map(|(_, thought)| thought))
                .thinking(thinking)
                .duration(duration)
                .error(error)
                .build(),
        );
    }
}

impl Iterator for Chat {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            // Asking what the chat should be called is a request of its own and waiting on it
            // holds up whatever is waiting on this. It is left until here, which is after the
            // last piece of the reply has gone out, so that it costs the person nothing but the
            // tokens.
            self.name_chat();
            return None;
        }

        // Nothing was ever asked for, so the only thing to say is why.
        if let Some(failure) = self.failure.take() {
            self.done = true;
            return Some(
                ResponseParamsAndLast::builder()
                    .response_params(self.response_params(
                        String::new(),
                        None,
                        Duration::ZERO,
                        false,
                        Some(failure),
                    ))
                    .last(true)
                    .build(),
            );
        }

        let results_rx: &Receiver<Streamed> = self.results_rx.as_ref()?;

        let streamed: Streamed = match results_rx.recv() {
            Ok(streamed) => streamed,
            Err(error) => {
                log::error!("Failed to receive a piece of the reply: {}", error);
                self.done = true;
                self.record_reply();
                return Some(
                    ResponseParamsAndLast::builder()
                        .response_params(self.response_params(
                            String::new(),
                            None,
                            Duration::ZERO,
                            false,
                            Some(error.to_string()),
                        ))
                        .last(true)
                        .build(),
                );
            }
        };

        self.reply.push_str(&streamed.delta);

        if streamed.done {
            self.done = true;
            if let Some(handle) = self.ai_streamer_handle.take() {
                let _ = handle.join();
            }
            self.record_reply();
        }

        return Some(
            ResponseParamsAndLast::builder()
                .response_params(self.response_params(
                    streamed.delta,
                    streamed.tokens,
                    streamed.duration,
                    streamed.thinking,
                    streamed.error,
                ))
                .last(streamed.done)
                .build(),
        );
    }
}

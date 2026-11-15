//! Streams a reply from an AI inference engine.

use std::cell::Cell;
use std::mem;
use std::time::{Duration, Instant};

use crate::tools::Toolbox;

use ai_client::{
    AiClient, ChatOptions, ChatStream, Chunk, Message, Role, ToolChoice, ToolDefinition, ToolResult,
};

use crossbeam::channel::Sender;
use typed_builder::TypedBuilder;

/// How often the text which has arrived is passed on.
///
/// A reply arrives a few characters at a time, which is far more often than the screen can be
/// drawn, so the pieces are gathered up and sent on at a rate which is worth waking the client for.
const UPDATE_INTERVAL: Duration = Duration::from_millis(50);

/// The most times the engine is allowed to use tools in one reply.
///
/// An engine which keeps asking for tools would otherwise never answer. Once it has used this
/// many it is asked again without being allowed any more, so that it answers with what it has.
const MAX_TOOL_ROUNDS: usize = 8;

/// What goes between the text the engine wrote before using a tool and the text it wrote after.
const ROUND_SEPARATOR: &str = "\n\n";

/// A piece of a reply.
#[derive(TypedBuilder)]
pub struct Streamed {
    /// The text which arrived since the last piece.
    #[builder(default)]
    pub delta: String,
    /// How many tokens the reply came to, once the engine has said, and how many of them were it
    /// thinking rather than answering.
    #[builder(default)]
    pub tokens: Option<(usize, usize)>,
    /// How long the reply has been coming.
    pub duration: Duration,
    /// Whether the engine is thinking rather than answering.
    #[builder(default)]
    pub thinking: bool,
    /// What the engine is doing other than thinking or answering, like searching the web.
    #[builder(default)]
    pub activity: Option<String>,
    /// Whether the reply is over.
    #[builder(default)]
    pub done: bool,
    /// What went wrong, if anything did.
    #[builder(default)]
    pub error: Option<String>,
}

/// How a reply which can take several requests to the engine is going.
struct Progress {
    /// When the reply was asked for.
    started: Instant,
    /// How many tokens the requests which are over came to, and how many of them were thinking.
    tokens: Option<(usize, usize)>,
    /// Whether any of the answer has been passed on yet.
    said: bool,
}

impl Progress {
    /// Return how many tokens the reply has come to, counting the request which is going.
    fn tokens(&self, going: Option<(usize, usize)>) -> Option<(usize, usize)> {
        match (self.tokens, going) {
            (Some((total, thought)), Some((more, more_thought))) => {
                Some((total + more, thought + more_thought))
            }
            (tokens, None) | (None, tokens) => tokens,
        }
    }
}

/// Streams a reply from an AI inference engine.
#[derive(TypedBuilder)]
pub struct AiStreamer {
    /// A sender for the pieces of the reply.
    results_tx: Sender<Streamed>,
    /// The tools which the engine can use.
    #[builder(default)]
    toolbox: Toolbox,
    /// Whether nothing is waiting for the reply any more.
    ///
    /// A reply which uses tools can take several requests to the engine and the tools, all of
    /// which cost something, so none of them are made once nobody is going to see what comes of
    /// them.
    #[builder(default, setter(skip))]
    abandoned: Cell<bool>,
}

impl AiStreamer {
    /// Stream a reply to a conversation.
    ///
    /// When the engine asks to use tools, they are used and it is asked again with what came of
    /// them, until it answers without asking for any. The text of every one of those requests is
    /// the reply.
    pub fn run(
        &mut self,
        client: AiClient,
        mut messages: Vec<Message>,
        instructions: Option<String>,
    ) {
        let mut progress: Progress = Progress {
            started: Instant::now(),
            tokens: None,
            said: false,
        };
        let tools: Vec<ToolDefinition> = self.toolbox.definitions();

        for round in 0..=MAX_TOOL_ROUNDS {
            if self.abandoned.get() {
                return;
            }

            let tool_choice: ToolChoice = match round < MAX_TOOL_ROUNDS {
                true => ToolChoice::Auto,
                false => ToolChoice::None,
            };
            let options: ChatOptions = ChatOptions::builder()
                .messages(messages.clone())
                .instructions(instructions.clone())
                .tools(tools.clone())
                .tool_choice(tool_choice)
                .build();

            let reply: Message = match self.stream(&client, options, &mut progress) {
                Some(reply) => reply,
                None => return,
            };

            if reply.tool_calls.is_empty() {
                break;
            }

            self.send(
                Streamed::builder()
                    .tokens(progress.tokens)
                    .duration(progress.started.elapsed())
                    .activity(self.toolbox.activity(&reply.tool_calls))
                    .build(),
            );
            if self.abandoned.get() {
                return;
            }
            let results: Vec<ToolResult> = self.toolbox.call_all(&reply.tool_calls);

            messages.push(reply);
            messages.push(
                Message::builder()
                    .role(Role::User)
                    .tool_results(results)
                    .build(),
            );
        }

        self.send(
            Streamed::builder()
                .tokens(progress.tokens)
                .duration(progress.started.elapsed())
                .done(true)
                .build(),
        );
    }

    /// Stream one request's worth of the reply, and return all of it once it is over.
    ///
    /// When it fails, the failure is passed on as the end of the reply and nothing is returned.
    fn stream(
        &self,
        client: &AiClient,
        options: ChatOptions,
        progress: &mut Progress,
    ) -> Option<Message> {
        let mut stream: ChatStream = match client.chat(options) {
            Ok(stream) => stream,
            Err(error) => {
                log::error!("Failed to ask for a reply: {}", error);
                self.send(
                    Streamed::builder()
                        .tokens(progress.tokens)
                        .duration(progress.started.elapsed())
                        .done(true)
                        .error(Some(error.to_string()))
                        .build(),
                );
                return None;
            }
        };

        let mut tokens: Option<(usize, usize)> = None;
        let mut delta: String = String::new();
        let mut sent_at: Instant = Instant::now();
        let mut said: bool = false;

        for chunk in stream.by_ref() {
            // Dropping the stream hangs up on the engine, which stops it writing the rest.
            if self.abandoned.get() {
                return None;
            }

            match chunk {
                Ok(Chunk::Text(text)) => {
                    // Text which comes after a tool was used is kept apart from the text which
                    // came before it.
                    if !said && progress.said {
                        delta.push_str(ROUND_SEPARATOR);
                    }
                    said = true;

                    // The engine has stopped thinking, since this is the answer.
                    delta.push_str(&text);
                    if sent_at.elapsed() >= UPDATE_INTERVAL {
                        self.send(
                            Streamed::builder()
                                .delta(mem::take(&mut delta))
                                .tokens(progress.tokens(tokens))
                                .duration(progress.started.elapsed())
                                .build(),
                        );
                        sent_at = Instant::now();
                    }
                }
                Ok(Chunk::Thinking(_)) => {
                    // Nothing of the thinking is shown, but how long it has been going is, so a
                    // reply which is all thinking still ticks along rather than sitting still.
                    if sent_at.elapsed() >= UPDATE_INTERVAL {
                        self.send(
                            Streamed::builder()
                                .delta(mem::take(&mut delta))
                                .tokens(progress.tokens(tokens))
                                .duration(progress.started.elapsed())
                                .thinking(true)
                                .build(),
                        );
                        sent_at = Instant::now();
                    }
                }
                Ok(Chunk::Tokens {
                    total,
                    thinking: thought,
                }) => tokens = Some((total, thought)),
                Ok(Chunk::Done) => break,
                Err(error) => {
                    // Whatever arrived before the error is still worth keeping, so it is sent
                    // along with it rather than thrown away.
                    log::error!("Failed to stream a reply: {}", error);
                    self.send(
                        Streamed::builder()
                            .delta(delta)
                            .tokens(progress.tokens(tokens))
                            .duration(progress.started.elapsed())
                            .done(true)
                            .error(Some(error.to_string()))
                            .build(),
                    );
                    return None;
                }
            }
        }

        progress.tokens = progress.tokens(tokens);
        progress.said |= said;

        if !delta.is_empty() {
            self.send(
                Streamed::builder()
                    .delta(delta)
                    .tokens(progress.tokens)
                    .duration(progress.started.elapsed())
                    .build(),
            );
        }

        return Some(stream.into_reply());
    }

    /// Pass a piece of the reply on, taking note when nothing is waiting for it any more.
    fn send(&self, streamed: Streamed) {
        if self.abandoned.get() {
            return;
        }

        if self.results_tx.send(streamed).is_err() {
            log::info!("Stopped a reply since nothing is waiting for it any more.");
            self.abandoned.set(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossbeam::channel::{self, Receiver};

    /// Once nothing is waiting for the reply, the streamer takes note so that it stops.
    #[test]
    fn test_a_reply_nobody_is_waiting_for_is_abandoned() {
        let (results_tx, results_rx): (Sender<Streamed>, Receiver<Streamed>) = channel::unbounded();
        let streamer: AiStreamer = AiStreamer::builder().results_tx(results_tx).build();

        streamer.send(Streamed::builder().duration(Duration::ZERO).build());
        assert!(!streamer.abandoned.get());

        drop(results_rx);
        streamer.send(Streamed::builder().duration(Duration::ZERO).build());
        assert!(streamer.abandoned.get());
    }
}

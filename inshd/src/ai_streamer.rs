//! Streams a reply from an AI inference engine.

use std::mem;
use std::time::{Duration, Instant};

use ai_client::{AiClient, ChatOptions, Chunk, Message};

use crossbeam::channel::Sender;
use typed_builder::TypedBuilder;

/// How often the text which has arrived is passed on.
///
/// A reply arrives a few characters at a time, which is far more often than the screen can be
/// drawn, so the pieces are gathered up and sent on at a rate which is worth waking the client for.
const UPDATE_INTERVAL: Duration = Duration::from_millis(50);

/// A piece of a reply.
pub struct Streamed {
    /// The text which arrived since the last piece.
    pub delta: String,
    /// How many tokens the reply came to, once the engine has said, and how many of them were it
    /// thinking rather than answering.
    pub tokens: Option<(usize, usize)>,
    /// How long the reply has been coming.
    pub duration: Duration,
    /// Whether the engine is thinking rather than answering.
    pub thinking: bool,
    /// Whether the reply is over.
    pub done: bool,
    /// What went wrong, if anything did.
    pub error: Option<String>,
}

/// Streams a reply from an AI inference engine.
#[derive(TypedBuilder)]
pub struct AiStreamer {
    /// A sender for the pieces of the reply.
    results_tx: Sender<Streamed>,
}

impl AiStreamer {
    /// Stream a reply to a conversation.
    pub fn run(&mut self, client: AiClient, messages: Vec<Message>, instructions: Option<String>) {
        let started: Instant = Instant::now();
        let mut tokens: Option<(usize, usize)> = None;

        let options: ChatOptions = ChatOptions::builder()
            .messages(messages)
            .instructions(instructions)
            .build();

        let stream = match client.chat(options) {
            Ok(stream) => stream,
            Err(error) => {
                self.send(
                    String::new(),
                    None,
                    started.elapsed(),
                    false,
                    true,
                    Some(error.to_string()),
                );
                return;
            }
        };

        let mut delta: String = String::new();
        let mut sent_at: Instant = Instant::now();

        for chunk in stream {
            match chunk {
                Ok(Chunk::Text(text)) => {
                    // The engine has stopped thinking, since this is the answer.
                    delta.push_str(&text);
                    if sent_at.elapsed() >= UPDATE_INTERVAL {
                        self.send(
                            mem::take(&mut delta),
                            tokens,
                            started.elapsed(),
                            false,
                            false,
                            None,
                        );
                        sent_at = Instant::now();
                    }
                }
                Ok(Chunk::Thinking(_)) => {
                    // Nothing of the thinking is shown, but how long it has been going is, so a
                    // reply which is all thinking still ticks along rather than sitting still.
                    if sent_at.elapsed() >= UPDATE_INTERVAL {
                        self.send(
                            mem::take(&mut delta),
                            tokens,
                            started.elapsed(),
                            true,
                            false,
                            None,
                        );
                        sent_at = Instant::now();
                    }
                }
                Ok(Chunk::Tokens {
                    total,
                    thinking: thought,
                }) => tokens = Some((total, thought)),
                Ok(Chunk::Done) => {
                    self.send(delta, tokens, started.elapsed(), false, true, None);
                    return;
                }
                Err(error) => {
                    // Whatever arrived before the error is still worth keeping, so it is sent
                    // along with it rather than thrown away.
                    self.send(
                        delta,
                        tokens,
                        started.elapsed(),
                        false,
                        true,
                        Some(error.to_string()),
                    );
                    return;
                }
            }
        }

        // The stream always ends with a piece which says it is over, so this is only reached if
        // that changes.
        self.send(delta, tokens, started.elapsed(), false, true, None);
    }

    /// Pass a piece of the reply on.
    fn send(
        &self,
        delta: String,
        tokens: Option<(usize, usize)>,
        duration: Duration,
        thinking: bool,
        done: bool,
        error: Option<String>,
    ) {
        let streamed: Streamed = Streamed {
            delta,
            tokens,
            duration,
            thinking,
            done,
            error,
        };

        if let Err(error) = self.results_tx.send(streamed) {
            log::error!("Failed to send a piece of the reply: {}", error);
        }
    }
}

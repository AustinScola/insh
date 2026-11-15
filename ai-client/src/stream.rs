//! The reply to a chat, streamed a piece at a time.

use std::collections::VecDeque;
use std::io::BufReader;

use super::api::Api;
use super::chunk::Chunk;
use super::delta::Delta;
use super::error::ChatError;
use super::message::Message;
use super::reply::Reply;
use super::sse::{SseEvent, SseEvents};

use typed_builder::TypedBuilder;
use ureq::BodyReader;

/// The reply to a chat, streamed a piece at a time.
///
/// The last item is always either [`Chunk::Done`] or an error, so that whatever is waiting on the
/// reply learns that it is over exactly once. Once it is over, [`ChatStream::into_reply`] has the
/// whole of it, including the tools which the engine asked to use.
#[derive(TypedBuilder)]
pub struct ChatStream {
    /// The kind of API which the reply is coming from.
    api: Api,
    /// The events of the reply.
    events: SseEvents<BufReader<BodyReader<'static>>>,
    /// The pieces of an event which have not been gone through yet.
    #[builder(default)]
    deltas: VecDeque<Delta>,
    /// The reply so far.
    #[builder(default)]
    reply: Reply,
    /// Whether the reply is over.
    #[builder(default)]
    done: bool,
}

impl ChatStream {
    /// Return the reply as a message to send back as part of the conversation.
    pub fn into_reply(self) -> Message {
        return self.reply.into_message();
    }
}

impl Iterator for ChatStream {
    type Item = Result<Chunk, ChatError>;

    fn next(&mut self) -> Option<Result<Chunk, ChatError>> {
        if self.done {
            return None;
        }

        loop {
            if let Some(delta) = self.deltas.pop_front() {
                self.reply.add(&delta);

                match delta {
                    Delta::Text(text) => return Some(Ok(Chunk::Text(text))),
                    Delta::Thinking { text, .. } => return Some(Ok(Chunk::Thinking(text))),
                    Delta::Tokens { total, thinking } => {
                        return Some(Ok(Chunk::Tokens { total, thinking }))
                    }
                    Delta::Done => {
                        self.done = true;
                        return Some(Ok(Chunk::Done));
                    }
                    // These are only worth anything put together, which the reply does.
                    Delta::Signature { .. } | Delta::Redacted { .. } | Delta::ToolCall { .. } => {
                        continue
                    }
                }
            }

            let event: SseEvent = match self.events.next() {
                // The body ended without the inference engine saying that the reply was over.
                // Whatever arrived is all there is, so the reply is over now.
                None => {
                    self.done = true;
                    return Some(Ok(Chunk::Done));
                }
                Some(Err(error)) => {
                    self.done = true;
                    return Some(Err(ChatError::ReadFailed { error }));
                }
                Some(Ok(event)) => event,
            };

            match self.api.deltas(&event) {
                Ok(deltas) => self.deltas.extend(deltas),
                Err(error) => {
                    self.done = true;
                    return Some(Err(error));
                }
            }
        }
    }
}

//! The reply to a chat, streamed a piece at a time.

use std::io::BufReader;

use super::api::Api;
use super::chunk::Chunk;
use super::error::ChatError;
use super::sse::{SseEvent, SseEvents};

use typed_builder::TypedBuilder;
use ureq::BodyReader;

/// The reply to a chat, streamed a piece at a time.
///
/// The last item is always either [`Chunk::Done`] or an error, so that whatever is waiting on the
/// reply learns that it is over exactly once.
#[derive(TypedBuilder)]
pub struct ChatStream {
    /// The kind of API which the reply is coming from.
    api: Api,
    /// The events of the reply.
    events: SseEvents<BufReader<BodyReader<'static>>>,
    /// Whether the reply is over.
    #[builder(default)]
    done: bool,
}

impl Iterator for ChatStream {
    type Item = Result<Chunk, ChatError>;

    fn next(&mut self) -> Option<Result<Chunk, ChatError>> {
        if self.done {
            return None;
        }

        loop {
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

            match self.api.chunk(&event) {
                // The event was one of the several which carry no part of the reply.
                Ok(None) => continue,
                Ok(Some(Chunk::Done)) => {
                    self.done = true;
                    return Some(Ok(Chunk::Done));
                }
                Ok(Some(chunk)) => return Some(Ok(chunk)),
                Err(error) => {
                    self.done = true;
                    return Some(Err(error));
                }
            }
        }
    }
}

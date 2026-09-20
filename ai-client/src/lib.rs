/*!
Streams replies from an AI inference engine.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_return)]

mod anthropic;
mod api;
mod chunk;
mod client;
mod error;
mod message;
mod openai;
mod sse;
mod stream;

pub use api::Api;
pub use chunk::Chunk;
pub use client::{AiClient, ChatOptions};
pub use error::ChatError;
pub use message::{Message, Role};
pub use stream::ChatStream;

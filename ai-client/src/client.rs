//! A client of an AI inference engine.

use std::io::BufReader;

use super::api::Api;
use super::chunk::Chunk;
use super::error::ChatError;
use super::message::Message;
use super::sse::SseEvents;
use super::stream::ChatStream;

use serde_json::Value;
use typed_builder::TypedBuilder;
use ureq::http::Response;
use ureq::{Agent, Body, RequestBuilder};

/// The most tokens a reply is allowed to be when the caller does not say.
const DEFAULT_MAX_TOKENS: usize = 4096;

/// A client of an AI inference engine.
#[derive(TypedBuilder, Debug, Clone)]
pub struct AiClient {
    /// The URL which the inference engine is reachable at, without a path.
    #[builder(setter(into))]
    base_url: String,
    /// The key which authenticates requests.
    #[builder(setter(into))]
    api_key: String,
    /// The model which replies.
    #[builder(setter(into))]
    model: String,
    /// The kind of API which the inference engine speaks.
    api: Api,
    /// The most tokens a reply is allowed to be.
    #[builder(default = DEFAULT_MAX_TOKENS)]
    max_tokens: usize,
}

impl AiClient {
    /// Return the same client with a different cap on how long a reply can be.
    pub fn with_max_tokens(&self, max_tokens: usize) -> Self {
        let mut client: Self = self.clone();
        client.max_tokens = max_tokens;
        client
    }
}

impl AiClient {
    /// Ask for a reply and wait for the whole of it.
    ///
    /// This is for the short answers which are not shown as they arrive, like naming a chat.
    pub fn complete(&self, options: ChatOptions) -> Result<String, ChatError> {
        let mut reply: String = String::new();

        for chunk in self.chat(options)? {
            match chunk? {
                Chunk::Text(text) => reply.push_str(&text),
                Chunk::Thinking(_) | Chunk::Tokens { .. } => {}
                Chunk::Done => break,
            }
        }

        return Ok(reply);
    }

    /// Ask for a reply to a conversation.
    pub fn chat(&self, options: ChatOptions) -> Result<ChatStream, ChatError> {
        let url: String = format!("{}{}", self.base_url.trim_end_matches('/'), self.api.path());
        let body: Value = self.api.body(
            &self.model,
            self.max_tokens,
            &options.messages,
            options.instructions.as_deref(),
        );

        let agent: Agent = Agent::config_builder()
            // A reply takes as long as it takes. A global timeout would cut off a long one part
            // way through.
            .timeout_global(None)
            // Without this a request which is refused is only a status code, and the body saying
            // why (an expired key, an unknown model) is thrown away.
            .http_status_as_error(false)
            .build()
            .into();

        let mut request: RequestBuilder<_> = agent.post(&url);
        for (name, value) in self.api.headers(&self.api_key) {
            request = request.header(name, value);
        }

        let mut response: Response<Body> = match request.send_json(&body) {
            Ok(response) => response,
            Err(error) => {
                return Err(ChatError::RequestFailed {
                    error: error.to_string(),
                })
            }
        };

        let code: u16 = response.status().as_u16();
        if !response.status().is_success() {
            let body: String = response
                .body_mut()
                .read_to_string()
                .unwrap_or_else(|error| format!("<the body could not be read: {}>", error));
            return Err(ChatError::Status { code, body });
        }

        let events = SseEvents::builder()
            .reader(BufReader::new(response.into_body().into_reader()))
            .build();

        return Ok(ChatStream::builder().api(self.api).events(events).build());
    }
}

/// What to ask an inference engine for.
#[derive(TypedBuilder, Debug, Clone)]
pub struct ChatOptions {
    /// The messages of the conversation so far, oldest first.
    pub messages: Vec<Message>,
    /// What the inference engine should be told about how to answer, if anything.
    #[builder(default, setter(into))]
    pub instructions: Option<String>,
}

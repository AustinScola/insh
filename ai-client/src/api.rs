//! The kind of API which an inference engine speaks.

use super::anthropic::Anthropic;
use super::chunk::Chunk;
use super::error::ChatError;
use super::message::Message;
use super::openai::OpenAi;
use super::sse::SseEvent;

use serde::Deserialize;
use serde_json::Value;

/// The kind of API which an inference engine speaks.
#[derive(Deserialize, Debug, Default, Clone, Copy, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Api {
    /// The Anthropic messages API.
    #[default]
    Anthropic,
    /// The OpenAI chat completions API, which Ollama and LM Studio also speak.
    #[serde(rename = "openai")]
    OpenAi,
}

impl Api {
    /// Return the path which replies are streamed from.
    pub fn path(&self) -> &'static str {
        return match self {
            Self::Anthropic => Anthropic::PATH,
            Self::OpenAi => OpenAi::PATH,
        };
    }

    /// Return the headers which authenticate a request and say what it carries.
    pub fn headers(&self, api_key: &str) -> Vec<(&'static str, String)> {
        let mut headers: Vec<(&'static str, String)> =
            vec![("content-type", "application/json".to_string())];

        match self {
            Self::Anthropic => {
                headers.push(("x-api-key", api_key.to_string()));
                headers.push(("anthropic-version", Anthropic::VERSION.to_string()));
            }
            Self::OpenAi => {
                headers.push(("authorization", format!("Bearer {}", api_key)));
            }
        }

        return headers;
    }

    /// Return the body of a request for a reply.
    pub fn body(
        &self,
        model: &str,
        max_tokens: usize,
        messages: &[Message],
        instructions: Option<&str>,
    ) -> Value {
        return match self {
            Self::Anthropic => Anthropic::body(model, max_tokens, messages, instructions),
            Self::OpenAi => OpenAi::body(model, max_tokens, messages, instructions),
        };
    }

    /// Return the piece of the reply which an event carries, if it carries one.
    pub fn chunk(&self, event: &SseEvent) -> Result<Option<Chunk>, ChatError> {
        return match self {
            Self::Anthropic => Anthropic::chunk(event),
            Self::OpenAi => OpenAi::chunk(event),
        };
    }
}

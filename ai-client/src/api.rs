//! The kind of API which an inference engine speaks.

use super::anthropic::Anthropic;
use super::client::ChatOptions;
use super::delta::Delta;
use super::error::ChatError;
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
    pub fn body(&self, model: &str, max_tokens: usize, options: &ChatOptions) -> Value {
        return match self {
            Self::Anthropic => Anthropic::body(model, max_tokens, options),
            Self::OpenAi => OpenAi::body(model, max_tokens, options),
        };
    }

    /// Return what an event adds to the reply.
    pub fn deltas(&self, event: &SseEvent) -> Result<Vec<Delta>, ChatError> {
        return match self {
            Self::Anthropic => Anthropic::deltas(event),
            Self::OpenAi => OpenAi::deltas(event),
        };
    }
}

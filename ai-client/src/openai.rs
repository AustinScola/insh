//! The OpenAI chat completions API, which Ollama and LM Studio also speak.

use super::chunk::Chunk;
use super::error::ChatError;
use super::message::Message;
use super::sse::SseEvent;

use serde_json::{json, Value};

/// The OpenAI chat completions API.
pub struct OpenAi;

impl OpenAi {
    /// The path which replies are streamed from.
    pub const PATH: &'static str = "/v1/chat/completions";

    /// The data which marks the end of a reply.
    const DONE: &'static str = "[DONE]";

    /// Return the body of a request for a reply.
    pub fn body(
        model: &str,
        max_tokens: usize,
        messages: &[Message],
        instructions: Option<&str>,
    ) -> Value {
        // Instructions go in a message of their own before the rest here, rather than in a field.
        let mut all: Vec<Value> = Vec::with_capacity(messages.len() + 1);
        if let Some(instructions) = instructions {
            all.push(json!({"role": "system", "content": instructions}));
        }
        for message in messages {
            all.push(json!(message));
        }

        return json!({
            "model": model,
            "max_tokens": max_tokens,
            "stream": true,
            "messages": all,
        });
    }

    /// Return the piece of the reply which an event carries, if it carries one.
    pub fn chunk(event: &SseEvent) -> Result<Option<Chunk>, ChatError> {
        if event.data.is_empty() {
            return Ok(None);
        }

        if event.data == Self::DONE {
            return Ok(Some(Chunk::Done));
        }

        let value: Value = match serde_json::from_str(&event.data) {
            Ok(value) => value,
            Err(error) => {
                return Err(ChatError::ParseFailed {
                    error: error.to_string(),
                })
            }
        };

        // Engines which are asked to say how many tokens a reply came to put it in the last piece
        // of it. The ones which are not asked leave it out, which is why this is not required.
        if let Some(usage) = value.get("usage") {
            if let Some(total) = usage.get("completion_tokens").and_then(Value::as_u64) {
                let thinking: u64 = usage
                    .get("completion_tokens_details")
                    .and_then(|details| details.get("reasoning_tokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);

                return Ok(Some(Chunk::Tokens {
                    total: total as usize,
                    thinking: thinking as usize,
                }));
            }
        }

        if let Some(error) = value.get("error") {
            let message: String = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("an unknown error")
                .to_string();
            return Err(ChatError::Reported { message });
        }

        let delta: Option<&Value> = value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("delta"));

        // Engines which reason before answering put it in a field of its own beside the answer.
        if let Some(thinking) = delta
            .and_then(|delta| delta.get("reasoning_content").or(delta.get("reasoning")))
            .and_then(Value::as_str)
        {
            if !thinking.is_empty() {
                return Ok(Some(Chunk::Thinking(thinking.to_string())));
            }
        }

        let content: Option<&str> = delta
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str);

        return match content {
            // The first event of a reply carries the role and an empty content, which is not
            // worth waking the interface up for.
            Some(text) if !text.is_empty() => Ok(Some(Chunk::Text(text.to_string()))),
            _ => Ok(None),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::message::Role;

    use test_case::test_case;

    /// Return an event carrying data.
    fn event(data: &str) -> SseEvent {
        SseEvent {
            name: None,
            data: data.to_string(),
        }
    }

    #[test_case(
        r#"{"choices":[{"delta":{"content":"hi"}}]}"#,
        Some(Chunk::Text("hi".to_string()));
        "a content delta"
    )]
    #[test_case("[DONE]", Some(Chunk::Done); "the end of the reply")]
    #[test_case(
        r#"{"choices":[{"delta":{"role":"assistant","content":""}}]}"#,
        None;
        "the opening delta which only carries the role"
    )]
    #[test_case(
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
        None;
        "a delta with nothing in it"
    )]
    #[test_case(r#"{"choices":[]}"#, None; "no choices")]
    #[test_case("", None; "an event with no data")]
    fn test_chunk(data: &str, expected: Option<Chunk>) {
        assert_eq!(OpenAi::chunk(&event(data)).unwrap(), expected);
    }

    #[test]
    fn test_instructions_come_first_as_a_message() {
        let messages = vec![Message::builder().role(Role::User).content("hi").build()];

        let with = OpenAi::body("m", 10, &messages, Some("Be terse."));
        let without = OpenAi::body("m", 10, &messages, None);

        let all = with["messages"].as_array().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0]["role"], "system");
        assert_eq!(all[0]["content"], "Be terse.");
        assert_eq!(all[1]["role"], "user");

        assert_eq!(without["messages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_reported_error() {
        let data: &str = r#"{"error":{"message":"model not found","type":"invalid_request"}}"#;

        match OpenAi::chunk(&event(data)) {
            Err(ChatError::Reported { message }) => assert_eq!(message, "model not found"),
            other => panic!("Expected a reported error but got {:?}.", other),
        }
    }

    #[test]
    fn test_malformed_data() {
        match OpenAi::chunk(&event("{not json")) {
            Err(ChatError::ParseFailed { .. }) => {}
            other => panic!("Expected a parse failure but got {:?}.", other),
        }
    }
}

//! The Anthropic messages API.

use super::chunk::Chunk;
use super::error::ChatError;
use super::message::Message;
use super::sse::SseEvent;

use serde_json::{json, Value};

/// The Anthropic messages API.
pub struct Anthropic;

impl Anthropic {
    /// The path which replies are streamed from.
    pub const PATH: &'static str = "/v1/messages";

    /// The version of the API which requests are made against.
    pub const VERSION: &'static str = "2023-06-01";

    /// Return the body of a request for a reply.
    pub fn body(
        model: &str,
        max_tokens: usize,
        messages: &[Message],
        instructions: Option<&str>,
    ) -> Value {
        let mut body: Value = json!({
            "model": model,
            "max_tokens": max_tokens,
            "stream": true,
            "messages": messages,
        });

        // Instructions are their own field here rather than a message.
        if let Some(instructions) = instructions {
            body["system"] = json!(instructions);
        }

        return body;
    }

    /// Return the piece of the reply which an event carries, if it carries one.
    pub fn chunk(event: &SseEvent) -> Result<Option<Chunk>, ChatError> {
        if event.data.is_empty() {
            return Ok(None);
        }

        let value: Value = match serde_json::from_str(&event.data) {
            Ok(value) => value,
            Err(error) => {
                return Err(ChatError::ParseFailed {
                    error: error.to_string(),
                })
            }
        };

        match value.get("type").and_then(Value::as_str) {
            Some("content_block_delta") => {
                let delta: &Value = match value.get("delta") {
                    Some(delta) => delta,
                    None => return Ok(None),
                };

                // A model which thinks before it answers sends what it is thinking as well as the
                // answer, and the two are kept apart.
                return Ok(match delta.get("type").and_then(Value::as_str) {
                    Some("text_delta") => delta
                        .get("text")
                        .and_then(Value::as_str)
                        .map(|text| Chunk::Text(text.to_string())),
                    Some("thinking_delta") => delta
                        .get("thinking")
                        .and_then(Value::as_str)
                        .map(|text| Chunk::Thinking(text.to_string())),
                    _ => None,
                });
            }
            // How many tokens the reply came to is said once, just before the end of it. The
            // thinking is counted in the total, which is why a one word answer can cost a lot.
            Some("message_delta") => {
                let usage: &Value = match value.get("usage") {
                    Some(usage) => usage,
                    None => return Ok(None),
                };

                let total: Option<u64> = usage.get("output_tokens").and_then(Value::as_u64);
                let thinking: u64 = usage
                    .get("output_tokens_details")
                    .and_then(|details| details.get("thinking_tokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);

                return Ok(total.map(|total| Chunk::Tokens {
                    total: total as usize,
                    thinking: thinking as usize,
                }));
            }
            Some("message_stop") => {
                return Ok(Some(Chunk::Done));
            }
            Some("error") => {
                let message: String = value
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("an unknown error")
                    .to_string();
                return Err(ChatError::Reported { message });
            }
            _ => {
                return Ok(None);
            }
        }
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
        r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"hi"}}"#,
        Some(Chunk::Text("hi".to_string()));
        "a text delta"
    )]
    #[test_case(
        r#"{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hm"}}"#,
        Some(Chunk::Thinking("hm".to_string()));
        "a thinking delta"
    )]
    #[test_case(
        r#"{"type":"content_block_delta","delta":{"type":"signature_delta","signature":"x"}}"#,
        None;
        "a delta which is neither"
    )]
    #[test_case(r#"{"type":"message_stop"}"#, Some(Chunk::Done); "the end of the reply")]
    #[test_case(r#"{"type":"ping"}"#, None; "a ping")]
    #[test_case(r#"{"type":"message_start"}"#, None; "the start of a message")]
    #[test_case(r#"{"type":"content_block_delta"}"#, None; "a delta with nothing in it")]
    #[test_case("", None; "an event with no data")]
    fn test_chunk(data: &str, expected: Option<Chunk>) {
        assert_eq!(Anthropic::chunk(&event(data)).unwrap(), expected);
    }

    #[test]
    fn test_instructions_are_a_field_of_their_own() {
        let messages = vec![Message::builder().role(Role::User).content("hi").build()];

        let with = Anthropic::body("m", 10, &messages, Some("Be terse."));
        let without = Anthropic::body("m", 10, &messages, None);

        assert_eq!(with["system"], "Be terse.");
        assert_eq!(with["messages"].as_array().unwrap().len(), 1);
        // Nothing is sent at all when there is nothing to say.
        assert!(without.get("system").is_none());
    }

    #[test]
    fn test_reported_error() {
        let data: &str =
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;

        match Anthropic::chunk(&event(data)) {
            Err(ChatError::Reported { message }) => assert_eq!(message, "Overloaded"),
            other => panic!("Expected a reported error but got {:?}.", other),
        }
    }

    #[test]
    fn test_malformed_data() {
        match Anthropic::chunk(&event("{not json")) {
            Err(ChatError::ParseFailed { .. }) => {}
            other => panic!("Expected a parse failure but got {:?}.", other),
        }
    }
}

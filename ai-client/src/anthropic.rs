//! The Anthropic messages API.

use super::client::ChatOptions;
use super::delta::Delta;
use super::error::ChatError;
use super::message::Message;
use super::sse::SseEvent;
use super::tool::{Thought, ToolChoice};

use serde_json::{json, Value};

/// The Anthropic messages API.
pub struct Anthropic;

impl Anthropic {
    /// The path which replies are streamed from.
    pub const PATH: &'static str = "/v1/messages";

    /// The version of the API which requests are made against.
    pub const VERSION: &'static str = "2023-06-01";

    /// Return the body of a request for a reply.
    pub fn body(model: &str, max_tokens: usize, options: &ChatOptions) -> Value {
        let messages: Vec<Value> = options.messages.iter().map(Self::message).collect();

        let mut body: Value = json!({
            "model": model,
            "max_tokens": max_tokens,
            "stream": true,
            "messages": messages,
        });

        // Instructions are their own field here rather than a message.
        if let Some(instructions) = &options.instructions {
            body["system"] = json!(instructions);
        }

        if !options.tools.is_empty() {
            let tools: Vec<Value> = options
                .tools
                .iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "input_schema": tool.input_schema,
                    })
                })
                .collect();
            body["tools"] = json!(tools);

            if options.tool_choice == ToolChoice::None {
                body["tool_choice"] = json!({"type": "none"});
            }
        }

        return body;
    }

    /// Return a message the way this API takes it.
    ///
    /// A message which is only text is sent as text. Anything else is a list of blocks: what was
    /// thought comes first, the results of tools have to come before any text, and the tools which
    /// were asked for come after it.
    fn message(message: &Message) -> Value {
        if message.thoughts.is_empty()
            && message.tool_calls.is_empty()
            && message.tool_results.is_empty()
        {
            return json!({"role": message.role, "content": message.content});
        }

        let mut blocks: Vec<Value> = Vec::new();

        for thought in &message.thoughts {
            blocks.push(match thought {
                Thought::Thinking { text, signature } => {
                    json!({"type": "thinking", "thinking": text, "signature": signature})
                }
                Thought::Redacted { data } => json!({"type": "redacted_thinking", "data": data}),
            });
        }

        for result in &message.tool_results {
            blocks.push(json!({
                "type": "tool_result",
                "tool_use_id": result.id,
                "content": result.content,
                "is_error": result.is_error,
            }));
        }

        if !message.content.is_empty() {
            blocks.push(json!({"type": "text", "text": message.content}));
        }

        for call in &message.tool_calls {
            blocks.push(json!({
                "type": "tool_use",
                "id": call.id,
                "name": call.name,
                "input": call.input,
            }));
        }

        return json!({"role": message.role, "content": blocks});
    }

    /// Return what an event adds to the reply.
    pub fn deltas(event: &SseEvent) -> Result<Vec<Delta>, ChatError> {
        if event.data.is_empty() {
            return Ok(vec![]);
        }

        let value: Value = match serde_json::from_str(&event.data) {
            Ok(value) => value,
            Err(error) => {
                return Err(ChatError::ParseFailed {
                    error: error.to_string(),
                })
            }
        };

        let index: usize = value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;

        match value.get("type").and_then(Value::as_str) {
            // A tool call says which tool it is when it starts and what the tool is given after,
            // and thinking which was encrypted arrives all at once.
            Some("content_block_start") => {
                let block: &Value = match value.get("content_block") {
                    Some(block) => block,
                    None => return Ok(vec![]),
                };

                return Ok(match block.get("type").and_then(Value::as_str) {
                    Some("tool_use") => vec![Delta::ToolCall {
                        index,
                        id: block.get("id").and_then(Value::as_str).map(str::to_string),
                        name: block
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        input: String::new(),
                    }],
                    Some("redacted_thinking") => vec![Delta::Redacted {
                        index,
                        data: block
                            .get("data")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    }],
                    _ => vec![],
                });
            }
            Some("content_block_delta") => {
                let delta: &Value = match value.get("delta") {
                    Some(delta) => delta,
                    None => return Ok(vec![]),
                };
                let field = |name: &str| -> Option<String> {
                    delta.get(name).and_then(Value::as_str).map(str::to_string)
                };

                // A model which thinks before it answers sends what it is thinking as well as the
                // answer, and the two are kept apart.
                let delta: Option<Delta> = match delta.get("type").and_then(Value::as_str) {
                    Some("text_delta") => field("text").map(Delta::Text),
                    Some("thinking_delta") => field("thinking").map(|text| Delta::Thinking {
                        index: Some(index),
                        text,
                    }),
                    Some("signature_delta") => {
                        field("signature").map(|signature| Delta::Signature { index, signature })
                    }
                    Some("input_json_delta") => {
                        field("partial_json").map(|input| Delta::ToolCall {
                            index,
                            id: None,
                            name: None,
                            input,
                        })
                    }
                    _ => None,
                };
                return Ok(delta.into_iter().collect());
            }
            // How many tokens the reply came to is said once, just before the end of it. The
            // thinking is counted in the total, which is why a one word answer can cost a lot.
            Some("message_delta") => {
                let usage: &Value = match value.get("usage") {
                    Some(usage) => usage,
                    None => return Ok(vec![]),
                };

                let total: Option<u64> = usage.get("output_tokens").and_then(Value::as_u64);
                let thinking: u64 = usage
                    .get("output_tokens_details")
                    .and_then(|details| details.get("thinking_tokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);

                return Ok(total
                    .map(|total| Delta::Tokens {
                        total: total as usize,
                        thinking: thinking as usize,
                    })
                    .into_iter()
                    .collect());
            }
            Some("message_stop") => {
                return Ok(vec![Delta::Done]);
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
                return Ok(vec![]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::message::Role;
    use crate::tool::{ToolCall, ToolDefinition, ToolResult};

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
        vec![Delta::Text("hi".to_string())];
        "a text delta"
    )]
    #[test_case(
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hm"}}"#,
        vec![Delta::Thinking { index: Some(0), text: "hm".to_string() }];
        "a thinking delta"
    )]
    #[test_case(
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"x"}}"#,
        vec![Delta::Signature { index: 0, signature: "x".to_string() }];
        "a signature delta"
    )]
    #[test_case(
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"web_search","input":{}}}"#,
        vec![Delta::ToolCall {
            index: 1,
            id: Some("toolu_1".to_string()),
            name: Some("web_search".to_string()),
            input: String::new(),
        }];
        "the start of a tool call"
    )]
    #[test_case(
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"q"}}"#,
        vec![Delta::ToolCall { index: 1, id: None, name: None, input: "{\"q".to_string() }];
        "a piece of the input of a tool call"
    )]
    #[test_case(
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"redacted_thinking","data":"x"}}"#,
        vec![Delta::Redacted { index: 0, data: "x".to_string() }];
        "redacted thinking"
    )]
    #[test_case(
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        vec![];
        "the start of a text block"
    )]
    #[test_case(r#"{"type":"message_stop"}"#, vec![Delta::Done]; "the end of the reply")]
    #[test_case(r#"{"type":"ping"}"#, vec![]; "a ping")]
    #[test_case(r#"{"type":"message_start"}"#, vec![]; "the start of a message")]
    #[test_case(r#"{"type":"content_block_delta"}"#, vec![]; "a delta with nothing in it")]
    #[test_case("", vec![]; "an event with no data")]
    fn test_deltas(data: &str, expected: Vec<Delta>) {
        assert_eq!(Anthropic::deltas(&event(data)).unwrap(), expected);
    }

    #[test]
    fn test_instructions_are_a_field_of_their_own() {
        let messages = vec![Message::builder().role(Role::User).content("hi").build()];

        let with = Anthropic::body(
            "m",
            10,
            &ChatOptions::builder()
                .messages(messages.clone())
                .instructions(Some("Be terse.".to_string()))
                .build(),
        );
        let without = Anthropic::body("m", 10, &ChatOptions::builder().messages(messages).build());

        assert_eq!(with["system"], "Be terse.");
        assert_eq!(with["messages"].as_array().unwrap().len(), 1);
        // Nothing is sent at all when there is nothing to say.
        assert!(without.get("system").is_none());
        assert!(without.get("tools").is_none());
    }

    #[test]
    fn test_a_conversation_which_used_a_tool() {
        let messages = vec![
            Message::builder().role(Role::User).content("hi").build(),
            Message::builder()
                .role(Role::Assistant)
                .content("Looking.")
                .thoughts(vec![Thought::Thinking {
                    text: "Hm.".to_string(),
                    signature: "sig".to_string(),
                }])
                .tool_calls(vec![ToolCall::builder()
                    .id("toolu_1")
                    .name("web_search")
                    .input(json!({"query": "q"}))
                    .build()])
                .build(),
            Message::builder()
                .role(Role::User)
                .tool_results(vec![ToolResult::builder()
                    .id("toolu_1")
                    .content("Results.")
                    .build()])
                .build(),
        ];
        let options = ChatOptions::builder()
            .messages(messages)
            .tools(vec![ToolDefinition::builder()
                .name("web_search")
                .description("Search the web.")
                .input_schema(json!({"type": "object"}))
                .build()])
            .tool_choice(ToolChoice::None)
            .build();

        let body = Anthropic::body("m", 10, &options);

        assert_eq!(body["messages"][0]["content"], "hi");
        assert_eq!(
            body["messages"][1]["content"],
            json!([
                {"type": "thinking", "thinking": "Hm.", "signature": "sig"},
                {"type": "text", "text": "Looking."},
                {"type": "tool_use", "id": "toolu_1", "name": "web_search", "input": {"query": "q"}},
            ])
        );
        assert_eq!(
            body["messages"][2]["content"],
            json!([
                {"type": "tool_result", "tool_use_id": "toolu_1", "content": "Results.", "is_error": false},
            ])
        );
        assert_eq!(body["tools"][0]["name"], "web_search");
        assert_eq!(body["tools"][0]["input_schema"], json!({"type": "object"}));
        assert_eq!(body["tool_choice"], json!({"type": "none"}));
    }

    #[test]
    fn test_reported_error() {
        let data: &str =
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;

        match Anthropic::deltas(&event(data)) {
            Err(ChatError::Reported { message }) => assert_eq!(message, "Overloaded"),
            other => panic!("Expected a reported error but got {:?}.", other),
        }
    }

    #[test]
    fn test_malformed_data() {
        match Anthropic::deltas(&event("{not json")) {
            Err(ChatError::ParseFailed { .. }) => {}
            other => panic!("Expected a parse failure but got {:?}.", other),
        }
    }
}

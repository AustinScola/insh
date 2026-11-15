//! The OpenAI chat completions API, which Ollama and LM Studio also speak.

use super::client::ChatOptions;
use super::delta::Delta;
use super::error::ChatError;
use super::message::Message;
use super::sse::SseEvent;
use super::tool::ToolChoice;

use serde_json::{json, Value};

/// The OpenAI chat completions API.
pub struct OpenAi;

impl OpenAi {
    /// The path which replies are streamed from.
    pub const PATH: &'static str = "/v1/chat/completions";

    /// The data which marks the end of a reply.
    const DONE: &'static str = "[DONE]";

    /// Return the body of a request for a reply.
    pub fn body(model: &str, max_tokens: usize, options: &ChatOptions) -> Value {
        // Instructions go in a message of their own before the rest here, rather than in a field.
        let mut all: Vec<Value> = Vec::with_capacity(options.messages.len() + 1);
        if let Some(instructions) = &options.instructions {
            all.push(json!({"role": "system", "content": instructions}));
        }
        for message in &options.messages {
            all.extend(Self::messages(message));
        }

        let mut body: Value = json!({
            "model": model,
            "max_tokens": max_tokens,
            "stream": true,
            "messages": all,
        });

        if !options.tools.is_empty() {
            let tools: Vec<Value> = options
                .tools
                .iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.input_schema,
                        },
                    })
                })
                .collect();
            body["tools"] = json!(tools);

            if options.tool_choice == ToolChoice::None {
                body["tool_choice"] = json!("none");
            }
        }

        return body;
    }

    /// Return a message the way this API takes it, which can be more than one.
    ///
    /// The result of each tool is a message of its own here. Thinking is never sent back, since
    /// nothing which speaks this API asks for it.
    fn messages(message: &Message) -> Vec<Value> {
        let mut messages: Vec<Value> = Vec::new();

        for result in &message.tool_results {
            messages.push(json!({
                "role": "tool",
                "tool_call_id": result.id,
                "content": result.content,
            }));
        }

        if !message.tool_calls.is_empty() {
            let calls: Vec<Value> = message
                .tool_calls
                .iter()
                .map(|call| {
                    json!({
                        "id": call.id,
                        "type": "function",
                        "function": {
                            "name": call.name,
                            // The input is a string of JSON here rather than JSON.
                            "arguments": call.input.to_string(),
                        },
                    })
                })
                .collect();
            let content: Value = match message.content.is_empty() {
                true => Value::Null,
                false => json!(message.content),
            };
            messages.push(json!({"role": message.role, "content": content, "tool_calls": calls}));
        } else if !message.content.is_empty() || message.tool_results.is_empty() {
            messages.push(json!({"role": message.role, "content": message.content}));
        }

        return messages;
    }

    /// Return what an event adds to the reply.
    pub fn deltas(event: &SseEvent) -> Result<Vec<Delta>, ChatError> {
        if event.data.is_empty() {
            return Ok(vec![]);
        }

        if event.data == Self::DONE {
            return Ok(vec![Delta::Done]);
        }

        let value: Value = match serde_json::from_str(&event.data) {
            Ok(value) => value,
            Err(error) => {
                return Err(ChatError::ParseFailed {
                    error: error.to_string(),
                })
            }
        };

        if let Some(error) = value.get("error") {
            let message: String = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("an unknown error")
                .to_string();
            return Err(ChatError::Reported { message });
        }

        let mut deltas: Vec<Delta> = Vec::new();

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
                deltas.push(Delta::Thinking {
                    index: None,
                    text: thinking.to_string(),
                });
            }
        }

        // The first event of a reply carries the role and an empty content, which is not worth
        // waking the interface up for.
        if let Some(text) = delta
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
        {
            if !text.is_empty() {
                deltas.push(Delta::Text(text.to_string()));
            }
        }

        let calls: &[Value] = delta
            .and_then(|delta| delta.get("tool_calls"))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for (position, call) in calls.iter().enumerate() {
            let function: Option<&Value> = call.get("function");
            let input: String = match function.and_then(|function| function.get("arguments")) {
                Some(Value::String(arguments)) => arguments.to_string(),
                // Some engines send the input whole, as JSON rather than a string of it.
                Some(arguments @ Value::Object(_)) => arguments.to_string(),
                _ => String::new(),
            };
            deltas.push(Delta::ToolCall {
                index: call
                    .get("index")
                    .and_then(Value::as_u64)
                    .map(|index| index as usize)
                    .unwrap_or(position),
                id: call.get("id").and_then(Value::as_str).map(str::to_string),
                name: function
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                input,
            });
        }

        // Engines which are asked to say how many tokens a reply came to put it in the last piece
        // of it. The ones which are not asked leave it out, which is why this is not required.
        if let Some(usage) = value.get("usage") {
            if let Some(total) = usage.get("completion_tokens").and_then(Value::as_u64) {
                let thinking: u64 = usage
                    .get("completion_tokens_details")
                    .and_then(|details| details.get("reasoning_tokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);

                deltas.push(Delta::Tokens {
                    total: total as usize,
                    thinking: thinking as usize,
                });
            }
        }

        return Ok(deltas);
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
        r#"{"choices":[{"delta":{"content":"hi"}}]}"#,
        vec![Delta::Text("hi".to_string())];
        "a content delta"
    )]
    #[test_case("[DONE]", vec![Delta::Done]; "the end of the reply")]
    #[test_case(
        r#"{"choices":[{"delta":{"role":"assistant","content":""}}]}"#,
        vec![];
        "the opening delta which only carries the role"
    )]
    #[test_case(
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
        vec![];
        "a delta with nothing in it"
    )]
    #[test_case(
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"web_search","arguments":""}}]}}]}"#,
        vec![Delta::ToolCall {
            index: 0,
            id: Some("call_1".to_string()),
            name: Some("web_search".to_string()),
            input: String::new(),
        }];
        "the start of a tool call"
    )]
    #[test_case(
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"q"}}]}}]}"#,
        vec![Delta::ToolCall { index: 0, id: None, name: None, input: "{\"q".to_string() }];
        "a piece of the input of a tool call"
    )]
    #[test_case(
        r#"{"choices":[{"delta":{"tool_calls":[{"id":"c","function":{"name":"n","arguments":{"q":1}}}]}}]}"#,
        vec![Delta::ToolCall {
            index: 0,
            id: Some("c".to_string()),
            name: Some("n".to_string()),
            input: r#"{"q":1}"#.to_string(),
        }];
        "a tool call sent whole"
    )]
    #[test_case(r#"{"choices":[]}"#, vec![]; "no choices")]
    #[test_case("", vec![]; "an event with no data")]
    fn test_deltas(data: &str, expected: Vec<Delta>) {
        assert_eq!(OpenAi::deltas(&event(data)).unwrap(), expected);
    }

    #[test]
    fn test_instructions_come_first_as_a_message() {
        let messages = vec![Message::builder().role(Role::User).content("hi").build()];

        let with = OpenAi::body(
            "m",
            10,
            &ChatOptions::builder()
                .messages(messages.clone())
                .instructions(Some("Be terse.".to_string()))
                .build(),
        );
        let without = OpenAi::body("m", 10, &ChatOptions::builder().messages(messages).build());

        let all = with["messages"].as_array().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0]["role"], "system");
        assert_eq!(all[0]["content"], "Be terse.");
        assert_eq!(all[1]["role"], "user");

        assert_eq!(without["messages"].as_array().unwrap().len(), 1);
        assert!(without.get("tools").is_none());
    }

    #[test]
    fn test_a_conversation_which_used_a_tool() {
        let messages = vec![
            Message::builder()
                .role(Role::Assistant)
                .tool_calls(vec![ToolCall::builder()
                    .id("call_1")
                    .name("web_search")
                    .input(json!({"query": "q"}))
                    .build()])
                .build(),
            Message::builder()
                .role(Role::User)
                .tool_results(vec![ToolResult::builder()
                    .id("call_1")
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

        let body = OpenAi::body("m", 10, &options);

        assert_eq!(
            body["messages"],
            json!([
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "web_search", "arguments": "{\"query\":\"q\"}"},
                    }],
                },
                {"role": "tool", "tool_call_id": "call_1", "content": "Results."},
            ])
        );
        assert_eq!(body["tools"][0]["function"]["name"], "web_search");
        assert_eq!(body["tool_choice"], "none");
    }

    #[test]
    fn test_reported_error() {
        let data: &str = r#"{"error":{"message":"model not found","type":"invalid_request"}}"#;

        match OpenAi::deltas(&event(data)) {
            Err(ChatError::Reported { message }) => assert_eq!(message, "model not found"),
            other => panic!("Expected a reported error but got {:?}.", other),
        }
    }

    #[test]
    fn test_malformed_data() {
        match OpenAi::deltas(&event("{not json")) {
            Err(ChatError::ParseFailed { .. }) => {}
            other => panic!("Expected a parse failure but got {:?}.", other),
        }
    }
}

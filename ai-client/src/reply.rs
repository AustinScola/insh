//! A reply put back together from its pieces.

use std::collections::BTreeMap;

use super::delta::Delta;
use super::message::{Message, Role};
use super::tool::{Thought, ToolCall};

use serde_json::{Map, Value};

/// A reply put back together from its pieces.
///
/// Thinking is kept by the block of the reply it is in, which is what its pieces say it belongs
/// to, and in the order of those blocks. Tool calls are kept in the order they started.
#[derive(Debug, Default)]
pub struct Reply {
    /// The text of the reply.
    text: String,
    /// What the engine thought, by block.
    thoughts: BTreeMap<usize, PartialThought>,
    /// The tools which the engine asked to use, in the order they started.
    tool_calls: Vec<PartialToolCall>,
    /// Which of the tool calls the pieces with each index go to.
    tool_call_indexes: BTreeMap<usize, usize>,
}

/// Thinking which is still arriving.
#[derive(Debug)]
enum PartialThought {
    /// Thinking which can be read.
    Thinking {
        /// What has been thought so far.
        text: String,
        /// What the engine checks the thinking against, once it has said.
        signature: String,
    },
    /// Thinking which was encrypted before it was sent.
    Redacted {
        /// The encrypted thinking.
        data: String,
    },
}

/// A tool call which is still arriving.
#[derive(Debug, Default)]
struct PartialToolCall {
    /// What the engine calls this use of the tool.
    id: String,
    /// The name of the tool.
    name: String,
    /// The JSON of what the tool is given, so far.
    input: String,
}

impl Reply {
    /// Add a piece of the reply.
    pub fn add(&mut self, delta: &Delta) {
        match delta {
            Delta::Text(text) => self.text.push_str(text),
            Delta::Thinking {
                index: Some(index),
                text,
            } => {
                if let PartialThought::Thinking { text: so_far, .. } = self.thinking(*index) {
                    so_far.push_str(text);
                }
            }
            Delta::Signature { index, signature } => {
                if let PartialThought::Thinking {
                    signature: so_far, ..
                } = self.thinking(*index)
                {
                    so_far.push_str(signature);
                }
            }
            Delta::Redacted { index, data } => {
                self.thoughts.insert(
                    *index,
                    PartialThought::Redacted {
                        data: data.to_string(),
                    },
                );
            }
            Delta::ToolCall {
                index,
                id,
                name,
                input,
            } => {
                let call: &mut PartialToolCall = self.tool_call(*index, id.as_deref());
                if let Some(id) = id {
                    call.id.push_str(id);
                }
                if let Some(name) = name {
                    call.name.push_str(name);
                }
                call.input.push_str(input);
            }
            Delta::Thinking { index: None, .. } | Delta::Tokens { .. } | Delta::Done => {}
        }
    }

    /// Return the tool call which a piece with an index goes to, starting one if it is the first.
    ///
    /// Most engines give each call of a reply an index of its own. Some send each call whole in
    /// an event of its own and give every one the same index, so a piece which says what the call
    /// is called when the call at its index is called something else starts another one.
    fn tool_call(&mut self, index: usize, id: Option<&str>) -> &mut PartialToolCall {
        let existing: Option<usize> = self.tool_call_indexes.get(&index).copied();
        let position: usize = match existing {
            Some(position) => {
                let called: &str = &self.tool_calls[position].id;
                match id {
                    Some(id) if !called.is_empty() && called != id => None,
                    _ => Some(position),
                }
            }
            None => None,
        }
        .unwrap_or_else(|| {
            self.tool_calls.push(PartialToolCall::default());
            let position: usize = self.tool_calls.len() - 1;
            self.tool_call_indexes.insert(index, position);
            position
        });

        return &mut self.tool_calls[position];
    }

    /// Return the thinking in a block, starting it if this is the first of it.
    fn thinking(&mut self, index: usize) -> &mut PartialThought {
        return self
            .thoughts
            .entry(index)
            .or_insert_with(|| PartialThought::Thinking {
                text: String::new(),
                signature: String::new(),
            });
    }

    /// Return the reply as a message to send back as part of the conversation.
    pub fn into_message(self) -> Message {
        let thoughts: Vec<Thought> = self
            .thoughts
            .into_values()
            .map(|thought| match thought {
                PartialThought::Thinking { text, signature } => {
                    Thought::Thinking { text, signature }
                }
                PartialThought::Redacted { data } => Thought::Redacted { data },
            })
            .collect();

        let tool_calls: Vec<ToolCall> = self
            .tool_calls
            .into_iter()
            .map(|call| {
                // A tool which takes nothing gets no input at all rather than an empty object.
                let (input, malformed): (Value, Option<String>) = match call.input.trim() {
                    "" => (Value::Object(Map::new()), None),
                    text => match serde_json::from_str(text) {
                        Ok(input @ Value::Object(_)) => (input, None),
                        Ok(_) => (
                            Value::Object(Map::new()),
                            Some("The input is not a JSON object.".to_string()),
                        ),
                        Err(error) => (
                            Value::Object(Map::new()),
                            Some(format!("The input is not valid JSON: {}", error)),
                        ),
                    },
                };
                ToolCall::builder()
                    .id(call.id)
                    .name(call.name)
                    .input(input)
                    .malformed(malformed)
                    .build()
            })
            .collect();

        return Message::builder()
            .role(Role::Assistant)
            .content(self.text)
            .thoughts(thoughts)
            .tool_calls(tool_calls)
            .build();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn test_a_tool_call_arriving_in_pieces() {
        let mut reply: Reply = Reply::default();
        for delta in [
            Delta::Text("Let me look.".to_string()),
            Delta::ToolCall {
                index: 1,
                id: Some("call_1".to_string()),
                name: Some("web_search".to_string()),
                input: String::new(),
            },
            Delta::ToolCall {
                index: 1,
                id: None,
                name: None,
                input: r#"{"query": "#.to_string(),
            },
            Delta::ToolCall {
                index: 1,
                id: None,
                name: None,
                input: r#""rust"}"#.to_string(),
            },
        ] {
            reply.add(&delta);
        }

        let message: Message = reply.into_message();

        assert_eq!(message.content, "Let me look.");
        assert_eq!(
            message.tool_calls,
            vec![ToolCall::builder()
                .id("call_1")
                .name("web_search")
                .input(json!({"query": "rust"}))
                .build()]
        );
    }

    /// Calls which each arrive whole with the same index are kept apart rather than run together.
    #[test]
    fn test_calls_sent_whole_with_the_same_index() {
        let mut reply: Reply = Reply::default();
        for (id, query) in [("call_1", "one"), ("call_2", "two")] {
            reply.add(&Delta::ToolCall {
                index: 0,
                id: Some(id.to_string()),
                name: Some("web_search".to_string()),
                input: format!(r#"{{"query": "{}"}}"#, query),
            });
        }

        let calls: Vec<ToolCall> = reply.into_message().tool_calls;

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].input, json!({"query": "one"}));
        assert_eq!(calls[1].id, "call_2");
        assert_eq!(calls[1].input, json!({"query": "two"}));
    }

    #[test]
    fn test_a_tool_call_with_no_input() {
        let mut reply: Reply = Reply::default();
        reply.add(&Delta::ToolCall {
            index: 0,
            id: Some("call_1".to_string()),
            name: Some("now".to_string()),
            input: String::new(),
        });

        let call: ToolCall = reply.into_message().tool_calls.remove(0);
        assert_eq!(call.input, json!({}));
        assert_eq!(call.malformed, None);
    }

    /// Input which is not JSON is sent back as nothing, and why is kept to tell the engine.
    #[test]
    fn test_a_tool_call_with_malformed_input() {
        let mut reply: Reply = Reply::default();
        reply.add(&Delta::ToolCall {
            index: 0,
            id: Some("call_1".to_string()),
            name: Some("web_search".to_string()),
            input: r#"{"query": "cut off"#.to_string(),
        });

        let call: ToolCall = reply.into_message().tool_calls.remove(0);
        assert_eq!(call.input, json!({}));
        assert!(call.malformed.is_some());
    }

    #[test]
    fn test_thinking_is_kept_with_its_signature() {
        let mut reply: Reply = Reply::default();
        for delta in [
            Delta::Thinking {
                index: Some(0),
                text: "Hm".to_string(),
            },
            Delta::Thinking {
                index: Some(0),
                text: "m.".to_string(),
            },
            Delta::Signature {
                index: 0,
                signature: "abc".to_string(),
            },
            Delta::Redacted {
                index: 1,
                data: "xyz".to_string(),
            },
            // Thinking which is not kept to be sent back is not kept.
            Delta::Thinking {
                index: None,
                text: "Ignored.".to_string(),
            },
        ] {
            reply.add(&delta);
        }

        assert_eq!(
            reply.into_message().thoughts,
            vec![
                Thought::Thinking {
                    text: "Hmm.".to_string(),
                    signature: "abc".to_string(),
                },
                Thought::Redacted {
                    data: "xyz".to_string(),
                },
            ]
        );
    }
}

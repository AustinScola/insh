//! A message in a conversation.

use super::tool::{Thought, ToolCall, ToolResult};

use serde::Serialize;
use typed_builder::TypedBuilder;

/// Who sent a message.
#[derive(Serialize, Debug, Clone, Copy, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The person using insh.
    User,
    /// The inference engine.
    Assistant,
}

/// A message in a conversation.
#[derive(TypedBuilder, Debug, Clone, PartialEq)]
pub struct Message {
    /// Who sent the message.
    pub role: Role,
    /// What the message says.
    #[builder(default, setter(into))]
    pub content: String,
    /// What the engine thought before it said this, when it is the engine which said it.
    #[builder(default)]
    pub thoughts: Vec<Thought>,
    /// The tools which the engine asked to use, when it is the engine which said it.
    #[builder(default)]
    pub tool_calls: Vec<ToolCall>,
    /// What came of the tools which the engine asked to use in the message before this one.
    #[builder(default)]
    pub tool_results: Vec<ToolResult>,
}

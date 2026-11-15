//! The tools which an inference engine can ask to use.

use serde_json::Value;
use typed_builder::TypedBuilder;

/// A tool which an inference engine can ask to use.
#[derive(TypedBuilder, Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    /// The name which the engine asks for the tool by.
    #[builder(setter(into))]
    pub name: String,
    /// What the tool does and when to use it, which is all the engine has to go on.
    #[builder(setter(into))]
    pub description: String,
    /// The JSON schema of what the tool is given.
    pub input_schema: Value,
}

/// Whether an inference engine is allowed to ask to use a tool.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum ToolChoice {
    /// The engine decides.
    #[default]
    Auto,
    /// The engine has to answer without one.
    ///
    /// The tools are still sent, since an engine refuses a conversation which used tools it is no
    /// longer told about.
    None,
}

/// An inference engine asking to use a tool.
#[derive(TypedBuilder, Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// What the engine calls this use of the tool, which the result is matched up with.
    #[builder(setter(into))]
    pub id: String,
    /// The name of the tool.
    #[builder(setter(into))]
    pub name: String,
    /// What the tool is given.
    pub input: Value,
    /// Why what the engine said to give the tool could not be used, when it could not.
    ///
    /// The input is left empty then, since an engine refuses a conversation in which a tool was
    /// given anything else, and this is what the engine is told instead of what the tool says.
    #[builder(default)]
    pub malformed: Option<String>,
}

/// What came of using a tool.
#[derive(TypedBuilder, Debug, Clone, Eq, PartialEq)]
pub struct ToolResult {
    /// The id of the call which this is the result of.
    #[builder(setter(into))]
    pub id: String,
    /// What the tool said.
    #[builder(setter(into))]
    pub content: String,
    /// Whether the tool failed, in which case what it said is why.
    #[builder(default)]
    pub is_error: bool,
}

/// What an inference engine thought before it answered, kept to be sent back to it.
///
/// Anthropic refuses a conversation in which a reply which used a tool is missing what was thought
/// before it, and checks that it is unchanged, so it is kept exactly as it came.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Thought {
    /// Thinking which can be read.
    Thinking {
        /// What was thought.
        text: String,
        /// What the engine checks the thinking against.
        signature: String,
    },
    /// Thinking which was encrypted before it was sent.
    Redacted {
        /// The encrypted thinking.
        data: String,
    },
}

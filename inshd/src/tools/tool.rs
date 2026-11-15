//! A tool which the inference engine can use.

use ai_client::ToolDefinition;

use serde_json::Value;

/// A tool which the inference engine can use.
///
/// The tools which the engine asks for at once are used at once, which is why a tool has to be
/// shareable between threads.
pub trait Tool: Send + Sync {
    /// Return the name which the inference engine asks for the tool by.
    fn name(&self) -> &'static str;

    /// Return what the inference engine is told about the tool.
    fn definition(&self) -> ToolDefinition;

    /// Return what to show while the tool is being used with some input.
    fn activity(&self, input: &Value) -> String;

    /// Use the tool, and return what it said or why it failed.
    fn call(&self, input: &Value) -> Result<String, String>;
}

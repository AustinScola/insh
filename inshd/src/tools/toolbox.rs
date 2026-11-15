//! The tools which the inference engine can use in a reply.

use std::thread::{self, ScopedJoinHandle};

use super::tool::Tool;
use super::web_fetch::WebFetch;
use super::web_search::WebSearch;
use crate::config::AiToolsConfig;

use ai_client::{ToolCall, ToolDefinition, ToolResult};
use web_client::WebClient;

/// The tools which the inference engine can use in a reply.
#[derive(Default)]
pub struct Toolbox {
    /// The tools.
    tools: Vec<Box<dyn Tool>>,
}

impl Toolbox {
    /// Return the tools which are configured.
    pub fn from_config(config: &AiToolsConfig) -> Self {
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();

        // Searching and reading go through the same provider, so the web is one thing to
        // configure and two tools to use.
        if let Some(web) = config.web() {
            let client: WebClient = WebClient::builder()
                .provider(web.provider())
                .api_key(web.api_key())
                .base_url(web.base_url().cloned())
                .num_results(web.search().num_results())
                .highlight_characters(web.search().highlight_characters())
                .max_characters(web.fetch().max_characters())
                .build();
            tools.push(Box::new(
                WebSearch::builder().client(client.clone()).build(),
            ));
            tools.push(Box::new(WebFetch::builder().client(client).build()));
        }

        return Self { tools };
    }

    /// Return what the inference engine is told about the tools.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|tool| tool.definition()).collect()
    }

    /// Return the tool which a call asks for, if there is one by that name.
    fn tool(&self, call: &ToolCall) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|tool| tool.name() == call.name)
            .map(|tool| tool.as_ref())
    }

    /// Return what to show while some calls are being made.
    ///
    /// Only the first is said, with how many others there are, so that it fits on a line.
    pub fn activity(&self, calls: &[ToolCall]) -> Option<String> {
        let first: &ToolCall = calls.first()?;
        let activity: String = match self.tool(first) {
            Some(tool) => tool.activity(&first.input),
            None => format!("Using {}", first.name),
        };

        return Some(match calls.len() {
            1 => activity,
            count => format!("{} and {} more", activity, count - 1),
        });
    }

    /// Make some calls at once, and return what came of each in the order they were asked for.
    ///
    /// Each call is a request of its own which can take seconds, so waiting on them one after
    /// another would make waiting on all of them take as long as all of them added up.
    pub fn call_all(&self, calls: &[ToolCall]) -> Vec<ToolResult> {
        return thread::scope(|scope| {
            let handles: Vec<ScopedJoinHandle<ToolResult>> = calls
                .iter()
                .map(|call| scope.spawn(move || self.call(call)))
                .collect();

            handles
                .into_iter()
                .zip(calls)
                .map(|(handle, call)| {
                    handle.join().unwrap_or_else(|_| {
                        log::error!("The tool {:?} panicked.", call.name);
                        ToolResult::builder()
                            .id(call.id.clone())
                            .content("The tool failed unexpectedly.")
                            .is_error(true)
                            .build()
                    })
                })
                .collect()
        });
    }

    /// Make a call and return what came of it.
    ///
    /// A tool failing is something the inference engine is told about rather than the end of the
    /// reply, since it can often do something about it, like search for something else.
    fn call(&self, call: &ToolCall) -> ToolResult {
        let result: Result<String, String> = match (self.tool(call), &call.malformed) {
            (None, _) => Err(format!("There is no tool named {:?}.", call.name)),
            (Some(_), Some(malformed)) => Err(malformed.clone()),
            (Some(tool), None) => tool.call(&call.input),
        };

        let (content, is_error): (String, bool) = match result {
            Ok(content) => (content, false),
            Err(error) => {
                log::error!("Failed to use the tool {:?}: {}", call.name, error);
                (error, true)
            }
        };

        return ToolResult::builder()
            .id(call.id.clone())
            .content(content)
            .is_error(is_error)
            .build();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    use serde_json::{json, Value};

    /// A tool which says back what it is given, after waiting for as long as it is told to.
    struct Echo;

    impl Tool for Echo {
        fn name(&self) -> &'static str {
            "echo"
        }

        fn definition(&self) -> ToolDefinition {
            ToolDefinition::builder()
                .name(self.name())
                .description("Say it back.")
                .input_schema(json!({"type": "object"}))
                .build()
        }

        fn activity(&self, input: &Value) -> String {
            format!("Echoing {}", input["text"])
        }

        fn call(&self, input: &Value) -> Result<String, String> {
            std::thread::sleep(Duration::from_millis(input["wait"].as_u64().unwrap_or(0)));
            Ok(input["text"].as_str().unwrap_or_default().to_string())
        }
    }

    /// Return a toolbox with only the echo in it.
    fn toolbox() -> Toolbox {
        Toolbox {
            tools: vec![Box::new(Echo)],
        }
    }

    /// Return a call to a tool.
    fn call(id: &str, name: &str, input: Value) -> ToolCall {
        ToolCall::builder().id(id).name(name).input(input).build()
    }

    /// What comes of each call is in the order the calls were asked for, whichever finished first.
    #[test]
    fn test_results_are_in_the_order_asked_for() {
        let calls: Vec<ToolCall> = vec![
            call("1", "echo", json!({"text": "slow", "wait": 50})),
            call("2", "echo", json!({"text": "fast"})),
            call("3", "missing", json!({})),
        ];

        let results: Vec<ToolResult> = toolbox().call_all(&calls);

        assert_eq!(
            results
                .iter()
                .map(|result| (result.id.as_str(), result.is_error))
                .collect::<Vec<_>>(),
            vec![("1", false), ("2", false), ("3", true)]
        );
        assert_eq!(results[0].content, "slow");
        assert_eq!(results[1].content, "fast");
    }

    /// Input which could not be used is reported to the engine rather than given to the tool.
    #[test]
    fn test_malformed_input_is_reported() {
        let mut malformed: ToolCall = call("1", "echo", json!({}));
        malformed.malformed = Some("The input is not valid JSON.".to_string());

        let results: Vec<ToolResult> = toolbox().call_all(&[malformed]);

        assert!(results[0].is_error);
        assert_eq!(results[0].content, "The input is not valid JSON.");
    }

    /// Only the first of several calls is said, with how many others there are.
    #[test]
    fn test_activity_of_several_calls() {
        let one: ToolCall = call("1", "echo", json!({"text": "a"}));
        let two: ToolCall = call("2", "echo", json!({"text": "b"}));

        assert_eq!(toolbox().activity(&[]), None);
        assert_eq!(
            toolbox().activity(std::slice::from_ref(&one)),
            Some(r#"Echoing "a""#.to_string())
        );
        assert_eq!(
            toolbox().activity(&[one, two]),
            Some(r#"Echoing "a" and 1 more"#.to_string())
        );
    }
}

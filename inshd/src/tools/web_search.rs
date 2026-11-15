//! Searches the web.

use super::tool::Tool;

use ai_client::ToolDefinition;
use web_client::{SearchResult, WebClient};

use serde_json::{json, Value};
use typed_builder::TypedBuilder;

/// The name which the inference engine asks for the tool by.
const NAME: &str = "web_search";

/// What the inference engine is told about the tool.
const DESCRIPTION: &str = "Search the web. Use this for anything which may have changed since \
     your training data, anything recent, and anything you are unsure of. Each result has the \
     title, the URL, when it was published if that is known, and the passages of the page which \
     have most to do with the query. When those are not enough, read the page with web_fetch. \
     Say which URLs your answer came from.";

/// What goes between the results of a search.
const SEPARATOR: &str = "\n\n---\n\n";

/// Searches the web.
#[derive(TypedBuilder)]
pub struct WebSearch {
    /// What does the searching.
    client: WebClient,
}

impl WebSearch {
    /// Return the query which the tool is given, if it is given one.
    fn query(input: &Value) -> Option<&str> {
        input
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|query| !query.is_empty())
    }
}

impl Tool for WebSearch {
    fn name(&self) -> &'static str {
        NAME
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::builder()
            .name(NAME)
            .description(DESCRIPTION)
            .input_schema(json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "What to search for.",
                    },
                },
                "required": ["query"],
            }))
            .build()
    }

    fn activity(&self, input: &Value) -> String {
        match Self::query(input) {
            Some(query) => format!("Searching the web for \u{201c}{}\u{201d}", query),
            None => "Searching the web".to_string(),
        }
    }

    fn call(&self, input: &Value) -> Result<String, String> {
        let query: &str = Self::query(input).ok_or_else(|| "No query was given.".to_string())?;

        let results: Vec<SearchResult> = self
            .client
            .search(query)
            .map_err(|error| error.to_string())?;

        if results.is_empty() {
            return Ok("Nothing was found.".to_string());
        }

        return Ok(results
            .iter()
            .map(SearchResult::to_string)
            .collect::<Vec<String>>()
            .join(SEPARATOR));
    }
}

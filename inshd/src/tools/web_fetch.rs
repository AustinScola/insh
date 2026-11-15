//! Reads a page of the web.

use super::tool::Tool;

use ai_client::ToolDefinition;
use web_client::{Page, WebClient};

use serde_json::{json, Value};
use typed_builder::TypedBuilder;

/// The name which the inference engine asks for the tool by.
const NAME: &str = "web_fetch";

/// What the inference engine is told about the tool.
const DESCRIPTION: &str = "Read a page of the web. Use this to read a page which web_search \
     found when the passages it returned are not enough, or a URL which you were given. Returns \
     the title, the URL, when it was published if that is known, and the text of the page, which \
     is cut off if the page is long.";

/// Reads a page of the web.
#[derive(TypedBuilder)]
pub struct WebFetch {
    /// What does the reading.
    client: WebClient,
}

impl WebFetch {
    /// Return the URL which the tool is given, if it is given one.
    fn url(input: &Value) -> Option<&str> {
        input
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| !url.is_empty())
    }
}

impl Tool for WebFetch {
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
                    "url": {
                        "type": "string",
                        "description": "The URL of the page to read.",
                    },
                },
                "required": ["url"],
            }))
            .build()
    }

    fn activity(&self, input: &Value) -> String {
        match Self::url(input) {
            Some(url) => format!("Reading {}", url),
            None => "Reading a page".to_string(),
        }
    }

    fn call(&self, input: &Value) -> Result<String, String> {
        let url: &str = Self::url(input).ok_or_else(|| "No URL was given.".to_string())?;

        let page: Page = self.client.fetch(url).map_err(|error| error.to_string())?;

        return Ok(page.to_string());
    }
}

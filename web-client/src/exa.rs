//! The Exa API.

use super::error::WebError;
use super::page::Page;
use super::search_result::SearchResult;

use serde_json::{json, Value};

/// The Exa API.
pub struct Exa;

impl Exa {
    /// The URL which Exa is reachable at.
    pub const BASE_URL: &'static str = "https://api.exa.ai";

    /// The path which searches are made at.
    pub const SEARCH_PATH: &'static str = "/search";

    /// The path which pages are read at.
    pub const FETCH_PATH: &'static str = "/contents";

    /// Return the headers which authenticate a request and say what it carries.
    pub fn headers(api_key: &str) -> Vec<(&'static str, String)> {
        return vec![
            ("content-type", "application/json".to_string()),
            ("x-api-key", api_key.to_string()),
        ];
    }

    /// Return the body of a search.
    ///
    /// Only the passages of each page which have most to do with the query are asked for, rather
    /// than the page itself, so that a search is cheap and the pages worth reading can be read
    /// after.
    pub fn search_body(query: &str, num_results: usize, highlight_characters: usize) -> Value {
        return json!({
            "query": query,
            "type": "auto",
            "numResults": num_results,
            "contents": {
                "highlights": {
                    "maxCharacters": highlight_characters,
                },
            },
        });
    }

    /// Return the results in the body of the response to a search.
    pub fn search_results(body: &Value) -> Result<Vec<SearchResult>, WebError> {
        return Ok(Self::results(body)?
            .iter()
            .filter_map(|result| {
                // A result is no use without somewhere to go and look.
                let url: String = Self::field(result, "url")?;
                let highlights: Vec<String> = result
                    .get("highlights")
                    .and_then(Value::as_array)
                    .map(|highlights| {
                        highlights
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::trim)
                            .filter(|highlight| !highlight.is_empty())
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();

                Some(
                    SearchResult::builder()
                        .url(url)
                        .title(Self::field(result, "title"))
                        .published(Self::field(result, "publishedDate"))
                        .highlights(highlights)
                        .build(),
                )
            })
            .collect());
    }

    /// Return the body of a request to read a page.
    pub fn fetch_body(url: &str, max_characters: usize) -> Value {
        return json!({
            "urls": [url],
            "text": {
                "maxCharacters": max_characters,
            },
        });
    }

    /// Return the page in the body of the response to a request to read it.
    ///
    /// A page which could not be read is still a successful response, and says why in a status of
    /// its own rather than in the status of the response.
    pub fn page(body: &Value) -> Result<Page, WebError> {
        let status: Option<&Value> = body
            .get("statuses")
            .and_then(Value::as_array)
            .and_then(|statuses| statuses.first());
        if let Some(status) = status {
            if status.get("status").and_then(Value::as_str) == Some("error") {
                let error: Option<&Value> = status.get("error");
                let tag: &str = error
                    .and_then(|error| error.get("tag"))
                    .and_then(Value::as_str)
                    .unwrap_or("an unknown error");
                let code: Option<u64> = error
                    .and_then(|error| error.get("httpStatusCode"))
                    .and_then(Value::as_u64);
                return Err(WebError::Unreadable {
                    reason: match code {
                        Some(code) => format!("{} ({})", tag, code),
                        None => tag.to_string(),
                    },
                });
            }
        }

        let result: &Value = match Self::results(body)?.first() {
            Some(result) => result,
            None => {
                return Err(WebError::Unreadable {
                    reason: "Nothing was returned".to_string(),
                })
            }
        };

        let url: String = match Self::field(result, "url") {
            Some(url) => url,
            None => {
                return Err(WebError::ParseFailed {
                    error: "The page has no URL".to_string(),
                })
            }
        };

        return Ok(Page::builder()
            .url(url)
            .title(Self::field(result, "title"))
            .published(Self::field(result, "publishedDate"))
            .text(Self::field(result, "text").unwrap_or_default())
            .build());
    }

    /// Return the results in the body of a response.
    fn results(body: &Value) -> Result<&Vec<Value>, WebError> {
        return body
            .get("results")
            .and_then(Value::as_array)
            .ok_or_else(|| WebError::ParseFailed {
                error: "There are no results in the response".to_string(),
            });
    }

    /// Return a field of a result which is a string with something in it.
    fn field(result: &Value, name: &str) -> Option<String> {
        return result
            .get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_body() {
        assert_eq!(
            Exa::search_body("rust", 3, 100),
            json!({
                "query": "rust",
                "type": "auto",
                "numResults": 3,
                "contents": {"highlights": {"maxCharacters": 100}},
            })
        );
    }

    #[test]
    fn test_search_results() {
        let body: Value = json!({
            "requestId": "r",
            "results": [
                {
                    "title": "Rust",
                    "url": "https://www.rust-lang.org",
                    "publishedDate": "2026-01-02T00:00:00.000Z",
                    "author": null,
                    "highlights": ["A language.", " ", "Fast."],
                },
                {"url": "https://example.com", "title": ""},
                {"title": "Nowhere"},
            ],
        });

        assert_eq!(
            Exa::search_results(&body).unwrap(),
            vec![
                SearchResult::builder()
                    .url("https://www.rust-lang.org")
                    .title(Some("Rust".to_string()))
                    .published(Some("2026-01-02T00:00:00.000Z".to_string()))
                    .highlights(vec!["A language.".to_string(), "Fast.".to_string()])
                    .build(),
                SearchResult::builder().url("https://example.com").build(),
            ]
        );
    }

    #[test]
    fn test_no_search_results() {
        match Exa::search_results(&json!({"error": "bad"})) {
            Err(WebError::ParseFailed { .. }) => {}
            other => panic!("Expected a parse failure but got {:?}.", other),
        }
    }

    #[test]
    fn test_fetch_body() {
        assert_eq!(
            Exa::fetch_body("https://example.com", 100),
            json!({"urls": ["https://example.com"], "text": {"maxCharacters": 100}})
        );
    }

    #[test]
    fn test_page() {
        let body: Value = json!({
            "results": [{"url": "https://example.com", "title": "Example", "text": "Hello."}],
            "statuses": [{"id": "https://example.com", "status": "success", "source": "cached"}],
        });

        assert_eq!(
            Exa::page(&body).unwrap(),
            Page::builder()
                .url("https://example.com")
                .title(Some("Example".to_string()))
                .text("Hello.")
                .build()
        );
    }

    #[test]
    fn test_a_page_which_could_not_be_read() {
        let body: Value = json!({
            "results": [],
            "statuses": [{
                "id": "https://example.com",
                "status": "error",
                "error": {"tag": "CRAWL_NOT_FOUND", "httpStatusCode": 404},
            }],
        });

        match Exa::page(&body) {
            Err(WebError::Unreadable { reason }) => assert_eq!(reason, "CRAWL_NOT_FOUND (404)"),
            other => panic!("Expected the page to be unreadable but got {:?}.", other),
        }
    }
}

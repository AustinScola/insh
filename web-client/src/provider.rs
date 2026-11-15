//! A service which searches and reads the web.

use super::error::WebError;
use super::exa::Exa;
use super::page::Page;
use super::search_result::SearchResult;

use serde::Deserialize;
use serde_json::Value;

/// A service which searches and reads the web.
#[derive(Deserialize, Debug, Default, Clone, Copy, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// Exa.
    #[default]
    Exa,
}

impl Provider {
    /// Return the URL which the provider is reachable at, without a path.
    pub fn base_url(&self) -> &'static str {
        return match self {
            Self::Exa => Exa::BASE_URL,
        };
    }

    /// Return the headers which authenticate a request and say what it carries.
    pub fn headers(&self, api_key: &str) -> Vec<(&'static str, String)> {
        return match self {
            Self::Exa => Exa::headers(api_key),
        };
    }

    /// Return the path which searches are made at.
    pub fn search_path(&self) -> &'static str {
        return match self {
            Self::Exa => Exa::SEARCH_PATH,
        };
    }

    /// Return the body of a search.
    pub fn search_body(
        &self,
        query: &str,
        num_results: usize,
        highlight_characters: usize,
    ) -> Value {
        return match self {
            Self::Exa => Exa::search_body(query, num_results, highlight_characters),
        };
    }

    /// Return the results in the body of the response to a search.
    pub fn search_results(&self, body: &Value) -> Result<Vec<SearchResult>, WebError> {
        return match self {
            Self::Exa => Exa::search_results(body),
        };
    }

    /// Return the path which pages are read at.
    pub fn fetch_path(&self) -> &'static str {
        return match self {
            Self::Exa => Exa::FETCH_PATH,
        };
    }

    /// Return the body of a request to read a page.
    pub fn fetch_body(&self, url: &str, max_characters: usize) -> Value {
        return match self {
            Self::Exa => Exa::fetch_body(url, max_characters),
        };
    }

    /// Return the page in the body of the response to a request to read it.
    pub fn page(&self, body: &Value) -> Result<Page, WebError> {
        return match self {
            Self::Exa => Exa::page(body),
        };
    }
}

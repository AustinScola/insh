//! A client of a web provider.

use std::time::Duration;

use super::error::WebError;
use super::page::Page;
use super::provider::Provider;
use super::search_result::SearchResult;

use serde_json::Value;
use typed_builder::TypedBuilder;
use ureq::http::Response;
use ureq::tls::{RootCerts, TlsConfig};
use ureq::{Agent, Body, RequestBuilder};

/// How long a request is given before it is given up on.
///
/// Reading a page which the provider has not kept a copy of means it reading the page itself, so
/// this is longer than a search alone would need.
const TIMEOUT: Duration = Duration::from_secs(60);

/// A client of a web provider.
#[derive(TypedBuilder, Debug, Clone)]
pub struct WebClient {
    /// The service which does the searching and reading.
    provider: Provider,
    /// The key which authenticates requests.
    #[builder(setter(into))]
    api_key: String,
    /// The URL which the provider is reachable at, when it is not the usual one.
    #[builder(default, setter(into))]
    base_url: Option<String>,
    /// How many pages a search finds.
    num_results: usize,
    /// The most characters of the passages of each page which a search returns.
    highlight_characters: usize,
    /// The most characters of a page which reading it returns.
    max_characters: usize,
    /// What makes the requests, which is made once so that the roots it trusts are only loaded
    /// once.
    #[builder(default = WebClient::agent(), setter(skip))]
    agent: Agent,
}

impl WebClient {
    /// Return the pages which a query finds.
    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>, WebError> {
        let body: Value =
            self.provider
                .search_body(query, self.num_results, self.highlight_characters);
        let response: Value = self.post(self.provider.search_path(), &body)?;

        return self.provider.search_results(&response);
    }

    /// Return what a page says.
    pub fn fetch(&self, url: &str) -> Result<Page, WebError> {
        let body: Value = self.provider.fetch_body(url, self.max_characters);
        let response: Value = self.post(self.provider.fetch_path(), &body)?;

        return self.provider.page(&response);
    }

    /// Return what makes the requests.
    fn agent() -> Agent {
        return Agent::config_builder()
            // The engine is waiting on this, so a provider which never answers must not hold it up
            // forever.
            .timeout_global(Some(TIMEOUT))
            // Without this a request which is refused is only a status code, and the body saying
            // why (an expired key, a malformed query) is thrown away.
            .http_status_as_error(false)
            // Trust what the operating system trusts, the same as the inference engine client.
            .tls_config(
                TlsConfig::builder()
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .build()
            .into();
    }

    /// Send a request to the provider and return the body of its response.
    fn post(&self, path: &str, body: &Value) -> Result<Value, WebError> {
        let base_url: &str = self.base_url.as_deref().unwrap_or(self.provider.base_url());
        let url: String = format!("{}{}", base_url.trim_end_matches('/'), path);

        let mut request: RequestBuilder<_> = self.agent.post(&url);
        for (name, value) in self.provider.headers(&self.api_key) {
            request = request.header(name, value);
        }

        let mut response: Response<Body> = match request.send_json(body) {
            Ok(response) => response,
            Err(error) => {
                return Err(WebError::RequestFailed {
                    error: error.to_string(),
                })
            }
        };

        let code: u16 = response.status().as_u16();
        if !response.status().is_success() {
            let body: String = response
                .body_mut()
                .read_to_string()
                .unwrap_or_else(|error| format!("<the body could not be read: {}>", error));
            return Err(WebError::Status { code, body });
        }

        return response
            .body_mut()
            .read_json()
            .map_err(|error| WebError::ParseFailed {
                error: error.to_string(),
            });
    }
}

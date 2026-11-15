/*!
Searches and reads the web through a web provider.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_return)]

mod client;
mod error;
mod exa;
mod page;
mod provider;
mod search_result;

pub use client::WebClient;
pub use error::WebError;
pub use page::Page;
pub use provider::Provider;
pub use search_result::SearchResult;

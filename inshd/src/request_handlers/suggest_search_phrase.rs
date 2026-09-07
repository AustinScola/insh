//! Handles requests to suggest a search phrase.
use std::error::Error;

use insh_api::{
    ResponseParams, ResponseParamsAndLast, SuggestSearchPhraseRequestParams,
    SuggestSearchPhraseResponseParams,
};
use insh_db::{search_history, DbConnPool};

/// Handles a request to suggest a search phrase.
pub struct SuggestSearchPhrase {
    /// The partial search phrase to suggest a completion for.
    partial: String,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// If suggesting a search phrase is done.
    done: bool,
}

impl SuggestSearchPhrase {
    /// Return a new handler for suggesting a search phrase.
    pub fn new(params: &SuggestSearchPhraseRequestParams, db_conn_pool: DbConnPool) -> Self {
        Self {
            partial: params.partial().to_string(),
            db_conn_pool,
            done: false,
        }
    }

    /// Return the most recent phrase starting with the partial one.
    fn suggest(&self) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
        return search_history::suggest(&self.db_conn_pool, &self.partial);
    }
}

impl Iterator for SuggestSearchPhrase {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let suggestion: Option<String> = match self.suggest() {
            Ok(suggestion) => suggestion,
            Err(error) => {
                log::error!("Failed to suggest a search phrase: {}", error);
                None
            }
        };

        let response_params: ResponseParams = ResponseParams::SuggestSearchPhrase(
            SuggestSearchPhraseResponseParams::builder()
                .suggestion(suggestion)
                .build(),
        );

        self.done = true;

        Some(
            ResponseParamsAndLast::builder()
                .response_params(response_params)
                .last(true)
                .build(),
        )
    }
}

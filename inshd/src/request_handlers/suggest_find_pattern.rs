//! Handles requests to suggest a find pattern.
use std::error::Error;

use insh_api::{
    ResponseParams, ResponseParamsAndLast, SuggestFindPatternRequestParams,
    SuggestFindPatternResponseParams,
};
use insh_db::{find_history, DbConnPool};

/// Handles a request to suggest a find pattern.
pub struct SuggestFindPattern {
    /// The partial find pattern to suggest a completion for.
    partial: String,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// If suggesting a find pattern is done.
    done: bool,
}

impl SuggestFindPattern {
    /// Return a new handler for suggesting a find pattern.
    pub fn new(params: &SuggestFindPatternRequestParams, db_conn_pool: DbConnPool) -> Self {
        Self {
            partial: params.partial().to_string(),
            db_conn_pool,
            done: false,
        }
    }

    /// Return the most recent pattern starting with the partial one.
    fn suggest(&self) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
        return find_history::suggest(&self.db_conn_pool, &self.partial);
    }
}

impl Iterator for SuggestFindPattern {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let suggestion: Option<String> = match self.suggest() {
            Ok(suggestion) => suggestion,
            Err(error) => {
                log::error!("Failed to suggest a find pattern: {}", error);
                None
            }
        };

        let response_params: ResponseParams = ResponseParams::SuggestFindPattern(
            SuggestFindPatternResponseParams::builder()
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

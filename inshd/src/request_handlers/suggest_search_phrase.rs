//! Handles requests to suggest a search phrase.
use insh_api::{
    ResponseParams, ResponseParamsAndLast, SuggestSearchPhraseRequestParams,
    SuggestSearchPhraseResponseParams,
};

use crate::data::Data;

/// Handles a request to suggest a search phrase.
pub struct SuggestSearchPhrase {
    /// The partial search phrase to suggest a completion for.
    partial: String,
    /// If suggesting a search phrase is done.
    done: bool,
}

impl SuggestSearchPhrase {
    /// Return a new handler for suggesting a search phrase.
    pub fn new(params: &SuggestSearchPhraseRequestParams) -> Self {
        Self {
            partial: params.partial().to_string(),
            done: false,
        }
    }
}

impl Iterator for SuggestSearchPhrase {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let data: Data = Data::read();
        let mut searches: Vec<String> = data.searcher.history.into();

        // Searches are stored oldest to newest so we want to iterate in reverse.
        searches.reverse();
        let mut suggestion: Option<String> = None;
        for search in searches.iter() {
            if search.starts_with(&self.partial) {
                suggestion = Some(search.to_string());
                break;
            }
        }

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

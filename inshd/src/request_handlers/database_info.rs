//! Handles requests for information about the database.
use insh_api::{DatabaseInfoResponseParams, ResponseParams, ResponseParamsAndLast};

use typed_builder::TypedBuilder;

/// Handles a request for information about the database.
#[derive(TypedBuilder)]
pub struct DatabaseInfo {
    /// The version of the database.
    version: String,
    /// If the information about the database has been responded with.
    #[builder(default = false)]
    done: bool,
}

impl Iterator for DatabaseInfo {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let response_params: ResponseParams = ResponseParams::DatabaseInfo(
            DatabaseInfoResponseParams::builder()
                .version(self.version.clone())
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

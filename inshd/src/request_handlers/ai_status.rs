//! Handles requests to find out whether an AI inference engine is configured.
use insh_api::{AiStatusResponseParams, ResponseParams, ResponseParamsAndLast};

use typed_builder::TypedBuilder;

/// Handles a request to find out whether an AI inference engine is configured.
#[derive(TypedBuilder)]
pub struct AiStatus {
    /// Whether an AI inference engine is configured.
    configured: bool,
    /// If answering is done.
    #[builder(default)]
    done: bool,
}

impl Iterator for AiStatus {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }
        self.done = true;

        return Some(
            ResponseParamsAndLast::builder()
                .response_params(ResponseParams::AiStatus(
                    AiStatusResponseParams::builder()
                        .configured(self.configured)
                        .build(),
                ))
                .last(true)
                .build(),
        );
    }
}

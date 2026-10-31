//! Handles requests to take note that a command was run for the multiplexer.
use crate::config::Config;

use insh_api::{ResponseParams, ResponseParamsAndLast, RunCommandResponseParams};
use insh_db::{command_history, DbConnPool};

use typed_builder::TypedBuilder;

/// Handles a request to take note that a command was run for the multiplexer.
#[derive(TypedBuilder)]
pub struct RunCommand {
    /// The command which was run.
    #[builder(setter(into))]
    command: String,
    /// The configuration for inshd.
    config: Config,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// If taking note of the command is done.
    #[builder(default)]
    done: bool,
}

impl Iterator for RunCommand {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let max_length: usize = self.config.multiplexing().commands().history().length();
        if let Err(error) = command_history::add(&self.db_conn_pool, &self.command, max_length) {
            log::error!(
                "Failed to add the command to the command history: {}",
                error
            );
        }

        self.done = true;

        Some(
            ResponseParamsAndLast::builder()
                .response_params(ResponseParams::RunCommand(
                    RunCommandResponseParams::builder().build(),
                ))
                .last(true)
                .build(),
        )
    }
}

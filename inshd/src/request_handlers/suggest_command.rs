//! Handles requests to suggest a command for the multiplexer.
use std::error::Error;

use insh_api::{
    ResponseParams, ResponseParamsAndLast, SuggestCommandResponseParams, COMMAND_NAMES,
};
use insh_db::{command_history, DbConnPool};

use typed_builder::TypedBuilder;

/// Handles a request to suggest a command for the multiplexer.
#[derive(TypedBuilder)]
pub struct SuggestCommand {
    /// The command so far.
    #[builder(setter(into))]
    partial: String,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// If suggesting a command is done.
    #[builder(default)]
    done: bool,
}

impl SuggestCommand {
    /// Return the command to suggest, which is nothing at all if there is not one.
    ///
    /// A command which has been run is preferred over one which has not, since it is the one more
    /// likely to be wanted again, and it carries whatever was typed after the name of it along
    /// with it.
    fn suggest(&self) -> Option<String> {
        let partial: &str = self.partial.trim_start();
        if partial.is_empty() {
            return None;
        }

        match self.suggest_run(partial) {
            Ok(Some(command)) => {
                return Some(command);
            }
            Ok(None) => {}
            Err(error) => {
                log::error!("Failed to read the command history: {}", error);
            }
        }

        self.suggest_name(partial)
    }

    /// Return the command which was most recently run which starts with the command so far.
    fn suggest_run(&self, partial: &str) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
        return command_history::suggest(&self.db_conn_pool, partial);
    }

    /// Return the name of the one command which starts with the command so far.
    ///
    /// Nothing is suggested once something has been typed after the name, since the name is all
    /// there is to go by, and nothing is suggested when more than one command starts that way,
    /// since there is no saying which is meant.
    fn suggest_name(&self, partial: &str) -> Option<String> {
        if partial.contains(char::is_whitespace) {
            return None;
        }

        let mut matching = COMMAND_NAMES
            .iter()
            .filter(|name| name.starts_with(partial));

        let only: &str = matching.next()?;
        if matching.next().is_some() {
            return None;
        }

        return Some(only.to_string());
    }
}

impl Iterator for SuggestCommand {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let response_params: ResponseParams = ResponseParams::SuggestCommand(
            SuggestCommandResponseParams::builder()
                .suggestion(self.suggest())
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

//! The commands which have been run for the multiplexer.
use std::error::Error;

use crate::helpers::like_prefix_pattern;
use crate::schema::command_history;
use crate::DbConnPool;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

/// Record that a command was run, dropping the commands which no longer fit in the history.
///
/// A command is only stored once. Running a command again moves it to the front of the history
/// rather than adding another entry for it.
pub fn add(
    db_conn_pool: &DbConnPool,
    command: &str,
    max_length: usize,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    connection.transaction(|connection| {
        diesel::insert_into(command_history::table)
            .values(command_history::command.eq(command))
            .on_conflict(command_history::command)
            .do_update()
            .set(command_history::last_run.eq(now))
            .execute(connection)?;

        // The commands which no longer fit in the history. There are none of these until the
        // history is full.
        let ids_to_drop: Vec<i64> = command_history::table
            .select(command_history::id)
            .order(command_history::last_run.desc())
            .offset(max_length as i64)
            .load::<i64>(connection)?;

        if !ids_to_drop.is_empty() {
            diesel::delete(command_history::table.filter(command_history::id.eq_any(&ids_to_drop)))
                .execute(connection)?;
        }

        return Ok::<(), DieselError>(());
    })?;

    return Ok(());
}

/// Return the most recently run command starting with a partial one.
pub fn suggest(
    db_conn_pool: &DbConnPool,
    partial: &str,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    let suggestion: Option<String> = command_history::table
        .select(command_history::command)
        .filter(command_history::command.like(like_prefix_pattern(partial)))
        .order(command_history::last_run.desc())
        .first::<String>(&mut connection)
        .optional()?;

    return Ok(suggestion);
}

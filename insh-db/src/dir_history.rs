//! The directories which have been visited.
use std::error::Error;

use crate::helpers::like_prefix_pattern;
use crate::schema::dir_history;
use crate::DbConnPool;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

/// Record that a directory was visited, dropping the directories which no longer fit in the
/// history.
///
/// A directory is only stored once. Visiting a directory again moves it to the front of the history
/// rather than adding another entry for it.
pub fn add(
    db_conn_pool: &DbConnPool,
    path: &str,
    max_length: usize,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    connection.transaction(|connection| {
        diesel::insert_into(dir_history::table)
            .values(dir_history::path.eq(path))
            .on_conflict(dir_history::path)
            .do_update()
            .set(dir_history::last_visited.eq(now))
            .execute(connection)?;

        // The directories which no longer fit in the history. There are none of these until the
        // history is full.
        let ids_to_drop: Vec<i64> = dir_history::table
            .select(dir_history::id)
            .order(dir_history::last_visited.desc())
            .offset(max_length as i64)
            .load::<i64>(connection)?;

        if !ids_to_drop.is_empty() {
            diesel::delete(dir_history::table.filter(dir_history::id.eq_any(&ids_to_drop)))
                .execute(connection)?;
        }

        return Ok::<(), DieselError>(());
    })?;

    return Ok(());
}

/// Return the paths starting with a partial one, most recently visited first.
///
/// More than one is returned because a directory which has been visited may since have been moved
/// or removed, and the caller is the one which can tell.
pub fn suggest(
    db_conn_pool: &DbConnPool,
    partial: &str,
    limit: usize,
) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    let suggestions: Vec<String> = dir_history::table
        .select(dir_history::path)
        .filter(dir_history::path.like(like_prefix_pattern(partial)))
        .order(dir_history::last_visited.desc())
        .limit(limit as i64)
        .load::<String>(&mut connection)?;

    return Ok(suggestions);
}

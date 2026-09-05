//! The patterns which have been used to find files.
use std::error::Error;

use crate::helpers::like_prefix_pattern;
use crate::schema::find_history;
use crate::DbConnPool;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

/// Record that a pattern was used to find files, dropping the patterns which no longer fit in the
/// history.
///
/// A pattern is only stored once. Using a pattern again moves it to the front of the history rather
/// than adding another entry for it.
pub fn add(
    db_conn_pool: &DbConnPool,
    pattern: &str,
    max_length: usize,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    connection.transaction(|connection| {
        diesel::insert_into(find_history::table)
            .values(find_history::pattern.eq(pattern))
            .on_conflict(find_history::pattern)
            .do_update()
            .set(find_history::last_found.eq(now))
            .execute(connection)?;

        // The patterns which no longer fit in the history. There are none of these until the
        // history is full.
        let ids_to_drop: Vec<i64> = find_history::table
            .select(find_history::id)
            .order(find_history::last_found.desc())
            .offset(max_length as i64)
            .load::<i64>(connection)?;

        if !ids_to_drop.is_empty() {
            diesel::delete(find_history::table.filter(find_history::id.eq_any(&ids_to_drop)))
                .execute(connection)?;
        }

        return Ok::<(), DieselError>(());
    })?;

    return Ok(());
}

/// Return the most recently used pattern which starts with a partial pattern.
pub fn suggest(
    db_conn_pool: &DbConnPool,
    partial: &str,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    let suggestion: Option<String> = find_history::table
        .select(find_history::pattern)
        .filter(find_history::pattern.like(like_prefix_pattern(partial)))
        .order(find_history::last_found.desc())
        .first::<String>(&mut connection)
        .optional()?;

    return Ok(suggestion);
}

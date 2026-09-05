//! The phrases which have been searched for.
use std::error::Error;

use crate::helpers::like_prefix_pattern;
use crate::schema::search_history;
use crate::DbConnPool;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

/// Record that a phrase was searched for, dropping the phrases which no longer fit in the history.
///
/// A phrase is only stored once. Searching for a phrase again moves it to the front of the history
/// rather than adding another entry for it.
pub fn add(
    db_conn_pool: &DbConnPool,
    phrase: &str,
    max_length: usize,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    connection.transaction(|connection| {
        diesel::insert_into(search_history::table)
            .values(search_history::phrase.eq(phrase))
            .on_conflict(search_history::phrase)
            .do_update()
            .set(search_history::last_searched.eq(now))
            .execute(connection)?;

        // The phrases which no longer fit in the history. There are none of these until the
        // history is full.
        let ids_to_drop: Vec<i64> = search_history::table
            .select(search_history::id)
            .order(search_history::last_searched.desc())
            .offset(max_length as i64)
            .load::<i64>(connection)?;

        if !ids_to_drop.is_empty() {
            diesel::delete(search_history::table.filter(search_history::id.eq_any(&ids_to_drop)))
                .execute(connection)?;
        }

        return Ok::<(), DieselError>(());
    })?;

    return Ok(());
}

/// Return the most recently searched for phrase which starts with a partial phrase.
pub fn suggest(
    db_conn_pool: &DbConnPool,
    partial: &str,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    let suggestion: Option<String> = search_history::table
        .select(search_history::phrase)
        .filter(search_history::phrase.like(like_prefix_pattern(partial)))
        .order(search_history::last_searched.desc())
        .first::<String>(&mut connection)
        .optional()?;

    return Ok(suggestion);
}

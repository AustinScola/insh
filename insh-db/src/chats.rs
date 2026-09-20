//! The chats which have been had with an AI inference engine.

use std::error::Error;

use crate::helpers::{like_contains_pattern, like_prefix_pattern};
use crate::schema::{chat_messages, chats};
use crate::DbConnPool;

use diesel::dsl::{exists, now};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use pgvector::{Vector, VectorExpressionMethods};

/// A chat.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Chat {
    /// Which chat it is.
    pub id: i64,
    /// What the chat is called.
    pub title: String,
    /// The directory the chat was started in.
    pub dir: String,
}

/// A message of a chat.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Message {
    /// Which message it is.
    pub id: i64,
    /// Who sent the message.
    pub role: String,
    /// What the message says.
    pub content: String,
}

/// A message which a search found, along with the chat it is in.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Hit {
    /// The chat the message is in.
    pub chat: Chat,
    /// Which message it is.
    pub message_id: i64,
    /// What the message says.
    pub content: String,
}

/// A message which has no vector yet.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Unembedded {
    /// Which message it is.
    pub id: i64,
    /// What the message says.
    pub content: String,
}

/// Start a chat and return which one it is.
pub fn create(
    db_conn_pool: &DbConnPool,
    title: &str,
    dir: &str,
) -> Result<i64, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    let id: i64 = diesel::insert_into(chats::table)
        .values((chats::title.eq(title), chats::dir.eq(dir)))
        .returning(chats::id)
        .get_result(&mut connection)?;

    return Ok(id);
}

/// Return the chats, the ones used most recently first.
pub fn list(
    db_conn_pool: &DbConnPool,
    limit: i64,
) -> Result<Vec<Chat>, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    let rows: Vec<(i64, String, String)> = chats::table
        .select((chats::id, chats::title, chats::dir))
        .order(chats::updated.desc())
        .limit(limit)
        .load(&mut connection)?;

    return Ok(rows.into_iter().map(Chat::from).collect());
}

/// Return the chats which are called something starting with some text, or which have a message
/// containing it.
pub fn search_keyword(
    db_conn_pool: &DbConnPool,
    text: &str,
    limit: i64,
) -> Result<Vec<Chat>, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    let rows: Vec<(i64, String, String)> = chats::table
        .select((chats::id, chats::title, chats::dir))
        .filter(
            chats::title.ilike(like_prefix_pattern(text)).or(exists(
                chat_messages::table
                    .filter(chat_messages::chat_id.eq(chats::id))
                    .filter(chat_messages::content.ilike(like_contains_pattern(text))),
            )),
        )
        .order(chats::updated.desc())
        .limit(limit)
        .load(&mut connection)?;

    return Ok(rows.into_iter().map(Chat::from).collect());
}

/// Return the messages which mean the most nearly the same thing as a vector.
///
/// Only the messages whose vector was made by the model which is in use are considered. Vectors
/// made by a different model do not sit in the same space, so how far apart they are means nothing.
///
/// Messages further away than `max_distance` are left out rather than returned at the bottom.
pub fn search_semantic(
    db_conn_pool: &DbConnPool,
    embedding: &[f32],
    embedding_model: &str,
    max_distance: f64,
    limit: i64,
) -> Result<Vec<Hit>, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;
    let vector: Vector = Vector::from(embedding.to_vec());

    let rows: Vec<(i64, String, String, i64, String)> = chat_messages::table
        .inner_join(chats::table)
        .select((
            chats::id,
            chats::title,
            chats::dir,
            chat_messages::id,
            chat_messages::content,
        ))
        .filter(chat_messages::embedding_model.eq(embedding_model))
        // Without this every message comes back, ordered, so a search for something nothing was
        // ever said about still answers with whatever was least unlike it.
        .filter(
            chat_messages::embedding
                .cosine_distance(vector.clone())
                .lt(max_distance),
        )
        .order(chat_messages::embedding.cosine_distance(vector))
        .limit(limit)
        .load(&mut connection)?;

    return Ok(rows
        .into_iter()
        .map(|(id, title, dir, message_id, content)| Hit {
            chat: Chat { id, title, dir },
            message_id,
            content,
        })
        .collect());
}

/// Return the messages of a chat, the oldest first.
pub fn messages(
    db_conn_pool: &DbConnPool,
    chat_id: i64,
) -> Result<Vec<Message>, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    let rows: Vec<(i64, String, String)> = chat_messages::table
        .select((
            chat_messages::id,
            chat_messages::role,
            chat_messages::content,
        ))
        .filter(chat_messages::chat_id.eq(chat_id))
        .order(chat_messages::id.asc())
        .load(&mut connection)?;

    return Ok(rows
        .into_iter()
        .map(|(id, role, content)| Message { id, role, content })
        .collect());
}

/// Add a message to a chat and return which message it is.
///
/// The chat is moved to the front of the history at the same time, so that the list of chats is
/// ordered by when each was last said something in.
pub fn add_message(
    db_conn_pool: &DbConnPool,
    chat_id: i64,
    role: &str,
    content: &str,
) -> Result<i64, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    let id: i64 = connection.transaction(|connection| {
        let id: i64 = diesel::insert_into(chat_messages::table)
            .values((
                chat_messages::chat_id.eq(chat_id),
                chat_messages::role.eq(role),
                chat_messages::content.eq(content),
            ))
            .returning(chat_messages::id)
            .get_result(connection)?;

        diesel::update(chats::table.filter(chats::id.eq(chat_id)))
            .set(chats::updated.eq(now))
            .execute(connection)?;

        return Ok::<i64, DieselError>(id);
    })?;

    return Ok(id);
}

/// Record the vector of a message and the model which made it.
pub fn set_embedding(
    db_conn_pool: &DbConnPool,
    message_id: i64,
    embedding: &[f32],
    embedding_model: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;
    let vector: Vector = Vector::from(embedding.to_vec());

    diesel::update(chat_messages::table.filter(chat_messages::id.eq(message_id)))
        .set((
            chat_messages::embedding.eq(Some(vector)),
            chat_messages::embedding_model.eq(Some(embedding_model)),
        ))
        .execute(&mut connection)?;

    return Ok(());
}

/// Return the messages which have no vector made by the model which is in use.
pub fn unembedded(
    db_conn_pool: &DbConnPool,
    embedding_model: &str,
    limit: i64,
) -> Result<Vec<Unembedded>, Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    let rows: Vec<(i64, String)> = chat_messages::table
        .select((chat_messages::id, chat_messages::content))
        .filter(
            chat_messages::embedding
                .is_null()
                .or(chat_messages::embedding_model.is_distinct_from(Some(embedding_model))),
        )
        .order(chat_messages::id.asc())
        .limit(limit)
        .load(&mut connection)?;

    return Ok(rows
        .into_iter()
        .map(|(id, content)| Unembedded { id, content })
        .collect());
}

/// Change what a chat is called.
pub fn rename(
    db_conn_pool: &DbConnPool,
    chat_id: i64,
    title: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    diesel::update(chats::table.filter(chats::id.eq(chat_id)))
        .set(chats::title.eq(title))
        .execute(&mut connection)?;

    return Ok(());
}

/// Delete a chat and every message in it.
pub fn delete(db_conn_pool: &DbConnPool, chat_id: i64) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut connection = db_conn_pool.get()?;

    // The messages go with it because of the foreign key.
    diesel::delete(chats::table.filter(chats::id.eq(chat_id))).execute(&mut connection)?;

    return Ok(());
}

impl From<(i64, String, String)> for Chat {
    fn from((id, title, dir): (i64, String, String)) -> Self {
        return Self { id, title, dir };
    }
}

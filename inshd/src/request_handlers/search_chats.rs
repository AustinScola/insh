//! Handles requests to search the chats.
use std::sync::Arc;

use embedder::{Embedder, MODEL_ID};
use insh_api::{
    Chat, ChatHit, ChatSearchMode, ResponseParams, ResponseParamsAndLast, SearchChatsRequestParams,
    SearchChatsResponseParams,
};
use insh_db::{chats, DbConnPool};

/// Handles a request to search the chats.
pub struct SearchChats {
    /// What to search for.
    text: String,
    /// How to search.
    mode: ChatSearchMode,
    /// The most chats to return.
    limit: usize,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// Turns text into vectors.
    embedder: Arc<Embedder>,
    /// How far apart the vectors of two texts can be before they stop being a match.
    max_distance: f64,
    /// If searching is done.
    done: bool,
}

impl SearchChats {
    /// Return a new handler for searching the chats.
    pub fn new(
        params: &SearchChatsRequestParams,
        db_conn_pool: DbConnPool,
        embedder: Arc<Embedder>,
        max_distance: f64,
    ) -> Self {
        Self {
            text: params.text().to_string(),
            mode: params.mode(),
            limit: params.limit(),
            db_conn_pool,
            embedder,
            max_distance,
            done: false,
        }
    }

    /// Return the chats which use the words which were searched for.
    fn by_keyword(&self) -> Vec<Chat> {
        let found = match chats::search_keyword(&self.db_conn_pool, &self.text, self.limit as i64) {
            Ok(found) => found,
            Err(error) => {
                log::error!("Failed to search the chats: {}", error);
                return Vec::new();
            }
        };

        return found
            .into_iter()
            .map(|chat| {
                Chat::builder()
                    .id(chat.id)
                    .title(chat.title)
                    .dir(chat.dir)
                    .build()
            })
            .collect();
    }

    /// Return the messages which mean what was searched for.
    fn by_meaning(&self) -> Vec<ChatHit> {
        let embedding: Vec<f32> = self.embedder.embed_one(&self.text);

        let found = match chats::search_semantic(
            &self.db_conn_pool,
            &embedding,
            MODEL_ID,
            self.max_distance,
            self.limit as i64,
        ) {
            Ok(found) => found,
            Err(error) => {
                log::error!("Failed to search the chats by meaning: {}", error);
                return Vec::new();
            }
        };

        return found
            .into_iter()
            .map(|hit| {
                ChatHit::builder()
                    .chat(
                        Chat::builder()
                            .id(hit.chat.id)
                            .title(hit.chat.title)
                            .dir(hit.chat.dir)
                            .build(),
                    )
                    .content(hit.content)
                    .build()
            })
            .collect();
    }
}

impl Iterator for SearchChats {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }
        self.done = true;

        let keyword: bool = matches!(self.mode, ChatSearchMode::Keyword | ChatSearchMode::Both);
        let semantic: bool = matches!(self.mode, ChatSearchMode::Semantic | ChatSearchMode::Both);

        let chats: Vec<Chat> = if keyword {
            self.by_keyword()
        } else {
            Vec::new()
        };
        let hits: Vec<ChatHit> = if semantic {
            self.by_meaning()
        } else {
            Vec::new()
        };

        return Some(
            ResponseParamsAndLast::builder()
                .response_params(ResponseParams::SearchChats(
                    SearchChatsResponseParams::builder()
                        .chats(chats)
                        .hits(hits)
                        .build(),
                ))
                .last(true)
                .build(),
        );
    }
}

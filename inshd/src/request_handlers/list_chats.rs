//! Handles requests to list the chats.
use insh_api::{
    Chat, ListChatsRequestParams, ListChatsResponseParams, ResponseParams, ResponseParamsAndLast,
};
use insh_db::{chats, DbConnPool};

/// Handles a request to list the chats.
pub struct ListChats {
    /// The most chats to list.
    limit: usize,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// If listing the chats is done.
    done: bool,
}

impl ListChats {
    /// Return a new handler for listing the chats.
    pub fn new(params: &ListChatsRequestParams, db_conn_pool: DbConnPool) -> Self {
        Self {
            limit: params.limit(),
            db_conn_pool,
            done: false,
        }
    }
}

impl Iterator for ListChats {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }
        self.done = true;

        let chats: Vec<Chat> = match chats::list(&self.db_conn_pool, self.limit as i64) {
            Ok(chats) => chats
                .into_iter()
                .map(|chat| {
                    Chat::builder()
                        .id(chat.id)
                        .title(chat.title)
                        .dir(chat.dir)
                        .build()
                })
                .collect(),
            Err(error) => {
                log::error!("Failed to list the chats: {}", error);
                Vec::new()
            }
        };

        return Some(
            ResponseParamsAndLast::builder()
                .response_params(ResponseParams::ListChats(
                    ListChatsResponseParams::builder().chats(chats).build(),
                ))
                .last(true)
                .build(),
        );
    }
}

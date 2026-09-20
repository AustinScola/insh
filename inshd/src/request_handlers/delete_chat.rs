//! Handles requests to delete a chat.
use insh_api::{
    DeleteChatRequestParams, DeleteChatResponseParams, ResponseParams, ResponseParamsAndLast,
};
use insh_db::{chats, DbConnPool};

/// Handles a request to delete a chat.
pub struct DeleteChat {
    /// Which chat to delete.
    chat_id: i64,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// If deleting the chat is done.
    done: bool,
}

impl DeleteChat {
    /// Return a new handler for deleting a chat.
    pub fn new(params: &DeleteChatRequestParams, db_conn_pool: DbConnPool) -> Self {
        Self {
            chat_id: params.chat_id(),
            db_conn_pool,
            done: false,
        }
    }
}

impl Iterator for DeleteChat {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }
        self.done = true;

        let deleted: bool = match chats::delete(&self.db_conn_pool, self.chat_id) {
            Ok(()) => true,
            Err(error) => {
                log::error!("Failed to delete a chat: {}", error);
                false
            }
        };

        return Some(
            ResponseParamsAndLast::builder()
                .response_params(ResponseParams::DeleteChat(
                    DeleteChatResponseParams::builder().deleted(deleted).build(),
                ))
                .last(true)
                .build(),
        );
    }
}

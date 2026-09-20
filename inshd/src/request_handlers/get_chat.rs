//! Handles requests to get the messages of a chat.
use insh_api::{
    ChatMessage, ChatRole, GetChatRequestParams, GetChatResponseParams, ResponseParams,
    ResponseParamsAndLast,
};
use insh_db::{chats, DbConnPool};

/// The role recorded for what the inference engine says.
const ASSISTANT: &str = "assistant";

/// Handles a request to get the messages of a chat.
pub struct GetChat {
    /// Which chat to get the messages of.
    chat_id: i64,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// If getting the messages is done.
    done: bool,
}

impl GetChat {
    /// Return a new handler for getting the messages of a chat.
    pub fn new(params: &GetChatRequestParams, db_conn_pool: DbConnPool) -> Self {
        Self {
            chat_id: params.chat_id(),
            db_conn_pool,
            done: false,
        }
    }
}

impl Iterator for GetChat {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }
        self.done = true;

        let mut error: Option<String> = None;
        let messages: Vec<ChatMessage> = match chats::messages(&self.db_conn_pool, self.chat_id) {
            Ok(messages) => messages
                .into_iter()
                .map(|message| {
                    let role: ChatRole = match message.role.as_str() {
                        ASSISTANT => ChatRole::Assistant,
                        _ => ChatRole::User,
                    };
                    ChatMessage::builder()
                        .role(role)
                        .content(message.content)
                        .build()
                })
                .collect(),
            Err(failure) => {
                log::error!("Failed to get the messages of a chat: {}", failure);
                // Answering with no messages on its own would show as a chat with nothing said in
                // it, which is not what happened.
                error = Some(format!("Failed to read the chat: {}", failure));
                Vec::new()
            }
        };

        return Some(
            ResponseParamsAndLast::builder()
                .response_params(ResponseParams::GetChat(
                    GetChatResponseParams::builder()
                        .messages(messages)
                        .error(error)
                        .build(),
                ))
                .last(true)
                .build(),
        );
    }
}

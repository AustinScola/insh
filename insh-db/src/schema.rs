//! The database schema.
//!
//! This is written by hand rather than generated with `diesel print-schema` so that building inshd
//! never requires a running database. It must be kept in sync with the migrations.
#![allow(missing_docs, clippy::missing_docs_in_private_items)]

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::Vector;

    chat_messages (id) {
        id -> Int8,
        chat_id -> Int8,
        role -> Text,
        content -> Text,
        created -> Timestamptz,
        embedding -> Nullable<Vector>,
        embedding_model -> Nullable<Text>,
    }
}

diesel::table! {
    chats (id) {
        id -> Int8,
        title -> Text,
        dir -> Text,
        created -> Timestamptz,
        updated -> Timestamptz,
    }
}

diesel::joinable!(chat_messages -> chats (chat_id));
diesel::allow_tables_to_appear_in_same_query!(chat_messages, chats);

diesel::table! {
    dir_history (id) {
        id -> Int8,
        path -> Text,
        last_visited -> Timestamptz,
    }
}

diesel::table! {
    find_history (id) {
        id -> Int8,
        pattern -> Text,
        last_found -> Timestamptz,
    }
}

diesel::table! {
    search_history (id) {
        id -> Int8,
        phrase -> Text,
        last_searched -> Timestamptz,
    }
}

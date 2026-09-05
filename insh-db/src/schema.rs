//! The database schema.
//!
//! This is written by hand rather than generated with `diesel print-schema` so that building inshd
//! never requires a running database. It must be kept in sync with the migrations.
#![allow(missing_docs, clippy::missing_docs_in_private_items)]

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

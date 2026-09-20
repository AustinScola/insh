//! The past chats.

mod history;
pub use history::{Chats, Effect as HistoryEffect, Entry, History, Movement};

mod searcher;
pub use searcher::{Effect as HistorySearcherEffect, Searcher as HistorySearcher};

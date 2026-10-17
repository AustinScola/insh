//! The components.

mod browser;
mod chat;
mod common;
mod file_creator;
mod finder;
mod history;
mod insh;
mod searcher;
mod session;

pub use chat::Chat;
pub use insh::{Insh, Props as InshProps, Start as InshStart};
pub use session::Session;

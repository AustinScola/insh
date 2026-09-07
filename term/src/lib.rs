/*!
Raw terminal control, and the events which the terminal sends.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::needless_return)]

mod event;
mod term;

pub use crate::event::{Key, KeyEvent, KeyMods, ParsedTermEvent, TermEvent, TermEventParseError};
pub use crate::term::{SavedAttrs, Term};

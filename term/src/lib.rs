#![allow(clippy::enum_variant_names)]
#![allow(clippy::needless_return)]

mod event;
mod term;

pub use crate::event::{Key, KeyEvent, KeyMods, TermEvent};
pub use crate::term::Term;

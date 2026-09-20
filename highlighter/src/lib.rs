/*!
Works out which part of some code is what, so that it can be shown in color.

The code is parsed rather than matched against patterns, which is what tree-sitter is for and what
editors use. Nothing here knows anything about terminals: it says what each piece of the code *is*,
and whoever is drawing decides what that should look like.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_return)]

mod highlighter;
mod language;
mod span;

pub use highlighter::Highlighter;
pub use language::Language;
pub use span::{Kind, Span};

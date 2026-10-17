/*!
The screen which the bytes a program writes mean to show.

A program running on a pseudoterminal writes bytes, and what it means by them is a screen: which
characters are in which columns of which rows, how each of them is written, and where the cursor
is. Working that out is what this does.

There are two parts to it. [`Parser`] turns the bytes into [`Action`]s — a character to write, one
of the controls below the printable characters, or a control function — holding on to whatever
arrives split across two lots of bytes until the rest of it comes. [`Screen`] is what the actions
are done to, and is what can then be drawn.

Nothing here reads from or writes to a pseudoterminal, and nothing here draws on a terminal. This
only says what the screen of a program is; putting it there is somebody else's job.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::module_inception)]
#![allow(clippy::needless_return)]

mod action;
mod character_set;
mod cursor;
mod grid;
mod modes;
mod parser;
mod screen;

pub use action::Action;
pub use character_set::CharacterSet;
pub use cursor::Cursor;
pub use grid::{Grid, Line};
pub use modes::Modes;
pub use parser::Parser;
pub use screen::{Props as ScreenProps, Screen};

/*!
Contains functionality for representing styled text, manipulating it, and rendering it to a
terminal screen.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::module_inception)]
#![allow(clippy::needless_return)]

mod cell;
mod fabric;
mod location;
mod renderer;
mod row;
mod spot;
mod style;
mod yarn;

pub use cell::Cell;
pub use fabric::Fabric;
pub use location::Location;
pub use renderer::{Engine, Frame, Instruction, Renderer, Text};
pub use row::Row;
pub use size::Size;
pub use spot::Spot;
pub use style::Style;
pub use yarn::Yarn;

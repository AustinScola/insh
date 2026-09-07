/*!
Contains functionality for representing styled text, manipulating it, and rendering it to a
terminal screen.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_return)]

mod cell;
mod fabric;
mod location;
mod renderer;
mod yarn;

pub use cell::Cell;
pub use fabric::Fabric;
pub use location::Location;
pub use renderer::Renderer;
pub use size::Size;
pub use yarn::Yarn;

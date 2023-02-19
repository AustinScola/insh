/*!
Contains functionality for representing styled text, manipulating it, and rendering it to a
terminal screen.
*/
mod renderer;
pub use renderer::Renderer;

mod yarn;
pub use yarn::Yarn;

mod fabric;
pub use fabric::Fabric;

mod location;
pub use location::Location;

mod size;
pub use size::Size;

pub mod renderables;
pub use renderables::{JoinAndWrap, VerticallyCentered};

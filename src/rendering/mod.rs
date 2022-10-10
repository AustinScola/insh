/*!
Contains functionality for representing styled text, manipulating it, and rendering it to a
terminal screen.
*/
mod fabric;
mod location;
mod renderer;
mod size;
mod yarn;

pub use fabric::Fabric;
pub use location::Location;
pub use renderer::Renderer;
pub use size::Size;
pub use yarn::Yarn;

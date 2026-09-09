//! Drawing a fabric on a terminal.

mod engine;
mod frame;
mod instruction;
mod loom;
mod optimizer;
mod renderer;
#[cfg(test)]
mod terminal;
mod text;

pub use engine::Engine;
pub use frame::Frame;
pub use instruction::Instruction;
pub use renderer::Renderer;
pub use text::Text;

use loom::Loom;
use optimizer::Optimizer;

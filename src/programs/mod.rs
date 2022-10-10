/*!
[`Program`](super::program::Program) that can be run.
*/
mod bash;
mod vim;

pub use bash::Bash;
pub use vim::{Args as VimArgs, ArgsBuilder as VimArgsBuilder, Vim};

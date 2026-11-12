//! Turns the current process into a daemon.
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_return)]

mod daemon;
mod error;

pub use daemon::Daemon;
pub use error::Error;

/*!
The terminal interface library which insh is built on.

An [`App`] owns the terminal and runs a [`Component`] in it.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::single_match)]
#![allow(clippy::manual_map)]

mod app;
mod ascii;
mod command_parser;
mod component;
mod event;
mod output_forwarder;
mod program;
mod program_monitor;
mod requester;
mod response_handler;
mod stopper;
mod system_effect;
mod term_event_forwarder;

pub use app::{App, AppRunOptions};
pub use command_parser::{CommandParser, KeyPattern, Parsed};
pub use component::Component;
pub use event::Event;
pub use program::{EnvVar, Program, ProgramCleanup, ProgramSetup, StdoutPipe};
pub use requester::Requester;
pub use response_handler::ResponseHandler;
pub use stopper::Stopper;
pub use system_effect::SystemEffect;

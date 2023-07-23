#![allow(clippy::single_match)]
#![allow(clippy::manual_map)]

mod app;
mod ascii;
mod component;
mod event;
mod output_forwarder;
mod paths;
mod program;
mod program_monitor;
mod requester;
mod response_handler;
mod stopper;
mod system_effect;
mod term_event_forwarder;

pub use app::{App, AppRunOptions};
pub use component::Component;
pub use event::Event;
pub use program::{EnvVar, Program, ProgramCleanup, ProgramSetup, StdoutPipe};
pub use requester::Requester;
pub use response_handler::ResponseHandler;
pub use stopper::Stopper;
pub use system_effect::SystemEffect;

#[macro_use]
extern crate lazy_static;

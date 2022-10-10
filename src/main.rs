//! A graphical, interactive, terminal environment.
#![deny(warnings)]
#![deny(missing_docs)]
#![allow(clippy::module_inception)]
#![allow(clippy::needless_return)]

#[macro_use]
extern crate lazy_static;

mod app;
mod args;
mod auto_completer;
mod auto_completers;
mod clipboard;
mod color;
mod component;
mod components;
mod config;
mod current_dir;
mod data;
#[cfg(feature = "logging")]
mod logging;
mod path_finder;
mod phrase_searcher;
mod program;
mod programs;
mod rendering;
mod stateful;
mod string;
mod system_effect;

use crate::app::App;
use crate::args::Args;
use crate::component::Component;
use crate::components::{Insh, InshProps};
use crate::config::Config;
#[cfg(feature = "logging")]
use crate::logging::{configure_logging, ConfigureLoggingResult};
use crate::stateful::Stateful;
use crate::system_effect::SystemEffect;

use clap::Parser;
#[cfg(feature = "logging")]
use flexi_logger::LoggerHandle;

use std::process::exit;

fn main() {
    let args: Args = Args::parse();

    #[cfg(feature = "logging")]
    let _logger_handle: LoggerHandle;
    #[cfg(feature = "logging")]
    if let Some(log_file_path) = args.log_file_path() {
        let configure_logging_result: ConfigureLoggingResult =
            configure_logging(log_file_path.to_path_buf(), args.log_specification());
        _logger_handle = match configure_logging_result {
            Ok(_logger_handle) => _logger_handle,
            Err(error) => {
                println!("{}", error);
                exit(1);
            }
        }
    }

    let starting_effects: Option<Vec<SystemEffect>> = args.starting_effects();

    let config: Config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            println!("{}", error);
            exit(1);
        }
    };

    let insh_props: InshProps = InshProps::from((args, config));

    let mut app: App = App::new();
    let mut root = Insh::new(insh_props);

    app.run(&mut root, starting_effects);
}

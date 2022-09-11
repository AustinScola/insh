#![deny(warnings)]
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
use crate::stateful::Stateful;
use crate::system_effect::SystemEffect;

use clap::Parser;

use std::process::exit;

fn main() {
    let args: Args = Args::parse();
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

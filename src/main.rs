#![deny(warnings)]
#![allow(clippy::module_inception)]

mod app;
mod args;
mod clipboard;
mod color;
mod component;
mod components;
mod current_dir;
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
use crate::stateful::Stateful;
use crate::system_effect::SystemEffect;

use clap::Parser;

fn main() {
    let args: Args = Args::parse();
    let starting_effects: Option<Vec<SystemEffect>> = args.starting_effects();

    let insh_props: InshProps = InshProps::from(args);

    let mut app: App = App::new();
    let mut root = Insh::new(insh_props);

    app.run(&mut root, starting_effects);
}

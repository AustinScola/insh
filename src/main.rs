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
mod system_effect;

use app::App;
use args::Args;
use component::Component;
use components::{Insh, InshProps};
use stateful::Stateful;

use clap::Parser;

fn main() {
    let args: Args = Args::parse();
    let insh_props: InshProps = InshProps::from(args);

    let mut app: App = App::new();
    let mut root = Insh::new(insh_props);

    app.run(&mut root);
}

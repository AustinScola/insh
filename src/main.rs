mod action;
mod bash_shell;
mod color;
mod effect;
mod finder;
mod insh;
mod searcher;
mod state;
mod terminal_size;
mod vim;
mod walker;

use crate::state::State;

extern crate clap;
use clap::{App, SubCommand};

fn main() {
    let matches = App::new("insh")
        .version("0.1.0")
        .author("Austin Scola <austinscola@gmail.com>")
        .about("Interactive Shell")
        .subcommand(
            SubCommand::with_name("find")
                .about("Find a file")
                .visible_alias("f"),
        )
        .subcommand(
            SubCommand::with_name("search")
                .about("Search file contents")
                .visible_alias("s"),
        )
        .get_matches();

    let state = match matches.subcommand_name() {
        Some("find") => State::new_find(),
        Some("search") => State::new_search(),
        _ => State::new(),
    };

    let mut insh = insh::Insh::from(state);
    insh.run();
}

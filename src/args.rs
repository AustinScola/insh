use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Starting directory to run in
    #[clap(short, long)]
    directory: Option<PathBuf>,

    #[clap(subcommand)]
    command: Option<Command>,
}

impl Args {
    pub fn directory(&self) -> &Option<PathBuf> {
        &self.directory
    }

    pub fn command(&self) -> &Option<Command> {
        &self.command
    }
}

#[derive(Subcommand, Clone)]
pub enum Command {
    /// Browse a directory
    #[clap(alias = "b", display_order = 1)]
    Browse,

    /// Find files by name
    #[clap(alias = "f", display_order = 2)]
    Find { phrase: Option<String> },

    /// Search file contents
    #[clap(alias = "s", display_order = 3)]
    Search { phrase: Option<String> },
}

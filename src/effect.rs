#![allow(clippy::enum_variant_names)]
use std::path::{Path, PathBuf};

pub enum Effect {
    RunBash(PathBuf),
    RunVim(Box<Path>),
    RunVimAtLine(Box<Path>, usize),
    RunVimWithCommand(Box<Path>, String),
}

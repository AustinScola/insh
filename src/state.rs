use crate::finder::Finder;
use crate::searcher::{SearchFileHit, Searcher};
use crate::terminal_size::TerminalSize;
use std::fs;
use std::path::PathBuf;

pub struct State {
    pub terminal_size: TerminalSize,

    pub mode: Mode,

    pub browse: BrowseState,
    pub find: FindState,
    pub search: SearchState,
}

#[derive(PartialEq)]
pub enum Mode {
    Browse,

    Find,
    BrowseFind,

    Search,
    BrowseSearch,
}

pub enum PatternState {
    NotCompiled,
    BadRegex,
    GoodRegex,
}

pub struct BrowseState {
    pub directory: Box<PathBuf>,
    pub entries: Vec<fs::DirEntry>,
    pub offset: usize,
    pub selected: usize,
}

pub struct FindState {
    pub pattern: String,
    pub pattern_state: PatternState,
    pub found: Vec<fs::DirEntry>,
    pub finder: Option<Finder>,
    pub offset: usize,
    pub selected: usize,
}

pub struct SearchState {
    pub search: String,
    pub hits: Vec<SearchFileHit>,
    pub searcher: Option<Searcher>,
    pub file_offset: usize,
    pub line_offset: Option<usize>,
    pub file_selected: usize,
    pub line_selected: Option<usize>,
}

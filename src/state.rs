use crate::finder::Finder;
use crate::searcher::{SearchFileHit, Searcher};
use crate::terminal_size::TerminalSize;
use std::fs;
use std::path::PathBuf;

pub struct State {
    pub terminal_size: TerminalSize,

    pub mode: Mode,

    // Browse mode state
    pub selected: usize,
    pub directory: Box<PathBuf>,
    pub entries: Vec<fs::DirEntry>,
    pub entry_offset: usize,

    // Find mode state
    pub pattern: String,
    pub pattern_state: PatternState,
    pub found: Vec<fs::DirEntry>,
    pub finder: Option<Finder>,
    pub find_offset: usize,
    pub find_selected: usize,

    // Search mode state
    pub search: String,
    pub hits: Vec<SearchFileHit>,
    pub searcher: Option<Searcher>,
    pub search_file_offset: usize,
    pub search_line_offset: Option<usize>,
    pub search_file_selected: usize,
    pub search_line_selected: Option<usize>,
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

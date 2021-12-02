use crate::finder::Finder;
use crate::searcher::{SearchFileHit, Searcher};
use crate::terminal_size::TerminalSize;
use std::env::current_dir;
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

impl Default for BrowseState {
    fn default() -> Self {
        let directory: Box<PathBuf> = Box::new(current_dir().unwrap());
        let entries: Vec<fs::DirEntry> = Vec::new();
        let offset = 0;
        let selected = 0;

        BrowseState {
            directory,
            entries,
            offset,
            selected,
        }
    }
}

pub struct FindState {
    pub pattern: String,
    pub pattern_state: PatternState,
    pub found: Vec<fs::DirEntry>,
    pub finder: Option<Finder>,
    pub offset: usize,
    pub selected: usize,
}

impl Default for FindState {
    fn default() -> Self {
        let pattern = String::new();
        let pattern_state = PatternState::NotCompiled;
        let found = Vec::new();
        let finder = None;
        let offset = 0;
        let selected = 0;

        FindState {
            pattern,
            pattern_state,
            found,
            finder,
            offset,
            selected,
        }
    }
}

impl FindState {
    pub fn selected_path(&self) -> PathBuf {
        let index = self.offset + self.selected;
        self.found[index].path()
    }
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

impl Default for SearchState {
    fn default() -> Self {
        let search = String::new();
        let hits = Vec::new();
        let searcher = None;
        let file_offset = 0;
        let line_offset = None;
        let file_selected = 0;
        let line_selected = None;

        SearchState {
            search,
            hits,
            searcher,
            file_offset,
            line_offset,
            file_selected,
            line_selected,
        }
    }
}

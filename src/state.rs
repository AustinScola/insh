use crate::action::Action;
use crate::effect::Effect;
use crate::finder::Finder;
use crate::searcher::{SearchFileHit, Searcher};
use crate::terminal_size::TerminalSize;
use crate::walker::Walker;

use std::env::current_dir;
use std::fs;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

pub struct State {
    pub terminal_size: TerminalSize,

    pub mode: Mode,

    pub browse: BrowseState,
    pub find: FindState,
    pub search: SearchState,
}

impl Default for State {
    fn default() -> Self {
        State {
            terminal_size: Default::default(),
            mode: Default::default(),
            browse: Default::default(),
            find: Default::default(),
            search: Default::default(),
        }
    }
}

impl State {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn new_find() -> Self {
        let mut state: State = Default::default();
        state.enter_find_mode();
        state
    }

    pub fn new_search() -> Self {
        let mut state: State = Default::default();
        state.enter_search_mode();
        state
    }

    pub fn search_selected_line(&self) -> usize {
        if self.search.file_offset >= self.search.hits.len() {
            return 0;
        }

        let mut selected_line = 0;

        let mut file_offset = self.search.file_offset;
        if self.search.file_selected == 0 {
            if let Some(search_line_selected) = self.search.line_selected {
                selected_line += search_line_selected + 1;
            }

            return selected_line;
        } else {
            selected_line += self.search.hits[file_offset].hits.len() + 1;
        }

        if let Some(search_line_offset) = self.search.line_offset {
            selected_line -= search_line_offset + 1;
        }

        if selected_line >= (self.terminal_size.height - 2).into() {
            return (self.terminal_size.height - 2).into();
        }

        file_offset += 1;
        selected_line += 1;

        loop {
            if file_offset >= self.search.file_offset + self.search.file_selected {
                break;
            }

            selected_line += self.search.hits[file_offset].hits.len() + 2;
            file_offset += 1;
        }

        if selected_line >= (self.terminal_size.height - 2).into() {
            return (self.terminal_size.height - 2).into();
        }

        if let Some(search_line_selected) = self.search.line_selected {
            selected_line += search_line_selected + 1;
        }

        if selected_line >= (self.terminal_size.height - 2).into() {
            return (self.terminal_size.height - 2).into();
        }

        selected_line
    }

    pub fn perform(&mut self, action: &Action) -> Option<Effect> {
        let mut effect: Option<Effect> = None;
        match action {
            Action::Exit => {}
            Action::RunBash => {
                effect = Some(self.run_bash());
            }

            // Browse actions.
            Action::EnterBrowseMode => {
                self.enter_browse_mode();
            }
            Action::BrowseScrollDown => {
                self.browse_scroll_down();
            }
            Action::BrowseScrollUp => {
                self.browse.scroll_up();
            }
            Action::BrowseGoToBottom => {
                self.browse_go_to_bottom();
            }
            Action::BrowseGoToTop => {
                self.browse.go_to_top();
            }
            Action::BrowseDrillDown => {
                effect = self.browse.drill_down();
            }
            Action::BrowseDrillUp => {
                self.browse.pop_directory();
            }
            Action::BrowseEdit => {
                effect = self.browse.edit_selected_file();
            }
            Action::BrowseRefresh => {
                self.browse.refresh();
            }

            // Find actions.
            Action::EnterFindMode => {
                self.enter_find_mode();
            }
            Action::FindAppendCharacter(character) => {
                self.find.append_character(character);
            }
            Action::FindDeletePreviousCharacter => {
                self.find.delete_previous_character();
            }

            // Browse find actions.
            Action::EnterBrowseFindMode => {
                self.enter_browse_find_mode();
            }
            Action::FindScrollDown => {
                self.find_scroll_down();
            }
            Action::FindScrollUp => {
                self.find.scroll_up();
            }
            Action::FindBrowseSelectedParent => {
                self.find_browse_selected_parent();
            }
            Action::FindEditFile => {
                effect = Some(self.find.edit_file());
            }

            // Search actions.
            Action::EnterSearchMode => {
                self.enter_search_mode();
            }
            Action::SearchDeletePreviousCharacter => {
                self.search.delete_previous_character();
            }
            Action::SearchAppendCharacter(character) => {
                self.search.append_character(character);
            }

            // Browse search actions.
            Action::EnterBrowseSearchMode => {
                self.enter_browse_search_mode();
            }
            Action::SearchScrollDown => {
                self.search_scroll_down();
            }
            Action::SearchScrollUp => {
                self.search_scroll_up();
            }
            Action::SearchEditFile => {
                effect = Some(self.search.edit_file());
            }
        }

        effect
    }

    fn enter_browse_mode(&mut self) {
        self.mode = Mode::Browse;
    }

    fn enter_find_mode(&mut self) {
        self.mode = Mode::Find;

        self.find.pattern.clear();
        self.find.pattern_state = PatternState::NotCompiled;

        let mut entries = Walker::from(&(*self.browse.directory.as_path()));
        self.find.found.clear();
        let mut there_are_more = true;
        for _ in 0..(self.terminal_size.height - 1).into() {
            let entry = entries.next();
            match entry {
                Some(entry) => self.find.found.push(entry),
                None => {
                    there_are_more = false;
                    break;
                }
            }
        }

        if there_are_more {
            self.find.finder = Some(Finder::from(entries));
        } else {
            self.find.finder = None;
        }

        self.find.offset = 0;
        self.find.selected = 0;
    }

    fn enter_browse_find_mode(&mut self) {
        self.mode = Mode::BrowseFind;

        self.find.found.clear();
        self.find.finder = None;

        let entries = Finder::new(&*self.browse.directory, &self.find.pattern);
        match entries {
            Err(_) => {
                self.find.pattern_state = PatternState::BadRegex;
            }
            Ok(mut entries) => {
                self.find.pattern_state = PatternState::GoodRegex;
                for _ in 0..(self.terminal_size.height - 1) {
                    let entry = entries.next();
                    match entry {
                        Some(entry) => self.find.found.push(entry),
                        None => break,
                    }
                }
                self.find.finder = Some(entries);
            }
        }
        self.find.selected = 0;
    }

    fn enter_search_mode(&mut self) {
        self.mode = Mode::Search;
        self.search.hits.clear();
        self.search.search.clear();

        self.search.file_offset = 0;
        self.search.line_offset = None;
        self.search.file_selected = 0;
        self.search.line_selected = None;
    }

    fn enter_browse_search_mode(&mut self) {
        self.mode = Mode::BrowseSearch;

        let mut searcher = Searcher::new(&*self.browse.directory, &self.search.search);

        self.search.hits.clear();
        let mut lines: usize = 0;
        while lines < (self.terminal_size.height - 1).into() {
            let hit = searcher.next();
            match hit {
                Some(search_file_hit) => {
                    lines += 1 + search_file_hit.hits.len();
                    self.search.hits.push(search_file_hit);
                }
                None => break,
            }
        }

        self.search.searcher = Some(searcher);
    }

    fn run_bash(&mut self) -> Effect {
        let directory = &self.browse.directory;
        Effect::RunBash(directory.to_path_buf())
    }

    fn browse_scroll_down(&mut self) {
        if self.browse.offset + self.browse.selected < self.browse.entries.len() - 1 {
            if self.browse.selected < self.terminal_size.height as usize - 1 {
                self.browse.selected += 1;
            } else {
                self.browse.offset += 1;
            }
        }
    }

    fn browse_go_to_bottom(&mut self) {
        if self.browse.entries.len() <= self.terminal_size.height as usize {
            self.browse.selected = self.browse.entries.len() - 1;
        } else {
            self.browse.offset = self.browse.entries.len() - self.terminal_size.height as usize;
            self.browse.selected = self.terminal_size.height as usize - 1;
        }
    }

    fn find_scroll_down(&mut self) {
        if !self.find.found_files() {
            return;
        }

        if self.find.selected < self.terminal_size.height as usize - 2 {
            if self.find.selected + self.find.offset < self.find.found.len() - 1 {
                self.find.selected += 1;
            }
        } else if self.find.selected + self.find.offset < self.find.found.len() - 1 {
            self.find.offset += 1;
        } else if let Some(ref mut finder) = self.find.finder {
            match finder.next() {
                Some(entry) => {
                    self.find.found.push(entry);
                    self.find.offset += 1;
                }
                None => {
                    self.find.finder = None;
                }
            }
        }
    }

    fn find_browse_selected_parent(&mut self) {
        let directory = self.find.selected_path_parent().to_path_buf();
        self.browse.change_directory(directory);
        self.enter_browse_mode();
    }

    pub fn increment_search_line_selected(&mut self) {
        match self.search.line_selected {
            Some(search_line_selected) => {
                if search_line_selected < (self.terminal_size.height - 2).into() {
                    self.search.line_selected = Some(search_line_selected + 1);
                }
            }
            None => {
                self.search.line_selected = Some(0);
            }
        };
    }

    fn search_scroll_down(&mut self) {
        if self.search.found_hits() {
            let first_file = self.search.hits[self.search.file_offset].clone();
            let selected_line = self.search_selected_line();
            let search_file_number = self.search.file_offset + self.search.file_selected;
            let search_file_hit = self.search.hits[search_file_number].clone();

            let search_line_number = self.search.line_number();

            if selected_line == (self.terminal_size.height - 2).into() {
                if search_line_number >= Some(search_file_hit.hits.len() - 1) {
                    if search_file_number == self.search.hits.len() - 1 {
                        if let Some(ref mut searcher) = self.search.searcher {
                            match searcher.next() {
                                Some(hit) => {
                                    self.search.hits.push(hit);

                                    self.search.line_selected = None;
                                    self.search.file_selected += 1;

                                    if self.search.line_offset == None {
                                        self.search.line_offset = Some(0);
                                    } else if self.search.line_offset < Some(first_file.hits.len())
                                    {
                                        self.search.increment_line_offset()
                                    } else {
                                        self.search.file_offset += 1;
                                        self.search.line_offset = None;
                                        self.search.file_selected -= 1;
                                    }

                                    let first_file =
                                        self.search.hits[self.search.file_offset].clone();

                                    if self.search.line_offset == None {
                                        self.search.line_offset = Some(0);
                                    } else if self.search.line_offset < Some(first_file.hits.len())
                                    {
                                        self.search.increment_line_offset()
                                    } else {
                                        self.search.file_offset += 1;
                                        self.search.line_offset = None;
                                        self.search.file_selected -= 1;
                                    }
                                }
                                None => {
                                    self.search.searcher = None;
                                }
                            }
                        }
                    } else {
                        self.search.line_selected = None;
                        self.search.file_selected += 1;

                        if self.search.line_offset == None {
                            self.search.line_offset = Some(0);
                        } else if self.search.line_offset < Some(first_file.hits.len()) {
                            self.search.increment_line_offset()
                        } else {
                            self.search.file_offset += 1;
                            self.search.line_offset = None;
                            self.search.file_selected -= 1;
                        }

                        let first_file = self.search.hits[self.search.file_offset].clone();

                        if self.search.line_offset == None {
                            self.search.line_offset = Some(0);
                        } else if self.search.line_offset < Some(first_file.hits.len()) {
                            self.search.increment_line_offset()
                        } else {
                            self.search.file_offset += 1;
                            self.search.line_offset = None;
                            self.search.file_selected -= 1;
                        }
                    }
                } else if self.search.line_offset == None {
                    if self.search.file_selected != 0 {
                        self.increment_search_line_selected();
                    }
                    self.search.line_offset = Some(0);
                } else if self.search.line_offset < Some(first_file.hits.len()) {
                    if self.search.file_selected != 0 {
                        self.increment_search_line_selected();
                    }
                    self.search.increment_line_offset()
                } else {
                    self.increment_search_line_selected();
                    self.search.file_offset += 1;
                    self.search.line_offset = None;
                    self.search.file_selected -= 1;
                }
            } else if selected_line == (self.terminal_size.height - 3).into()
                && search_line_number >= Some(search_file_hit.hits.len() - 1)
            {
                self.search.line_selected = None;
                self.search.file_selected += 1;

                if self.search.line_offset == None {
                    self.search.line_offset = Some(0);
                } else if self.search.line_offset < Some(first_file.hits.len()) {
                    self.search.increment_line_offset()
                } else {
                    self.search.file_offset += 1;
                    self.search.line_offset = None;
                    self.search.file_selected -= 1;
                }
            } else if search_line_number >= Some(search_file_hit.hits.len() - 1) {
                if search_file_number < self.search.hits.len() - 1 {
                    self.search.file_selected += 1;
                    self.search.line_selected = None;
                }
            } else {
                self.increment_search_line_selected();
            }
        }
    }

    fn search_scroll_up(&mut self) {
        // Determine the line on the screen that is selected.
        let selected_line = self.search_selected_line();

        if selected_line == 0 {
            match self.search.line_offset {
                None => {
                    if self.search.file_offset != 0 {
                        self.search.file_offset -= 1;
                        self.search.line_offset =
                            Some(self.search.hits[self.search.file_offset].hits.len() - 1);
                    }
                }
                Some(0) => {
                    self.search.line_offset = None;
                }
                Some(search_line_offset) => {
                    self.search.line_offset = Some(search_line_offset - 1);
                }
            }
        } else if self.search.line_selected.is_none() {
            if self.search.file_selected == 0 {
                if self.search.file_offset > 0 {
                    self.search.file_offset -= 1;

                    let search_file_number = self.search.file_offset + self.search.file_selected;
                    let search_file_hit = self.search.hits[search_file_number].clone();
                    self.search.line_selected = Some(search_file_hit.hits.len() - 1);
                }
            } else {
                self.search.file_selected -= 1;
                if selected_line == 1 {
                    self.search.line_selected = None;
                    let search_file_number = self.search.file_offset + self.search.file_selected;
                    let search_file_hit = self.search.hits[search_file_number].clone();
                    self.search.line_offset = Some(search_file_hit.hits.len() - 1);
                } else {
                    let search_file_number = self.search.file_offset + self.search.file_selected;
                    let search_file_hit = self.search.hits[search_file_number].clone();
                    if self.search.file_selected == 0 {
                        self.search.line_selected = match self.search.line_offset {
                            Some(search_line_offset) => {
                                if search_line_offset == search_file_hit.hits.len() - 1 {
                                    None
                                } else {
                                    Some(search_file_hit.hits.len() - search_line_offset - 2)
                                }
                            }
                            None => Some(search_file_hit.hits.len() - 1),
                        }
                    } else {
                        self.search.line_selected = Some(search_file_hit.hits.len() - 1);
                    }
                }
            }
        } else {
            self.search.decrement_line_selected();
        }
    }
}

#[derive(PartialEq)]
pub enum Mode {
    Browse,

    Find,
    BrowseFind,

    Search,
    BrowseSearch,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Browse
    }
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

        let mut browse_state = BrowseState {
            directory,
            entries,
            offset,
            selected,
        };

        browse_state.get_entries();

        browse_state
    }
}

impl BrowseState {
    fn selected_path(&self) -> PathBuf {
        self.entries[self.selected + self.offset].path()
    }

    fn scroll_up(&mut self) {
        if self.selected == 0 {
            if self.offset > 0 {
                self.offset -= 1;
            }
        } else {
            self.selected -= 1;
        }
    }
    fn go_to_top(&mut self) {
        self.offset = 0;
        self.selected = 0;
    }

    fn drill_down(&mut self) -> Option<Effect> {
        if self.entries.is_empty() {
            return None;
        }

        let selected_path: PathBuf = self.selected_path();
        if selected_path.is_dir() {
            self.push_directory();
        } else if selected_path.is_file() {
            return Some(Effect::RunVim(Box::from(selected_path.as_path())));
        }
        None
    }

    fn push_directory(&mut self) {
        self.directory.push(self.selected_path());
        self.selected = 0;
        self.offset = 0;
        self.get_entries();
    }

    fn pop_directory(&mut self) {
        self.directory.pop();
        self.selected = 0;
        self.offset = 0;
        self.get_entries();
    }

    fn change_directory(&mut self, directory: PathBuf) {
        self.directory = Box::new(directory);
        self.offset = 0;
        self.selected = 0;
        self.get_entries();
    }

    fn refresh(&mut self) {
        self.offset = 0;
        self.selected = 0;
        self.get_entries();
    }

    fn get_entries(&mut self) {
        let entries_iter = fs::read_dir(self.directory.as_path()).unwrap();
        let mut entries = Vec::from_iter(entries_iter.map(|entry| entry.unwrap()));
        entries.sort_unstable_by_key(|a| a.file_name());
        self.entries = entries;
    }

    pub fn edit_selected_file(&mut self) -> Option<Effect> {
        let selected_path = self.entries[self.selected].path();
        let selected_path = selected_path.as_path();
        if selected_path.is_file() {
            return Some(Effect::RunVim(Box::from(selected_path)));
        }
        None
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
    pub fn found_files(&self) -> bool {
        !self.found.is_empty()
    }

    pub fn selected_path(&self) -> PathBuf {
        let index = self.offset + self.selected;
        self.found[index].path()
    }

    pub fn selected_path_parent(&self) -> Box<Path> {
        let selected_path = self.selected_path();
        let selected_path_parent = selected_path
            .parent()
            .expect("The selected path is not a file.");
        Box::from(selected_path_parent)
    }

    pub fn delete_previous_character(&mut self) {
        self.pattern.pop();
        self.pattern_state = PatternState::NotCompiled;
    }

    pub fn append_character(&mut self, character: &char) {
        self.pattern.push(*character);
        self.pattern_state = PatternState::NotCompiled;
    }

    pub fn scroll_up(&mut self) {
        if self.selected == 0 {
            if self.offset > 0 {
                self.offset -= 1;
            }
        } else {
            self.selected -= 1;
        }
    }

    pub fn edit_file(&mut self) -> Effect {
        let filename = self.selected_path();
        let filename = Box::from(filename.as_path());
        Effect::RunVim(filename)
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

impl SearchState {
    pub fn found_hits(&self) -> bool {
        !self.hits.is_empty()
    }

    pub fn line_number(&self) -> Option<usize> {
        match self.file_selected {
            0 => match self.line_offset {
                Some(line_offset) => match self.line_selected {
                    Some(line_selected) => Some(line_offset + line_selected + 1),
                    None => Some(line_offset),
                },
                None => self.line_selected,
            },
            _ => self.line_selected,
        }
    }

    pub fn append_character(&mut self, character: &char) {
        self.search.push(*character);
    }

    pub fn delete_previous_character(&mut self) {
        self.search.pop();
    }

    pub fn increment_line_offset(&mut self) {
        self.line_offset = Some(match self.line_offset {
            Some(search_line_offset) => search_line_offset + 1,
            None => 0,
        });
    }

    pub fn decrement_line_selected(&mut self) {
        self.line_selected = match self.line_selected {
            Some(0) => None,
            Some(search_line_selected) => Some(search_line_selected - 1),
            None => Some(0),
        };
    }

    pub fn edit_file(&self) -> Effect {
        let search_file_number = self.file_offset + self.file_selected;
        let search_file_hit = self.hits[search_file_number].clone();

        let filename = search_file_hit.file.as_path();

        match self.line_number() {
            Some(search_line_number) => {
                let line_number = search_file_hit.hits[search_line_number].line_number;
                Effect::RunVimAtLine(Box::from(filename), line_number)
            }
            None => {
                let mut command = String::from("/");
                command.push_str(&self.search);
                Effect::RunVimWithCommand(Box::from(filename), command)
            }
        }
    }
}

use std::cmp::Ordering;
use std::convert::TryInto;
use std::env::current_dir;
use std::fs;
use std::io::{self, Stdout, Write};
use std::iter::FromIterator;
use std::path::PathBuf;

use crate::finder::Finder;
use crate::searcher::{SearchFileHit, Searcher};
use crate::vim::Vim;
use crate::walker::Walker;

extern crate crossterm;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor, Attribute:: {Bold}},
    terminal::{self, ClearType},
    QueueableCommand,
};

pub struct Insh {
    stdout: Stdout,
    terminal_size: (u16, u16),

    mode: Mode,

    // Browse mode state
    selected: usize,
    directory: Box<PathBuf>,
    entries: Vec<fs::DirEntry>,
    entry_offset: usize,

    // Find mode state
    pattern: String,
    pattern_state: PatternState,
    found: Vec<fs::DirEntry>,
    finder: Option<Finder>,
    find_offset: usize,
    find_selected: usize,

    // Search mode state
    search: String,
    hits: Vec<SearchFileHit>,
    searcher: Option<Searcher>,
}

#[derive(PartialEq)]
enum Mode {
    Browse,

    Find,
    BrowseFind,

    Search,
    BrowseSearch,
}

enum PatternState {
    NotCompiled,
    BadRegex,
    GoodRegex,
}

impl Insh {
    pub fn new() -> Insh {
        let stdout = io::stdout();
        let terminal_size = crossterm::terminal::size().unwrap();

        let mode = Mode::Browse;

        // Browse mode state
        let selected = 0;
        let directory: Box<PathBuf> = Box::new(current_dir().unwrap());
        let entries_iter = fs::read_dir(&*directory).unwrap();
        let entries: Vec<fs::DirEntry> = entries_iter
            .take(terminal_size.1.into())
            .map(|entry| entry.unwrap())
            .collect();
        let entry_offset = 0;

        // Find mode state
        let pattern = String::new();
        let pattern_state = PatternState::NotCompiled;
        let found = Vec::new();
        let finder = None;
        let find_offset = 0;
        let find_selected = 0;

        // Search mode state
        let search = String::new();
        let hits = Vec::new();
        let searcher = None;

        Insh {
            stdout,
            terminal_size,

            mode,

            // Browse mode state
            selected,
            directory,
            entries,
            entry_offset,

            // Find mode state
            pattern,
            pattern_state,
            found,
            finder,
            find_offset,
            find_selected,

            // Search mode state
            search,
            hits,
            searcher,
        }
    }

    fn get_entries(&mut self) -> Vec<fs::DirEntry> {
        let mut entries_iter = fs::read_dir(self.directory.as_path()).unwrap();
        for _ in 0..self.entry_offset {
            entries_iter.next();
        }

        Vec::from_iter(
            entries_iter
                .take(self.terminal_size.1.into())
                .map(|entry| entry.unwrap()),
        )
    }

    pub fn run(&mut self) {
        self.set_up();

        loop {
            let event = event::read().unwrap();

            if let Event::Key(KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::CONTROL,
            }) = event
            {
                break;
            }

            match self.mode {
                Mode::Browse => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => break,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    }) => {
                        if self.selected < self.terminal_size.1 as usize - 1 {
                            if self.selected < self.entries.len() - 1 {
                                self.selected += 1;
                            }
                        } else {
                            self.entry_offset += 1;
                            self.entries = self.get_entries();
                            if self.selected >= self.entries.len() {
                                self.entry_offset -= 1;
                                self.entries = self.get_entries();
                            }
                        }
                        self.lazy_display_browse();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    }) => {
                        if self.selected == 0 {
                            if self.entry_offset > 0 {
                                self.entry_offset -= 1;
                                self.entries = self.get_entries();
                                self.lazy_display_browse();
                                self.update_terminal();
                            }
                        } else {
                            self.selected -= 1;
                            self.entries = self.get_entries();
                            self.lazy_display_browse();
                            self.update_terminal();
                        }
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('l'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        if !self.entries.is_empty() {
                            let selected_path: PathBuf = self.entries[self.selected].path();

                            if selected_path.is_dir() {
                                self.directory.push(selected_path);
                                if !self.directory.exists() {
                                    self.directory.pop();
                                } else {
                                    self.selected = 0;
                                    self.entry_offset = 0;
                                    self.entries = self.get_entries();
                                    self.lazy_display_browse();
                                    self.update_terminal();
                                }
                            } else if selected_path.is_file() {
                                Vim::run(&selected_path);
                                self.lazy_hide_cursor();
                                self.lazy_display_browse();
                                self.update_terminal();
                            }
                        }
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('h'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    }) => {
                        self.directory.pop();
                        self.selected = 0;
                        self.entry_offset = 0;
                        self.entries = self.get_entries();
                        self.lazy_display_browse();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('e'),
                        ..
                    }) => {
                        let selected_path: PathBuf = self.entries[self.selected].path();
                        if selected_path.is_file() {
                            Vim::run(&selected_path);
                            self.lazy_hide_cursor();
                            self.lazy_display_browse();
                            self.update_terminal();
                        }
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('f'),
                        ..
                    }) => {
                        self.enter_find_mode();
                        self.lazy_display_find();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('s'),
                        ..
                    }) => {
                        self.enter_search_mode();
                        self.lazy_display_search();
                        self.update_terminal();
                    }
                    _ => {}
                },
                Mode::Find => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => {
                        self.enter_browse_mode();
                        self.lazy_display_browse();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    }) => {
                        self.pattern.pop();
                        self.pattern_state = PatternState::NotCompiled;
                        self.lazy_display_find();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        self.enter_browse_find_mode();
                        self.lazy_display_find();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char(c),
                        ..
                    }) => {
                        self.pattern.push(c);
                        self.pattern_state = PatternState::NotCompiled;
                        self.lazy_display_find();
                        self.update_terminal();
                    }
                    _ => {}
                },
                Mode::BrowseFind => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => {
                        self.mode = Mode::Find;
                        self.lazy_display_find();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    }) => {
                        if self.find_selected < self.terminal_size.1 as usize - 2 {
                            if self.find_selected + self.find_offset < self.found.len() - 1 {
                                self.find_selected += 1;
                            }
                        } else if self.find_selected + self.find_offset < self.found.len() - 1 {
                            self.find_offset += 1;
                        } else if let Some(ref mut finder) = self.finder {
                            match finder.next() {
                                Some(entry) => {
                                    self.found.push(entry);
                                    self.find_offset += 1;
                                }
                                None => {
                                    self.finder = None;
                                }
                            }
                        }

                        self.lazy_display_find();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    }) => {
                        if self.find_selected == 0 {
                            if self.find_offset > 0 {
                                self.find_offset -= 1;
                                self.lazy_display_find();
                                self.update_terminal();
                            }
                        } else {
                            self.find_selected -= 1;
                            self.lazy_display_find();
                            self.update_terminal();
                        }
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('e'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        let entry_index = self.find_offset + self.find_selected;
                        let selected_path: PathBuf = self.found[entry_index].path();
                        Vim::run(&selected_path);
                        self.lazy_hide_cursor();
                        self.lazy_display_find();
                        self.update_terminal();
                    }
                    _ => {}
                },
                Mode::Search => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => {
                        self.enter_browse_mode();
                        self.lazy_display_browse();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char(character),
                        ..
                    }) => {
                        self.search.push(character);
                        self.lazy_display_search();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    }) => {
                        self.search.pop();
                        self.lazy_display_search();
                        self.update_terminal();
                    }

                    Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        self.enter_browse_search_mode();

                        self.lazy_display_search();
                        self.update_terminal();
                    }
                    _ => {}
                },
                Mode::BrowseSearch => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => {
                        self.mode = Mode::Search;
                        self.lazy_display_search();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    }) => {
                        panic!("NOT IMPLEMENTED: Move selection down.");
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    }) => {
                        panic!("NOT IMPLEMENTED: Move selection up.");
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('e'),
                        ..
                    }) | Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        panic!("NOT IMPLEMENTED: Open editor at line hit.");
                    }
                    _ => {}
                },
            }
        }

        self.clean_up();
    }

    fn set_up(&mut self) {
        self.lazy_enable_alternate_terminal();
        self.enable_raw_terminal();
        self.lazy_hide_cursor();

        self.change_panic_hook();

        self.lazy_display_browse();
        self.update_terminal();
    }

    fn clean_up(&mut self) {
        self.lazy_disable_alternate_terminal();
        self.disable_raw_terminal();
        self.lazy_show_cursor();
    }

    fn enter_browse_mode(&mut self) {
        self.mode = Mode::Browse;
    }

    fn enter_find_mode(&mut self) {
        self.mode = Mode::Find;

        self.pattern.clear();
        self.pattern_state = PatternState::NotCompiled;

        let mut entries = Walker::from(&(*self.directory.as_path()));
        self.found.clear();
        let mut there_are_more = true;
        for _ in 0..(self.terminal_size.1 - 1).into() {
            let entry = entries.next();
            match entry {
                Some(entry) => self.found.push(entry),
                None => {
                    there_are_more = false;
                    break;
                }
            }
        }

        if there_are_more {
            self.finder = Some(Finder::from(entries));
        } else {
            self.finder = None;
        }

        self.find_offset = 0;
        self.find_selected = 0;
    }

    fn enter_browse_find_mode(&mut self) {
        self.mode = Mode::BrowseFind;

        self.found.clear();
        self.finder = None;

        let entries = Finder::new(&*self.directory, &self.pattern);
        match entries {
            Err(_) => {
                self.pattern_state = PatternState::BadRegex;
            }
            Ok(mut entries) => {
                self.pattern_state = PatternState::GoodRegex;
                for _ in 0..(self.terminal_size.1 - 1) {
                    let entry = entries.next();
                    match entry {
                        Some(entry) => self.found.push(entry),
                        None => break,
                    }
                }
                self.finder = Some(entries);
            }
        }
        self.find_selected = 0;
    }

    fn enter_search_mode(&mut self) {
        self.mode = Mode::Search;
        self.hits.clear();
        self.search.clear();
    }

    fn enter_browse_search_mode(&mut self) {
        self.mode = Mode::BrowseSearch;

        let mut searcher = Searcher::new(&*self.directory, &self.search);

        self.hits.clear();
        let mut lines: usize = 0;
        while lines < (self.terminal_size.1 - 1).into() {
            let hit = searcher.next();
            match hit {
                Some(search_file_hit) => {
                    lines += 1 + search_file_hit.hits.len();
                    self.hits.push(search_file_hit);
                }
                None => break,
            }
        }

        self.searcher = Some(searcher);
    }

    fn lazy_display_browse(&mut self) {
        self.lazy_clear_screen();

        for entry_number in 0..self.entries.len() {
            let file_name = self.entries[entry_number].file_name();
            let entry_name = file_name.to_string_lossy();

            self.lazy_move_cursor(0, entry_number as u16);

            let mut reset = false;
            if entry_number == self.selected {
                // Named arguments (not in Rust?) would be nice for lazy_color! Make a macro?
                self.lazy_start_color(Color::Black, Color::Yellow);
                reset = true;
            }
            self.lazy_print(&entry_name);

            let is_dir = self.entries[entry_number].path().is_dir();
            if is_dir {
                self.lazy_print("/");
            }

            if reset {
                self.lazy_reset_color()
            }
        }
    }

    fn lazy_display_find(&mut self) {
        self.lazy_clear_screen();

        // Display the search bar.
        self.lazy_move_cursor(0, 0);
        let pattern: &str = &(&self.pattern).clone();
        let truncated_pattern = match self
            .pattern
            .len()
            .partial_cmp(&(self.terminal_size.0 as usize))
            .unwrap()
        {
            Ordering::Greater | Ordering::Equal => &pattern[0..self.terminal_size.0 as usize],
            _ => pattern,
        };
        match self.pattern_state {
            PatternState::NotCompiled => {
                self.lazy_start_text_color(Color::Grey);
                self.lazy_print(truncated_pattern);
                self.lazy_reset_color();
            }
            PatternState::BadRegex => {
                self.lazy_start_text_color(Color::Red);
                self.lazy_print(truncated_pattern);
                self.lazy_reset_color();
            }
            PatternState::GoodRegex => {
                self.lazy_print(truncated_pattern);
            }
        }

        // Display found entries
        for entry_number in 0..(self.terminal_size.1 as usize - 1) {
            self.lazy_move_cursor(0, (entry_number + 1).try_into().unwrap());
            let entry_index = self.find_offset + entry_number;
            if entry_index == self.found.len() {
                break;
            }
            let file_name = self.found[entry_index].file_name();
            let entry_name = file_name.to_string_lossy();

            let reset;
            if entry_number == self.find_selected && self.mode == Mode::BrowseFind {
                self.lazy_start_color(Color::Black, Color::Yellow);
                reset = true;
            } else {
                reset = false;
            }

            self.lazy_print(&entry_name);

            if reset {
                self.lazy_reset_color()
            }
        }
    }

    fn lazy_display_search(&mut self) {
        self.lazy_clear_screen();

        // Display the search phrase.
        self.lazy_move_cursor(0, 0);
        let mut search = self.search.clone();
        search.truncate(self.terminal_size.0.into());
        self.lazy_print(&search);

        // Display hits.
        let mut lines = 0;
        let mut file_hit_number = 0;
        loop {
            // Print the file name.
            self.lazy_move_cursor(0, lines + 1);
            if file_hit_number >= self.hits.len() {
                break;
            }
            let file_hit = self.hits[file_hit_number].clone();
            self.lazy_start_bold();
            self.lazy_print(&file_hit.file.to_string_lossy());
            self.lazy_reset_color();
            lines += 1;
            if lines == (self.terminal_size.1 - 1).into() {
                break;
            }
            file_hit_number += 1;


            // Print the lines.
            let mut line_hit_number = 0;
            loop {
                let line_hit = file_hit.hits[line_hit_number].clone();

                self.lazy_move_cursor(0, lines + 1);
                let mut string = String::new();
                string.push_str(&line_hit.line_number.to_string());
                string.push(':');
                string.push_str(&line_hit.line);
                self.lazy_print(&string);
                // self.lazy_print(&line_hit.line);
                lines += 1;
                if lines == (self.terminal_size.1 - 1).into() {
                    return;
                }

                line_hit_number += 1;
                if line_hit_number == file_hit.hits.len() {
                    break;
                }
            }

            // Skip a line.
            lines += 1;
            if lines == (self.terminal_size.1 - 1).into() {
                break;
            }
            
        }
    }

    fn change_panic_hook(&mut self) {
        let hook_before = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let mut stdout = io::stdout();
            stdout.queue(terminal::LeaveAlternateScreen).unwrap();
            stdout.queue(cursor::Show).unwrap();
            stdout.flush().unwrap();
            terminal::disable_raw_mode().unwrap();
            hook_before(info);
        }));
    }

    fn enable_raw_terminal(&mut self) {
        terminal::enable_raw_mode().unwrap();
    }

    fn disable_raw_terminal(&mut self) {
        terminal::disable_raw_mode().unwrap();
    }

    fn lazy_enable_alternate_terminal(&mut self) {
        self.stdout.queue(terminal::EnterAlternateScreen).unwrap();
    }

    fn lazy_disable_alternate_terminal(&mut self) {
        self.stdout.queue(terminal::LeaveAlternateScreen).unwrap();
    }

    fn lazy_hide_cursor(&mut self) {
        self.stdout.queue(cursor::Hide).unwrap();
    }

    fn lazy_show_cursor(&mut self) {
        self.stdout.queue(cursor::Show).unwrap();
    }

    fn lazy_move_cursor(&mut self, x: u16, y: u16) {
        self.stdout.queue(cursor::MoveTo(x, y)).unwrap();
    }

    fn lazy_clear_screen(&mut self) {
        self.stdout.queue(terminal::Clear(ClearType::All)).unwrap();
    }

    fn lazy_start_color(&mut self, foreground: Color, background: Color) {
        self.stdout.queue(SetForegroundColor(foreground)).unwrap();
        self.stdout.queue(SetBackgroundColor(background)).unwrap();
    }

    fn lazy_start_text_color(&mut self, foreground: Color) {
        self.stdout.queue(SetForegroundColor(foreground)).unwrap();
    }

    fn lazy_reset_color(&mut self) {
        self.stdout.queue(ResetColor).unwrap();
    }

    fn lazy_start_bold(&mut self) {
        self.stdout.queue(SetAttribute(Bold)).unwrap();
    }

    fn lazy_print(&mut self, string: &str) {
        self.stdout.queue(Print(string)).unwrap();
    }

    fn update_terminal(&mut self) {
        self.stdout.flush().unwrap();
    }
}

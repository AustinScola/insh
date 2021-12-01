use std::convert::TryInto;
use std::env::current_dir;
use std::fs;
use std::io::{self, Stdout, Write};
use std::iter::FromIterator;
use std::path::PathBuf;

use crate::bash_shell::BashShell;
use crate::color::Color;
use crate::finder::Finder;
use crate::searcher::Searcher;
use crate::state::{BrowseState, FindState, Mode, PatternState, SearchState, State};
use crate::terminal_size::TerminalSize;
use crate::vim::Vim;
use crate::walker::Walker;

extern crate crossterm;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{
        Attribute::Bold, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal::{self, ClearType},
    QueueableCommand,
};

pub struct Insh {
    stdout: Stdout,

    state: State,
}

impl Insh {
    pub fn new() -> Insh {
        let stdout = io::stdout();

        let terminal_size: TerminalSize = crossterm::terminal::size().unwrap().into();

        let mode = Mode::Browse;

        // Browse mode state.
        let selected = 0;
        let directory: Box<PathBuf> = Box::new(current_dir().unwrap());
        let entries_iter = fs::read_dir(&*directory).unwrap();
        let entries: Vec<fs::DirEntry> = entries_iter
            .take(terminal_size.height.into())
            .map(|entry| entry.unwrap())
            .collect();
        let entry_offset = 0;

        let browse = BrowseState {
            selected,
            directory,
            entries,
            entry_offset,
        };

        // Find mode state.
        let pattern = String::new();
        let pattern_state = PatternState::NotCompiled;
        let found = Vec::new();
        let finder = None;
        let find_offset = 0;
        let find_selected = 0;

        let find = FindState {
            pattern,
            pattern_state,
            found,
            finder,
            find_offset,
            find_selected,
        };

        // Search mode state
        let search = String::new();
        let hits = Vec::new();
        let searcher = None;
        let search_file_offset = 0;
        let search_line_offset = None;
        let search_file_selected = 0;
        let search_line_selected = None;

        let search = SearchState {
            search,
            hits,
            searcher,
            search_file_offset,
            search_line_offset,
            search_file_selected,
            search_line_selected,
        };

        let state = State {
            terminal_size,

            mode,

            browse,
            find,
            search,
        };

        Insh { stdout, state }
    }

    fn get_selected_line(&mut self) -> usize {
        if self.state.search.search_file_offset >= self.state.search.hits.len() {
            return 0;
        }

        let mut selected_line = 0;

        let mut file_offset = self.state.search.search_file_offset;
        if self.state.search.search_file_selected == 0 {
            if let Some(search_line_selected) = self.state.search.search_line_selected {
                selected_line += search_line_selected + 1;
            }

            return selected_line;
        } else {
            selected_line += self.state.search.hits[file_offset].hits.len() + 1;
        }

        if let Some(search_line_offset) = self.state.search.search_line_offset {
            selected_line -= search_line_offset + 1;
        }

        if selected_line >= (self.state.terminal_size.height - 2).into() {
            return (self.state.terminal_size.height - 2).into();
        }

        file_offset += 1;
        selected_line += 1;

        loop {
            if file_offset
                >= self.state.search.search_file_offset + self.state.search.search_file_selected
            {
                break;
            }

            selected_line += self.state.search.hits[file_offset].hits.len() + 2;
            file_offset += 1;
        }

        if selected_line >= (self.state.terminal_size.height - 2).into() {
            return (self.state.terminal_size.height - 2).into();
        }

        if let Some(search_line_selected) = self.state.search.search_line_selected {
            selected_line += search_line_selected + 1;
        }

        if selected_line >= (self.state.terminal_size.height - 2).into() {
            return (self.state.terminal_size.height - 2).into();
        }

        selected_line
    }

    fn search_line_number(&mut self) -> Option<usize> {
        match self.state.search.search_file_selected {
            0 => match self.state.search.search_line_offset {
                Some(search_line_offset) => match self.state.search.search_line_selected {
                    Some(search_line_selected) => {
                        Some(search_line_offset + search_line_selected + 1)
                    }
                    None => Some(search_line_offset),
                },
                None => self.state.search.search_line_selected,
            },
            _ => self.state.search.search_line_selected,
        }
    }

    fn increment_search_line_selected(&mut self) {
        match self.state.search.search_line_selected {
            Some(search_line_selected) => {
                if search_line_selected < (self.state.terminal_size.height - 2).into() {
                    self.state.search.search_line_selected = Some(search_line_selected + 1);
                }
            }
            None => {
                self.state.search.search_line_selected = Some(0);
            }
        };
    }

    fn decrement_search_line_selected(&mut self) {
        self.state.search.search_line_selected = match self.state.search.search_line_selected {
            Some(0) => None,
            Some(search_line_selected) => Some(search_line_selected - 1),
            None => Some(0),
        };
    }

    fn increment_search_line_offset(&mut self) {
        self.state.search.search_line_offset = Some(match self.state.search.search_line_offset {
            Some(search_line_offset) => search_line_offset + 1,
            None => 0,
        });
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

            match self.state.mode {
                Mode::Browse => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => break,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    }) => {
                        if self.state.browse.selected < self.state.terminal_size.height as usize - 1
                        {
                            if self.state.browse.selected < self.state.browse.entries.len() - 1 {
                                self.state.browse.selected += 1;
                            }
                        } else {
                            self.state.browse.entry_offset += 1;
                            self.state.browse.entries = self.get_entries();
                            if self.state.browse.selected >= self.state.browse.entries.len() {
                                self.state.browse.entry_offset -= 1;
                                self.state.browse.entries = self.get_entries();
                            }
                        }
                        self.lazy_display_browse();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    }) => {
                        if self.state.browse.selected == 0 {
                            if self.state.browse.entry_offset > 0 {
                                self.state.browse.entry_offset -= 1;
                                self.state.browse.entries = self.get_entries();
                                self.lazy_display_browse();
                                self.update_terminal();
                            }
                        } else {
                            self.state.browse.selected -= 1;
                            self.state.browse.entries = self.get_entries();
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
                        if !self.state.browse.entries.is_empty() {
                            let selected_path: PathBuf =
                                self.state.browse.entries[self.state.browse.selected].path();

                            if selected_path.is_dir() {
                                self.state.browse.directory.push(selected_path);
                                if !self.state.browse.directory.exists() {
                                    self.state.browse.directory.pop();
                                } else {
                                    self.state.browse.selected = 0;
                                    self.state.browse.entry_offset = 0;
                                    self.state.browse.entries = self.get_entries();
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
                        self.state.browse.directory.pop();
                        self.state.browse.selected = 0;
                        self.state.browse.entry_offset = 0;
                        self.state.browse.entries = self.get_entries();
                        self.lazy_display_browse();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('e'),
                        ..
                    }) => {
                        let selected_path: PathBuf =
                            self.state.browse.entries[self.state.browse.selected].path();
                        if selected_path.is_file() {
                            Vim::run(&selected_path);
                            self.lazy_hide_cursor();
                            self.lazy_display_browse();
                            self.update_terminal();
                        }
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('b'),
                        ..
                    }) => {
                        self.disable_raw_terminal();
                        self.lazy_clear_screen();
                        self.lazy_move_cursor(0, 0);
                        self.lazy_show_cursor();
                        self.update_terminal();

                        BashShell::run(&self.state.browse.directory);

                        self.enable_raw_terminal();
                        self.lazy_hide_cursor();

                        self.lazy_display_browse();
                        self.update_terminal();
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
                        self.state.find.pattern.pop();
                        self.state.find.pattern_state = PatternState::NotCompiled;
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
                        self.state.find.pattern.push(c);
                        self.state.find.pattern_state = PatternState::NotCompiled;
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
                        self.state.mode = Mode::Find;
                        self.lazy_display_find();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    }) => {
                        if self.state.find.find_selected
                            < self.state.terminal_size.height as usize - 2
                        {
                            if self.state.find.find_selected + self.state.find.find_offset
                                < self.state.find.found.len() - 1
                            {
                                self.state.find.find_selected += 1;
                            }
                        } else if self.state.find.find_selected + self.state.find.find_offset
                            < self.state.find.found.len() - 1
                        {
                            self.state.find.find_offset += 1;
                        } else if let Some(ref mut finder) = self.state.find.finder {
                            match finder.next() {
                                Some(entry) => {
                                    self.state.find.found.push(entry);
                                    self.state.find.find_offset += 1;
                                }
                                None => {
                                    self.state.find.finder = None;
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
                        if self.state.find.find_selected == 0 {
                            if self.state.find.find_offset > 0 {
                                self.state.find.find_offset -= 1;
                                self.lazy_display_find();
                                self.update_terminal();
                            }
                        } else {
                            self.state.find.find_selected -= 1;
                            self.lazy_display_find();
                            self.update_terminal();
                        }
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('e'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Char('l'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        let entry_index =
                            self.state.find.find_offset + self.state.find.find_selected;
                        let selected_path: PathBuf = self.state.find.found[entry_index].path();
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
                        self.state.search.search.push(character);
                        self.lazy_display_search();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    }) => {
                        self.state.search.search.pop();
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
                        self.state.mode = Mode::Search;
                        self.lazy_display_search();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    }) => {
                        let first_file =
                            self.state.search.hits[self.state.search.search_file_offset].clone();
                        let selected_line = self.get_selected_line();
                        let search_file_number = self.state.search.search_file_offset
                            + self.state.search.search_file_selected;
                        let search_file_hit = self.state.search.hits[search_file_number].clone();

                        let search_line_number = self.search_line_number();

                        if selected_line == (self.state.terminal_size.height - 2).into() {
                            if search_line_number >= Some(search_file_hit.hits.len() - 1) {
                                if search_file_number == self.state.search.hits.len() - 1 {
                                    if let Some(ref mut searcher) = self.state.search.searcher {
                                        match searcher.next() {
                                            Some(hit) => {
                                                self.state.search.hits.push(hit);

                                                self.state.search.search_line_selected = None;
                                                self.state.search.search_file_selected += 1;

                                                if self.state.search.search_line_offset == None {
                                                    self.state.search.search_line_offset = Some(0);
                                                } else if self.state.search.search_line_offset
                                                    < Some(first_file.hits.len())
                                                {
                                                    self.increment_search_line_offset()
                                                } else {
                                                    self.state.search.search_file_offset += 1;
                                                    self.state.search.search_line_offset = None;
                                                    self.state.search.search_file_selected -= 1;
                                                }

                                                let first_file = self.state.search.hits
                                                    [self.state.search.search_file_offset]
                                                    .clone();

                                                if self.state.search.search_line_offset == None {
                                                    self.state.search.search_line_offset = Some(0);
                                                } else if self.state.search.search_line_offset
                                                    < Some(first_file.hits.len())
                                                {
                                                    self.increment_search_line_offset()
                                                } else {
                                                    self.state.search.search_file_offset += 1;
                                                    self.state.search.search_line_offset = None;
                                                    self.state.search.search_file_selected -= 1;
                                                }
                                            }
                                            None => {
                                                self.state.search.searcher = None;
                                            }
                                        }
                                    }
                                } else {
                                    self.state.search.search_line_selected = None;
                                    self.state.search.search_file_selected += 1;

                                    if self.state.search.search_line_offset == None {
                                        self.state.search.search_line_offset = Some(0);
                                    } else if self.state.search.search_line_offset
                                        < Some(first_file.hits.len())
                                    {
                                        self.increment_search_line_offset()
                                    } else {
                                        self.state.search.search_file_offset += 1;
                                        self.state.search.search_line_offset = None;
                                        self.state.search.search_file_selected -= 1;
                                    }

                                    let first_file = self.state.search.hits
                                        [self.state.search.search_file_offset]
                                        .clone();

                                    if self.state.search.search_line_offset == None {
                                        self.state.search.search_line_offset = Some(0);
                                    } else if self.state.search.search_line_offset
                                        < Some(first_file.hits.len())
                                    {
                                        self.increment_search_line_offset()
                                    } else {
                                        self.state.search.search_file_offset += 1;
                                        self.state.search.search_line_offset = None;
                                        self.state.search.search_file_selected -= 1;
                                    }
                                }
                            } else if self.state.search.search_line_offset == None {
                                if self.state.search.search_file_selected != 0 {
                                    self.increment_search_line_selected();
                                }
                                self.state.search.search_line_offset = Some(0);
                            } else if self.state.search.search_line_offset
                                < Some(first_file.hits.len())
                            {
                                if self.state.search.search_file_selected != 0 {
                                    self.increment_search_line_selected();
                                }
                                self.increment_search_line_offset()
                            } else {
                                self.increment_search_line_selected();
                                self.state.search.search_file_offset += 1;
                                self.state.search.search_line_offset = None;
                                self.state.search.search_file_selected -= 1;
                            }
                        } else if selected_line == (self.state.terminal_size.height - 3).into()
                            && search_line_number >= Some(search_file_hit.hits.len() - 1)
                        {
                            self.state.search.search_line_selected = None;
                            self.state.search.search_file_selected += 1;

                            if self.state.search.search_line_offset == None {
                                self.state.search.search_line_offset = Some(0);
                            } else if self.state.search.search_line_offset
                                < Some(first_file.hits.len())
                            {
                                self.increment_search_line_offset()
                            } else {
                                self.state.search.search_file_offset += 1;
                                self.state.search.search_line_offset = None;
                                self.state.search.search_file_selected -= 1;
                            }
                        } else if search_line_number >= Some(search_file_hit.hits.len() - 1) {
                            if search_file_number < self.state.search.hits.len() - 1 {
                                self.state.search.search_file_selected += 1;
                                self.state.search.search_line_selected = None;
                            }
                        } else {
                            self.increment_search_line_selected();
                        }

                        self.lazy_display_search();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    }) => {
                        // Determine the line on the screen that is selected.
                        let selected_line = self.get_selected_line();

                        if selected_line == 0 {
                            match self.state.search.search_line_offset {
                                None => {
                                    if self.state.search.search_file_offset != 0 {
                                        self.state.search.search_file_offset -= 1;
                                        self.state.search.search_line_offset = Some(
                                            self.state.search.hits
                                                [self.state.search.search_file_offset]
                                                .hits
                                                .len()
                                                - 1,
                                        );
                                    }
                                }
                                Some(0) => {
                                    self.state.search.search_line_offset = None;
                                }
                                Some(search_line_offset) => {
                                    self.state.search.search_line_offset =
                                        Some(search_line_offset - 1);
                                }
                            }
                        } else if self.state.search.search_line_selected.is_none() {
                            if self.state.search.search_file_selected == 0 {
                                if self.state.search.search_file_offset > 0 {
                                    self.state.search.search_file_offset -= 1;

                                    let search_file_number = self.state.search.search_file_offset
                                        + self.state.search.search_file_selected;
                                    let search_file_hit =
                                        self.state.search.hits[search_file_number].clone();
                                    self.state.search.search_line_selected =
                                        Some(search_file_hit.hits.len() - 1);
                                }
                            } else {
                                self.state.search.search_file_selected -= 1;
                                if selected_line == 1 {
                                    self.state.search.search_line_selected = None;
                                    let search_file_number = self.state.search.search_file_offset
                                        + self.state.search.search_file_selected;
                                    let search_file_hit =
                                        self.state.search.hits[search_file_number].clone();
                                    self.state.search.search_line_offset =
                                        Some(search_file_hit.hits.len() - 1);
                                } else {
                                    let search_file_number = self.state.search.search_file_offset
                                        + self.state.search.search_file_selected;
                                    let search_file_hit =
                                        self.state.search.hits[search_file_number].clone();
                                    if self.state.search.search_file_selected == 0 {
                                        self.state.search.search_line_selected =
                                            match self.state.search.search_line_offset {
                                                Some(search_line_offset) => {
                                                    if search_line_offset
                                                        == search_file_hit.hits.len() - 1
                                                    {
                                                        None
                                                    } else {
                                                        Some(
                                                            search_file_hit.hits.len()
                                                                - search_line_offset
                                                                - 2,
                                                        )
                                                    }
                                                }
                                                None => Some(search_file_hit.hits.len() - 1),
                                            }
                                    } else {
                                        self.state.search.search_line_selected =
                                            Some(search_file_hit.hits.len() - 1);
                                    }
                                }
                            }
                        } else {
                            self.decrement_search_line_selected();
                        }

                        self.lazy_display_search();
                        self.update_terminal();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('e'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Char('l'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        let search_file_number = self.state.search.search_file_offset
                            + self.state.search.search_file_selected;
                        let search_file_hit = self.state.search.hits[search_file_number].clone();

                        match self.search_line_number() {
                            Some(search_line_number) => {
                                let line_number =
                                    search_file_hit.hits[search_line_number].line_number;
                                Vim::run_at_line(search_file_hit.file.as_path(), line_number);
                            }
                            None => {
                                let mut command = String::from("/");
                                command.push_str(&self.state.search.search);
                                Vim::run_with_command(search_file_hit.file.as_path(), command);
                            }
                        }

                        self.lazy_display_search();
                        self.update_terminal();
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
        self.state.mode = Mode::Browse;
    }

    fn enter_find_mode(&mut self) {
        self.state.mode = Mode::Find;

        self.state.find.pattern.clear();
        self.state.find.pattern_state = PatternState::NotCompiled;

        let mut entries = Walker::from(&(*self.state.browse.directory.as_path()));
        self.state.find.found.clear();
        let mut there_are_more = true;
        for _ in 0..(self.state.terminal_size.height - 1).into() {
            let entry = entries.next();
            match entry {
                Some(entry) => self.state.find.found.push(entry),
                None => {
                    there_are_more = false;
                    break;
                }
            }
        }

        if there_are_more {
            self.state.find.finder = Some(Finder::from(entries));
        } else {
            self.state.find.finder = None;
        }

        self.state.find.find_offset = 0;
        self.state.find.find_selected = 0;
    }

    fn enter_browse_find_mode(&mut self) {
        self.state.mode = Mode::BrowseFind;

        self.state.find.found.clear();
        self.state.find.finder = None;

        let entries = Finder::new(&*self.state.browse.directory, &self.state.find.pattern);
        match entries {
            Err(_) => {
                self.state.find.pattern_state = PatternState::BadRegex;
            }
            Ok(mut entries) => {
                self.state.find.pattern_state = PatternState::GoodRegex;
                for _ in 0..(self.state.terminal_size.height - 1) {
                    let entry = entries.next();
                    match entry {
                        Some(entry) => self.state.find.found.push(entry),
                        None => break,
                    }
                }
                self.state.find.finder = Some(entries);
            }
        }
        self.state.find.find_selected = 0;
    }

    fn enter_search_mode(&mut self) {
        self.state.mode = Mode::Search;
        self.state.search.hits.clear();
        self.state.search.search.clear();

        self.state.search.search_file_offset = 0;
        self.state.search.search_line_offset = None;
        self.state.search.search_file_selected = 0;
        self.state.search.search_line_selected = None;
    }

    fn enter_browse_search_mode(&mut self) {
        self.state.mode = Mode::BrowseSearch;

        let mut searcher = Searcher::new(&*self.state.browse.directory, &self.state.search.search);

        self.state.search.hits.clear();
        let mut lines: usize = 0;
        while lines < (self.state.terminal_size.height - 1).into() {
            let hit = searcher.next();
            match hit {
                Some(search_file_hit) => {
                    lines += 1 + search_file_hit.hits.len();
                    self.state.search.hits.push(search_file_hit);
                }
                None => break,
            }
        }

        self.state.search.searcher = Some(searcher);
    }

    fn get_entries(&mut self) -> Vec<fs::DirEntry> {
        let mut entries_iter = fs::read_dir(self.state.browse.directory.as_path()).unwrap();
        for _ in 0..self.state.browse.entry_offset {
            entries_iter.next();
        }

        Vec::from_iter(
            entries_iter
                .take(self.state.terminal_size.height.into())
                .map(|entry| entry.unwrap()),
        )
    }

    fn lazy_display_browse(&mut self) {
        self.lazy_clear_screen();

        for entry_number in 0..self.state.browse.entries.len() {
            let file_name = self.state.browse.entries[entry_number].file_name();
            let entry_name = file_name.to_string_lossy();

            self.lazy_move_cursor(0, entry_number as u16);

            let mut reset = false;
            if entry_number == self.state.browse.selected {
                // Named arguments (not in Rust?) would be nice for lazy_color! Make a macro?
                self.lazy_start_color(Color::InvertedText, Color::Highlight);
                reset = true;
            }
            self.lazy_print(&entry_name);

            let is_dir = self.state.browse.entries[entry_number].path().is_dir();
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

        self.lazy_start_background_color(Color::InvertedBackground);
        let mut pattern = self.state.find.pattern.clone();
        pattern.truncate(self.state.terminal_size.width.into());
        pattern = format!(
            "{:width$}",
            pattern,
            width = self.state.terminal_size.width as usize
        );

        let text_color = match self.state.find.pattern_state {
            PatternState::NotCompiled => Color::NotCompiledRegex,
            PatternState::BadRegex => Color::BadRegex,
            PatternState::GoodRegex => Color::InvertedText,
        };

        self.lazy_start_text_color(text_color);
        self.lazy_print(&pattern);
        self.lazy_reset_color();

        // Display found entries
        for entry_number in 0..(self.state.terminal_size.height as usize - 1) {
            self.lazy_move_cursor(0, (entry_number + 1).try_into().unwrap());
            let entry_index = self.state.find.find_offset + entry_number;
            if entry_index == self.state.find.found.len() {
                break;
            }
            let file_name = self.state.find.found[entry_index].file_name();
            let entry_name = file_name.to_string_lossy();

            let reset;
            if entry_number == self.state.find.find_selected && self.state.mode == Mode::BrowseFind
            {
                self.lazy_start_color(Color::InvertedText, Color::Highlight);
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

        self.lazy_start_background_color(Color::InvertedBackground);

        let mut search = self.state.search.search.clone();
        search.truncate(self.state.terminal_size.width.into());
        search = format!(
            "{:width$}",
            search,
            width = self.state.terminal_size.width as usize
        );

        self.lazy_start_text_color(Color::InvertedText);
        self.lazy_print(&search);
        self.lazy_reset_color();

        let selected_line = self.get_selected_line();

        let mut lines = 0;

        // Display the first hit.

        let mut file_hit_number = self.state.search.search_file_offset;

        if self.state.search.hits.is_empty() {
            return;
        }

        // Print the first file name.
        if self.state.search.search_line_offset == None {
            self.lazy_move_cursor(0, 1);
            let file_hit = self.state.search.hits[file_hit_number].clone();
            if usize::from(lines) == selected_line {
                self.lazy_start_color(Color::InvertedText, Color::Highlight);
            }
            self.lazy_start_bold();
            self.lazy_print(&file_hit.file.to_string_lossy());
            self.lazy_reset_color();
            lines += 1;
            if lines == (self.state.terminal_size.height - 1) {
                return;
            }
        }

        // Print the line hits of the first file hit.
        let mut line_hit_number = self.state.search.search_line_offset.unwrap_or(0);
        let file_hit = self.state.search.hits[file_hit_number].clone();
        loop {
            if line_hit_number == file_hit.hits.len() {
                // Add a blank line between the first hit and the rest of the hits.
                lines += 1;
                if lines == (self.state.terminal_size.height - 1) {
                    return;
                }
                break;
            }
            if line_hit_number > file_hit.hits.len() {
                break;
            }

            let line_hit = file_hit.hits[line_hit_number].clone();

            self.lazy_move_cursor(0, lines + 1);

            let mut reset_color = false;
            if usize::from(lines) == selected_line {
                self.lazy_start_color(Color::InvertedText, Color::Highlight);
                reset_color = true;
            }

            let mut string = String::new();
            string.push_str(&line_hit.line_number.to_string());
            string.push(':');
            string.push_str(&line_hit.line);
            self.lazy_print(&string);

            if reset_color {
                self.lazy_reset_color();
            }

            lines += 1;
            if lines == (self.state.terminal_size.height - 1) {
                return;
            }

            line_hit_number += 1;
        }

        file_hit_number += 1;
        line_hit_number = 0;

        // Display the rest of the hits.
        loop {
            // Print the file name.
            self.lazy_move_cursor(0, lines + 1);
            if file_hit_number >= self.state.search.hits.len() {
                break;
            }
            let file_hit = self.state.search.hits[file_hit_number].clone();
            if usize::from(lines) == selected_line {
                self.lazy_start_color(Color::InvertedText, Color::Highlight);
            }
            self.lazy_start_bold();
            self.lazy_print(&file_hit.file.to_string_lossy());
            self.lazy_reset_color();
            lines += 1;
            if lines == (self.state.terminal_size.height - 1) {
                break;
            }
            file_hit_number += 1;

            // Print the lines.
            loop {
                let line_hit = file_hit.hits[line_hit_number].clone();

                self.lazy_move_cursor(0, lines + 1);

                let mut reset_color = false;
                if usize::from(lines) == selected_line {
                    self.lazy_start_color(Color::InvertedText, Color::Highlight);
                    reset_color = true;
                }

                let mut string = String::new();
                string.push_str(&line_hit.line_number.to_string());
                string.push(':');
                string.push_str(&line_hit.line);
                self.lazy_print(&string);

                if reset_color {
                    self.lazy_reset_color();
                }

                lines += 1;
                if lines == (self.state.terminal_size.height - 1) {
                    return;
                }

                line_hit_number += 1;
                if line_hit_number == file_hit.hits.len() {
                    break;
                }
            }

            line_hit_number = 0;

            // Skip a line.
            lines += 1;
            if lines == (self.state.terminal_size.height - 1) {
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
        self.stdout
            .queue(SetForegroundColor(foreground.into()))
            .unwrap();
        self.stdout
            .queue(SetBackgroundColor(background.into()))
            .unwrap();
    }

    fn lazy_start_text_color(&mut self, color: Color) {
        self.stdout.queue(SetForegroundColor(color.into())).unwrap();
    }

    fn lazy_start_background_color(&mut self, color: Color) {
        self.stdout.queue(SetBackgroundColor(color.into())).unwrap();
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

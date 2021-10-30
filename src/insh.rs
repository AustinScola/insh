extern crate crossterm;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType},
    QueueableCommand,
};
use regex::Regex;
use std::cmp::Ordering;
use std::convert::TryInto;
use std::env::current_dir;
use std::fs;
use std::io::{self, Stdout, Write};
use std::iter::FromIterator;
use std::mem::swap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

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
    more: Option<Finder>,
}

enum Mode {
    Browse,
    Find,
}

enum PatternState {
    NotCompiled,
    BadRegex,
    GoodRegex,
}

struct Finder {
    regex: Regex,
    stack: Vec<Box<dyn Iterator<Item = fs::DirEntry>>>,
    iterator: Box<dyn Iterator<Item = fs::DirEntry>>,
}

impl Finder {
    fn new(directory: &Path, pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        let stack = Vec::new();
        let iterator = Finder::get_directory_iterator(directory, regex.clone());

        Ok(Finder {
            regex,
            stack,
            iterator,
        })
    }

    fn get_directory_iterator(
        directory: &Path,
        regex: Regex,
    ) -> Box<dyn Iterator<Item = fs::DirEntry>> {
        let directory_entries = fs::read_dir(&*directory).unwrap();
        let filtered_entries = directory_entries
            .filter(move |entry| {
                entry.as_ref().unwrap().path().is_dir()
                    || regex.is_match(&entry.as_ref().unwrap().file_name().to_string_lossy())
            })
            .map(|entry| entry.unwrap());
        Box::new(filtered_entries)
    }
}

impl Iterator for Finder {
    type Item = fs::DirEntry;

    fn next(&mut self) -> Option<fs::DirEntry> {
        let next_entry = self.iterator.next();
        match next_entry {
            Some(entry) => {
                if entry.path().is_dir() {
                    let mut iterator =
                        Finder::get_directory_iterator(&entry.path(), self.regex.clone());
                    swap(&mut iterator, &mut self.iterator);
                    self.stack.push(Box::new(iterator));
                    self.next()
                } else {
                    Some(entry)
                }
            }
            None => {
                let next_iterator = self.stack.pop();
                match next_iterator {
                    Some(iterator) => {
                        self.iterator = Box::new(iterator);
                        self.next()
                    }
                    None => None,
                }
            }
        }
    }
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
        let more = None;

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
            more,
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
            match self.mode {
                Mode::Browse => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        ..
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
                            let mut vim: Child = Command::new("vim")
                                .arg(selected_path)
                                .stdin(Stdio::inherit())
                                .stdout(Stdio::inherit())
                                .spawn()
                                .unwrap();
                            vim.wait().unwrap();
                            self.lazy_hide_cursor();
                            self.lazy_display_browse();
                            self.update_terminal();
                        }
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('f'),
                        ..
                    }) => {
                        self.mode = Mode::Find;
                        self.lazy_display_find();
                        self.update_terminal();
                    }
                    _ => {}
                },
                Mode::Find => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Esc, ..
                    }) => {
                        self.mode = Mode::Browse;
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
                        self.found.clear();
                        self.more = None;

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
                                self.more = Some(entries);
                            }
                        }

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
        for entry_number in 0..self.found.len() {
            self.lazy_move_cursor(0, (entry_number + 1).try_into().unwrap());
            let file_name = self.found[entry_number].file_name();
            let entry_name = file_name.to_string_lossy();
            self.lazy_print(&entry_name);
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

    fn lazy_print(&mut self, string: &str) {
        self.stdout.queue(Print(string)).unwrap();
    }

    fn update_terminal(&mut self) {
        self.stdout.flush().unwrap();
    }
}

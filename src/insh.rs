extern crate crossterm;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType},
    QueueableCommand,
};
use std::env::current_dir;
use std::fs;
use std::io::{self, Stdout, Write};
use std::iter::FromIterator;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub struct Insh {
    stdout: Stdout,
    terminal_size: (u16, u16),
    selected: usize,
    directory: Box<PathBuf>,
    entries: Vec<fs::DirEntry>,
    entry_offset: usize,
}

impl Insh {
    pub fn new() -> Insh {
        let stdout = io::stdout();
        let terminal_size = crossterm::terminal::size().unwrap();
        let selected = 0;
        let directory: Box<PathBuf> = Box::new(current_dir().unwrap());
        let entries_iter = fs::read_dir(&*directory).unwrap();
        let entries: Vec<fs::DirEntry> = Vec::from_iter(
            entries_iter
                .take(terminal_size.1.into())
                .map(|entry| entry.unwrap()),
        );
        let entry_offset = 0;

        Insh {
            stdout,
            terminal_size,
            selected,
            directory,
            entries,
            entry_offset,
        }
    }

    fn get_entries(&mut self) -> Vec<fs::DirEntry> {
        let mut entries_iter = fs::read_dir(self.directory.as_path()).unwrap();
        for _ in 0..self.entry_offset {
            entries_iter.next();
        }

        let entries = Vec::from_iter(
            entries_iter
                .take(self.terminal_size.1.into())
                .map(|entry| entry.unwrap()),
        );
        return entries;
    }

    pub fn run(&mut self) {
        self.set_up();

        loop {
            let event = event::read().unwrap();
            match event {
                Event::Key(KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                }) => break,
                Event::Key(KeyEvent {
                    code: KeyCode::Char('j'),
                    ..
                }) => {
                    if self.selected < self.terminal_size.1 as usize - 1 {
                        self.selected += 1;
                    } else {
                        self.entry_offset += 1
                    }
                    self.entries = self.get_entries();
                    self.lazy_display_directory();
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('k'),
                    ..
                }) => {
                    if self.selected == 0 {
                        if self.entry_offset > 0 {
                            self.entry_offset -= 1
                        }
                    } else {
                        self.selected -= 1
                    }
                    self.entries = self.get_entries();
                    self.lazy_display_directory();
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('l'),
                    ..
                }) => {
                    if self.entries.len() > 0 {
                        let selected_path: PathBuf = self.entries[self.selected].path();

                        if selected_path.is_dir() {
                            self.directory.push(selected_path);
                            if !self.directory.exists() {
                                self.directory.pop();
                            } else {
                                self.selected = 0;
                                self.entry_offset = 0;
                                self.entries = self.get_entries();
                                self.lazy_display_directory();
                            }
                        }
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('h'),
                    ..
                }) => {
                    self.directory.pop();
                    self.selected = 0;
                    self.entry_offset = 0;
                    self.entries = self.get_entries();
                    self.lazy_display_directory();
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
                        self.lazy_display_directory();
                    }
                }
                _ => {}
            }
        }

        self.clean_up();
    }

    fn set_up(&mut self) {
        self.lazy_enable_alternate_terminal();
        self.enable_raw_terminal();
        self.lazy_hide_cursor();

        self.lazy_display_directory();
        self.update_terminal();
    }

    fn clean_up(&mut self) {
        self.lazy_disable_alternate_terminal();
        self.disable_raw_terminal();
        self.lazy_show_cursor();
    }

    fn lazy_display_directory(&mut self) {
        self.lazy_clear_screen();

        for entry_number in 0..self.entries.len() {
            let file_name = self.entries[entry_number].file_name();
            let entry_name = file_name.to_string_lossy();

            self.lazy_move_cursor(0, entry_number as u16);

            let mut reset = false;
            if usize::from(entry_number) == self.selected {
                // Named arguments (not in Rust?) would be nice for lazy_color! Make a macro?
                self.lazy_start_color(Color::Yellow, Color::Black);
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
        self.update_terminal();
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

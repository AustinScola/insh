extern crate termion;

use std::convert::TryInto;
use std::env::current_dir;
use std::fs;
use std::io::{stdin, stdout, Stdout, Write};
use std::iter::FromIterator;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use termion::color;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen::*;

pub struct Insh {
    screen: termion::screen::AlternateScreen<RawTerminal<Stdout>>,
    terminal_size: (u16, u16),
    selected: usize,
    directory: Box<PathBuf>,
    entries: Vec<fs::DirEntry>,
    entry_offset: usize,
}

impl Insh {
    pub fn new() -> Insh {
        let screen = AlternateScreen::from(stdout().into_raw_mode().unwrap());
        let terminal_size = termion::terminal_size().unwrap();
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
            screen,
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
        self.hide_cursor();
        self.display_directory();

        let stdin = stdin();
        for character in stdin.lock().keys() {
            match character.unwrap() {
                Key::Char('q') => break,
                Key::Char('j') => {
                    if self.selected < self.terminal_size.1 as usize - 1 {
                        self.selected += 1;
                    } else {
                        self.entry_offset += 1
                    }
                    self.entries = self.get_entries();
                    self.display_directory();
                }
                Key::Char('k') => {
                    if self.selected == 0 {
                        if self.entry_offset > 0 {
                            self.entry_offset -= 1
                        }
                    } else {
                        self.selected -= 1
                    }
                    self.entries = self.get_entries();
                    self.display_directory();
                }
                Key::Char('l') => {
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
                                self.display_directory();
                            }
                        }
                    }
                }
                Key::Char('h') => {
                    self.directory.pop();
                    self.selected = 0;
                    self.entry_offset = 0;
                    self.entries = self.get_entries();
                    self.display_directory();
                }
                Key::Char('e') => {
                    let selected_path: PathBuf = self.entries[self.selected].path();
                    if selected_path.is_file() {
                        let mut vim: Child = Command::new("vim")
                            .arg(selected_path)
                            .stdin(Stdio::inherit())
                            .stdout(Stdio::inherit())
                            .spawn()
                            .unwrap();
                        vim.wait().unwrap();
                        self.hide_cursor();
                        self.display_directory();
                    }
                }
                _ => {}
            }
        }

        self.show_cursor();
    }

    fn hide_cursor(&mut self) {
        write!(self.screen, "{}", termion::cursor::Hide).unwrap();
    }

    fn show_cursor(&mut self) {
        write!(self.screen, "{}", termion::cursor::Show).unwrap();
    }

    fn move_cursor(
        screen: &mut termion::screen::AlternateScreen<RawTerminal<Stdout>>,
        x: usize,
        y: usize,
    ) {
        write!(
            screen,
            "{}",
            termion::cursor::Goto((x + 1).try_into().unwrap(), (y + 1).try_into().unwrap())
        )
        .unwrap()
    }

    fn display_directory(&mut self) {
        write!(self.screen, "{}", termion::clear::All).unwrap();

        for (entry_number, entry) in self.entries.iter().enumerate() {
            Insh::move_cursor(&mut self.screen, 0, entry_number.into());

            let file_name = entry.file_name();
            let entry_name = file_name.to_string_lossy();
            let mut reset = false;
            if usize::from(entry_number) == self.selected {
                write!(
                    self.screen,
                    "{}{}",
                    color::Bg(color::White),
                    color::Fg(color::Black),
                )
                .unwrap();
                reset = true;
            }
            write!(self.screen, "{}", entry_name).unwrap();
            if entry.path().is_dir() {
                write!(self.screen, "/").unwrap();
            }
            if reset {
                write!(
                    self.screen,
                    "{}{}",
                    color::Bg(color::Reset),
                    color::Fg(color::Reset),
                )
                .unwrap();
            }
        }
        self.screen.flush().unwrap();
    }
}

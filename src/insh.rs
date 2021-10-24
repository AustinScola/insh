extern crate termion;

use std::convert::TryInto;
use std::fs;
use std::io::{stdin, stdout, Stdout, Write};
use termion::color;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen::*;

pub struct Insh {
    screen: termion::screen::AlternateScreen<RawTerminal<Stdout>>,
    terminal_size: (u16, u16),
    offset: usize,
    selected: usize,
}

impl Insh {
    pub fn new() -> Self {
        let screen = AlternateScreen::from(stdout().into_raw_mode().unwrap());
        let terminal_size = termion::terminal_size().unwrap();
        let offset = 0;
        let selected = 0;

        Insh {
            screen,
            terminal_size,
            offset,
            selected,
        }
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
                        self.offset += 1
                    }
                    self.display_directory();
                }
                Key::Char('k') => {
                    if self.selected == 0 {
                        if self.offset > 0 {
                            self.offset -= 1
                        }
                    } else {
                        self.selected -= 1
                    }
                    self.display_directory();
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

    fn move_cursor(&mut self, x: usize, y: usize) {
        write!(
            self.screen,
            "{}",
            termion::cursor::Goto(x.try_into().unwrap(), (y + 1).try_into().unwrap())
        )
        .unwrap()
    }

    fn display_directory(&mut self) {
        write!(self.screen, "{}", termion::clear::All).unwrap();

        if let Ok(mut entries) = fs::read_dir(".") {
            for _ in 0..self.offset {
                entries.next();
            }

            for (entry, entry_number) in entries.zip(0..self.terminal_size.1) {
                self.move_cursor(1, entry_number.into());

                if let Ok(entry) = entry {
                    let file_name = entry.file_name();
                    let entry_name = file_name.to_string_lossy();
                    if usize::from(entry_number) == self.selected {
                        write!(
                            self.screen,
                            "{}{}{}{}{}",
                            color::Bg(color::White),
                            color::Fg(color::Black),
                            entry_name,
                            color::Bg(color::Reset),
                            color::Fg(color::Reset)
                        )
                        .unwrap();
                    } else {
                        write!(self.screen, "{}", entry_name).unwrap();
                    }
                }
            }
        }

        self.screen.flush().unwrap();
    }
}

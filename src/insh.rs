extern crate termion;

use std::convert::TryInto;
use std::fs;
use std::io::{stdin, stdout, Stdout, Write};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen::*;

pub struct Insh {
    stdin: std::io::Stdin,
    screen: termion::screen::AlternateScreen<RawTerminal<Stdout>>,
}

impl Insh {
    pub fn new() -> Self {
        let stdin = stdin();
        let screen = AlternateScreen::from(stdout().into_raw_mode().unwrap());

        Insh { stdin, screen }
    }

    pub fn run(&mut self) {
        self.hide_cursor();
        self.display_directory();

        for character in self.stdin.lock().keys() {
            match character.unwrap() {
                Key::Char('q') => break,
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

    fn display_directory(&mut self) {
        write!(self.screen, "{}", termion::clear::All).unwrap();

        if let Ok(entries) = fs::read_dir(".") {
            for (entry_number, entry) in entries.enumerate() {
                write!(
                    self.screen,
                    "{}",
                    termion::cursor::Goto(1, (entry_number + 1).try_into().unwrap())
                )
                .unwrap();

                if let Ok(entry) = entry {
                    let file_name = entry.file_name();
                    let entry_name = file_name.to_string_lossy();
                    write!(self.screen, "{}", entry_name).unwrap();
                }
            }
        }

        self.screen.flush().unwrap();
    }
}

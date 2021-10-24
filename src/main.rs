extern crate termion;

use std::convert::TryInto;
use std::fs;
use std::io::{stdin, stdout, Write};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::*;

fn display_directory<W: Write>(screen: &mut W) {
    write!(screen, "{}", termion::clear::All).unwrap();

    if let Ok(entries) = fs::read_dir(".") {
        for (entry_number, entry) in entries.enumerate() {
            write!(
                screen,
                "{}",
                termion::cursor::Goto(1, (entry_number + 1).try_into().unwrap())
            )
            .unwrap();

            if let Ok(entry) = entry {
                let file_name = entry.file_name();
                let entry_name = file_name.to_string_lossy();
                write!(screen, "{}", entry_name).unwrap();
            }
        }
    }
}

fn main() {
    let stdin = stdin();
    let mut screen = AlternateScreen::from(stdout().into_raw_mode().unwrap());
    write!(screen, "{}", termion::cursor::Hide).unwrap();

    display_directory(&mut screen);

    screen.flush().unwrap();

    for c in stdin.keys() {
        match c.unwrap() {
            Key::Char('q') => break,
            _ => {}
        }
    }

    write!(screen, "{}", termion::cursor::Show).unwrap();
}

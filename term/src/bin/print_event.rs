use std::io::{self, Stdout, Write};

use ansi::{ControlFunction, Mode};
use term::{Key, KeyEvent, KeyMods, Term, TermEvent};

fn main() {
    let mut stdout: Stdout = io::stdout();

    let mut term = Term::new();
    term.save_attrs().unwrap();
    term.enable_raw().unwrap();

    // Ask the terminal to wrap pasted text so that it shows up as one event.
    write(&mut stdout, true);

    loop {
        let event: TermEvent = term.read().unwrap();
        print!("{:?}\r\n", event);
        if let TermEvent::KeyEvent(KeyEvent {
            key: Key::Char('c'),
            mods: KeyMods::CONTROL,
        }) = event
        {
            break;
        }
    }

    write(&mut stdout, false);

    term.restore_attrs().unwrap();
}

/// Turn bracketed paste mode on or off.
fn write(stdout: &mut Stdout, set: bool) {
    let function = ControlFunction::SetMode {
        modes: vec![Mode::BracketedPaste],
        set,
    };

    stdout.write_all(&Vec::from(&function)).unwrap();
    stdout.flush().unwrap();
}

use term::{Key, KeyEvent, KeyMods, Term, TermEvent};

fn main() {
    let mut term = Term::new();
    term.save_attrs().unwrap();
    term.enable_raw().unwrap();

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

    term.restore_attrs().unwrap();
}

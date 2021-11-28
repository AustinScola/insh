pub struct TerminalSize {
    pub width: u16,
    pub height: u16,
}

impl From<(u16, u16)> for TerminalSize {
    fn from(tuple: (u16, u16)) -> TerminalSize {
        TerminalSize {
            width: tuple.0,
            height: tuple.1,
        }
    }
}

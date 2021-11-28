use crossterm::style::Color as CrosstermColor;

pub enum Color {
    Highlight,
    InvertedText,
    BadRegex,
    NotCompiledRegex,
}

impl From<Color> for CrosstermColor {
    fn from(color: Color) -> CrosstermColor {
        match color {
            Color::Highlight => CrosstermColor::Yellow,
            Color::InvertedText => CrosstermColor::Black,
            Color::BadRegex => CrosstermColor::Red,
            Color::NotCompiledRegex => CrosstermColor::Grey,
        }
    }
}

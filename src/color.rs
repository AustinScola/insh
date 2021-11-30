use crossterm::style::Color as CrosstermColor;

const DARK_GREY: CrosstermColor = CrosstermColor::Rgb {
    r: 96,
    g: 96,
    b: 96,
};

pub enum Color {
    Highlight,
    InvertedText,
    InvertedBackground,
    BadRegex,
    NotCompiledRegex,
}

impl From<Color> for CrosstermColor {
    fn from(color: Color) -> CrosstermColor {
        match color {
            Color::Highlight => CrosstermColor::Yellow,
            Color::InvertedText => CrosstermColor::Black,
            Color::InvertedBackground => CrosstermColor::White,
            Color::BadRegex => CrosstermColor::Red,
            Color::NotCompiledRegex => DARK_GREY,
        }
    }
}

use crossterm::style::Color as CrosstermColor;

const DARK_GREY: CrosstermColor = CrosstermColor::Rgb {
    r: 96,
    g: 96,
    b: 96,
};

const LIGHT_GREY: CrosstermColor = CrosstermColor::Rgb {
    r: 159,
    g: 159,
    b: 159,
};

pub enum Color {
    Highlight,
    GrayedText,
    LightGrayedText,
    InvertedText,
    InvertedGrayedText,
    InvertedLightGrayedText,
    InvertedBackground,
    BadRegex,
    NotCompiledRegex,
}

impl From<Color> for CrosstermColor {
    fn from(color: Color) -> CrosstermColor {
        match color {
            Color::Highlight => CrosstermColor::Yellow,
            Color::GrayedText => DARK_GREY,
            Color::LightGrayedText => LIGHT_GREY,
            Color::InvertedText => CrosstermColor::Black,
            Color::InvertedGrayedText => LIGHT_GREY,
            Color::InvertedLightGrayedText => DARK_GREY,
            Color::InvertedBackground => CrosstermColor::White,
            Color::BadRegex => CrosstermColor::Red,
            Color::NotCompiledRegex => DARK_GREY,
        }
    }
}

impl Color {
    pub fn focus_or_important(focus: bool) -> Self {
        if focus {
            Self::Highlight
        } else {
            Self::InvertedBackground
        }
    }
}

use ansi::Color as AnsiColor;

const DARK_GREY: AnsiColor = AnsiColor::Rgb {
    red: 96,
    green: 96,
    blue: 96,
};

const LIGHT_GREY: AnsiColor = AnsiColor::Rgb {
    red: 159,
    green: 159,
    blue: 159,
};

pub enum Color {
    Highlight,
    GrayedText,
    LightGrayedText,
    InvertedText,
    InvertedGrayedText,
    InvertedLightGrayedText,
    InvertedBackground,
    FooterBackground,
    BadRegex,
    NotCompiledRegex,
}

impl From<Color> for AnsiColor {
    fn from(color: Color) -> AnsiColor {
        match color {
            Color::Highlight => AnsiColor::BrightYellow,
            Color::GrayedText => DARK_GREY,
            Color::LightGrayedText => LIGHT_GREY,
            Color::InvertedText => AnsiColor::Black,
            Color::InvertedGrayedText => LIGHT_GREY,
            Color::InvertedLightGrayedText => DARK_GREY,
            Color::InvertedBackground => AnsiColor::BrightWhite,
            Color::FooterBackground => AnsiColor::White,
            Color::BadRegex => AnsiColor::BrightRed,
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

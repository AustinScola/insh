//! Interface colors.

use ansi::Color as AnsiColor;

/// The dark gray for grayed out text.
const DARK_GRAY: AnsiColor = AnsiColor::Rgb {
    red: 96,
    green: 96,
    blue: 96,
};

/// The light gray for grayed out text.
const LIGHT_GRAY: AnsiColor = AnsiColor::Rgb {
    red: 159,
    green: 159,
    blue: 159,
};

/// A color.
pub enum Color {
    /// Something which stands out.
    Highlight,
    /// Less important text.
    GrayedText,
    /// Slightly less important text.
    LightGrayedText,
    /// Text on an inverted background.
    InvertedText,
    /// Less important text on an inverted background.
    InvertedGrayedText,
    /// Slightly less important text on an inverted background.
    InvertedLightGrayedText,
    /// An inverted background.
    InvertedBackground,
    /// The background of the footer.
    FooterBackground,
    /// An invalid regex.
    BadRegex,
    /// A regex not searched for yet.
    NotCompiledRegex,
}

impl From<Color> for AnsiColor {
    fn from(color: Color) -> AnsiColor {
        match color {
            Color::Highlight => AnsiColor::BrightYellow,
            Color::GrayedText => DARK_GRAY,
            Color::LightGrayedText => LIGHT_GRAY,
            Color::InvertedText => AnsiColor::Black,
            Color::InvertedGrayedText => LIGHT_GRAY,
            Color::InvertedLightGrayedText => DARK_GRAY,
            Color::InvertedBackground => AnsiColor::BrightWhite,
            Color::FooterBackground => AnsiColor::White,
            Color::BadRegex => AnsiColor::BrightRed,
            Color::NotCompiledRegex => DARK_GRAY,
        }
    }
}

impl Color {
    /// Return the color for something focused or important.
    pub fn focus_or_important(focus: bool) -> Self {
        if focus {
            Self::Highlight
        } else {
            Self::InvertedBackground
        }
    }
}

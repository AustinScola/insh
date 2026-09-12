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

/// The medium gray for a background.
const MEDIUM_BACKGROUND: AnsiColor = AnsiColor::Rgb {
    red: 168,
    green: 168,
    blue: 168,
};

/// A color.
pub enum Color {
    /// Something which stands out.
    Highlight,
    /// Something which stands out but is not focused.
    UnfocusedHighlight,
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
            Color::UnfocusedHighlight => MEDIUM_BACKGROUND,
            Color::GrayedText => DARK_GRAY,
            Color::LightGrayedText => LIGHT_GRAY,
            Color::InvertedText => AnsiColor::Black,
            Color::InvertedGrayedText => LIGHT_GRAY,
            Color::InvertedLightGrayedText => DARK_GRAY,
            Color::InvertedBackground => AnsiColor::BrightWhite,
            Color::FooterBackground => MEDIUM_BACKGROUND,
            Color::BadRegex => AnsiColor::BrightRed,
            Color::NotCompiledRegex => DARK_GRAY,
        }
    }
}

impl Color {
    /// Return the color to highlight a bar with, which is the inverted background rather than
    /// yellow when it is not focused so that only one thing on the screen is ever yellow.
    pub fn highlight(focus: bool) -> Self {
        if focus {
            Self::Highlight
        } else {
            Self::InvertedBackground
        }
    }

    /// Return the color to highlight a row of contents with. A bar which is not focused is left
    /// the color of the ones around it, but a row is grayed so that which one is selected can
    /// still be seen.
    pub fn row_highlight(focus: bool) -> Self {
        if focus {
            Self::Highlight
        } else {
            Self::UnfocusedHighlight
        }
    }
}

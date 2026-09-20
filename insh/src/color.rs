//! Interface colors.

use ansi::Color as AnsiColor;
use highlighter::Kind;

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
    /// The background of what you said in a chat, which is the gray a selection is left when it
    /// is not focused.
    SaidBackground,
    /// What code is written on.
    CodeBackground,
    /// A word which is part of a language, in code. Vim calls this `Statement`.
    Keyword,
    /// A value written out, in code. Vim calls this `Constant`.
    Constant,
    /// A note to whoever is reading, in code. Vim calls this `Comment`.
    Comment,
    /// The name of something, in code. Vim calls this `Identifier`.
    Identifier,
    /// The name of a type, in code. Vim calls this `Type`.
    TypeName,
    /// Something which is not the code itself, in code. Vim calls this `PreProc`.
    PreProc,
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
            Color::SaidBackground => MEDIUM_BACKGROUND,
            Color::CodeBackground => AnsiColor::Black,
            // These are the colors vim writes code in with a dark background, taken from vim
            // itself rather than from memory.
            Color::Keyword => AnsiColor::Yellow,
            Color::Constant => AnsiColor::Magenta,
            Color::Comment => AnsiColor::Cyan,
            Color::Identifier => AnsiColor::Cyan,
            Color::TypeName => AnsiColor::Green,
            Color::PreProc => AnsiColor::Blue,
            Color::BadRegex => AnsiColor::BrightRed,
            Color::NotCompiledRegex => DARK_GRAY,
        }
    }
}

impl Color {
    /// Return the color for a piece of code, which is nothing for the pieces vim leaves plain.
    ///
    /// These are the colors of the groups vim writes code in. Vim writes all of them in bold as
    /// well; code here is not written in bold, since a screen of it is easier to read without.
    pub fn of_code(kind: Option<Kind>) -> Option<Self> {
        return match kind? {
            // Statement.
            Kind::Keyword => Some(Self::Keyword),
            // Constant.
            Kind::String | Kind::Number | Kind::Constant => Some(Self::Constant),
            Kind::Comment => Some(Self::Comment),
            // Identifier.
            Kind::Function => Some(Self::Identifier),
            Kind::Type => Some(Self::TypeName),
            // PreProc.
            Kind::Attribute => Some(Self::PreProc),
            // A plain name is `Normal` in vim, which is neither colored nor bold. The grammars
            // here capture far more names than vim's own syntax files pick out, so coloring these
            // would leave whole files bold rather than the few words which are worth seeing.
            Kind::Variable | Kind::Operator | Kind::Punctuation => None,
        };
    }

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

//! Interface colors.

use ansi::Color as AnsiColor;
use highlighter::Kind;
use rend::Colors;

/// A color.
#[derive(Clone, Copy)]
pub enum Color {
    /// Something which stands out.
    Highlight,
    /// Something which stands out but is not focused.
    UnfocusedHighlight,
    /// What is written before something which stands out, which stands out a little less.
    LabelHighlight,
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
    /// The background of the footer along the bottom of a session.
    SessionFooterBackground,
    /// The background of the footer along the bottom of a session while the mouse is over it.
    SessionFooterHover,
    /// The line drawn between two panes.
    Border,
    /// The line drawn along the edge of the pane what is typed goes to.
    FocusedBorder,
    /// The line drawn between two panes while the mouse is over it.
    BorderHover,
    /// The line drawn along the edge of the pane what is typed goes to while the mouse is over
    /// it.
    FocusedBorderHover,
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
    /// The background of whatever the mouse is over which pressing on would go to, which is the
    /// gray a selection is left when it is not focused.
    Hover,
}

/// The colors insh is drawn in.
///
/// Every color is given exactly rather than as one of the terminal's own, so that insh looks the
/// same whatever the terminal is set up with. The ones which are only a darker version of another,
/// such as what is under the mouse, are worked out from it rather than given, so that they stay
/// that way whatever the other is.
///
/// NOTE: There is only the one theme for now, but everything which picks a color goes through
/// this, so that another could be used without anything else changing.
///
/// NOTE: What everything is drawn on and written in where nothing else is are left the terminal's
/// own, since the programs run in a pane are drawn in the terminal's colors and would not match.
pub struct Theme {
    /// Something which stands out.
    highlight: AnsiColor,
    /// Less important text.
    grayed_text: AnsiColor,
    /// Slightly less important text.
    light_grayed_text: AnsiColor,
    /// Text on an inverted background.
    inverted_text: AnsiColor,
    /// An inverted background.
    inverted_background: AnsiColor,
    /// The background of the footer along the bottom of a session.
    session_footer_background: AnsiColor,
    /// The background of the footer along the bottom of a session while the mouse is over it.
    session_footer_hover: AnsiColor,
    /// What code is written on.
    code_background: AnsiColor,
    /// The line drawn between two panes.
    border: AnsiColor,
    /// The line drawn between two panes while the mouse is over it.
    border_hover: AnsiColor,
    /// A word which is part of a language, in code.
    keyword: AnsiColor,
    /// A value written out, in code.
    constant: AnsiColor,
    /// A note to whoever is reading, and the name of something, in code.
    comment: AnsiColor,
    /// The name of a type, in code.
    type_name: AnsiColor,
    /// Something which is not the code itself, in code.
    pre_proc: AnsiColor,
    /// An invalid regex.
    bad_regex: AnsiColor,
}

impl Theme {
    /// The theme insh is drawn in.
    pub const DEFAULT: Self = Self {
        highlight: Self::rgb(255, 255, 0),
        grayed_text: Self::rgb(96, 96, 96),
        light_grayed_text: Self::rgb(159, 159, 159),
        inverted_text: Self::rgb(0, 0, 0),
        inverted_background: Self::rgb(255, 255, 255),
        // NOTE: The footer of a session is the terminal's own green, and its bright green while
        // the mouse is over it, rather than a green of insh's own.
        session_footer_background: AnsiColor::Green,
        session_footer_hover: AnsiColor::BrightGreen,
        code_background: Self::rgb(0, 0, 0),
        // NOTE: The lines between panes are gray, and white while the mouse is over them, so that
        // they stand back from what is in the panes until they are what is being pointed at.
        border: Self::rgb(128, 128, 128),
        border_hover: Self::rgb(229, 229, 229),
        // NOTE: These are the colors vim writes code in with a dark background, given as the
        // components xterm draws them in, apart from blue, which is lightened enough to be read
        // on the background.
        keyword: Self::rgb(205, 205, 0),
        constant: Self::rgb(205, 0, 205),
        comment: Self::rgb(0, 205, 205),
        type_name: Self::rgb(0, 205, 0),
        pre_proc: Self::rgb(92, 92, 255),
        bad_regex: Self::rgb(255, 0, 0),
    };

    /// How much of each of its components a darker version of a color keeps, out of a hundred.
    ///
    /// This is what takes white to the gray of a row which is not focused.
    const DARKER: u16 = 66;

    /// Return the color with the given components.
    const fn rgb(red: u8, green: u8, blue: u8) -> AnsiColor {
        return AnsiColor::Rgb { red, green, blue };
    }

    /// Return a darker version of the color, which is the color itself when it is not given by
    /// its components.
    fn darker(color: AnsiColor) -> AnsiColor {
        let darken = |amount: u8| (u16::from(amount) * Self::DARKER / 100) as u8;
        return match color {
            AnsiColor::Rgb { red, green, blue } => {
                Self::rgb(darken(red), darken(green), darken(blue))
            }
            color => color,
        };
    }

    /// Return the components of the color.
    pub fn of(&self, color: Color) -> AnsiColor {
        return match color {
            Color::Highlight => self.highlight,
            Color::UnfocusedHighlight => Self::darker(self.inverted_background),
            Color::LabelHighlight => Self::darker(self.highlight),
            Color::GrayedText => self.grayed_text,
            Color::LightGrayedText => self.light_grayed_text,
            Color::InvertedText => self.inverted_text,
            Color::InvertedGrayedText => self.light_grayed_text,
            Color::InvertedLightGrayedText => self.grayed_text,
            Color::InvertedBackground => self.inverted_background,
            Color::FooterBackground => Self::darker(self.inverted_background),
            Color::SessionFooterBackground => self.session_footer_background,
            Color::SessionFooterHover => self.session_footer_hover,
            Color::Border => self.border,
            // NOTE: The pane what is typed goes to is edged in the green of the footer, so that the
            // two of them are plainly the same session.
            Color::FocusedBorder => self.session_footer_background,
            Color::BorderHover => self.border_hover,
            Color::FocusedBorderHover => self.session_footer_hover,
            Color::SaidBackground => Self::darker(self.inverted_background),
            Color::CodeBackground => self.code_background,
            Color::Keyword => self.keyword,
            Color::Constant => self.constant,
            Color::Comment => self.comment,
            Color::Identifier => self.comment,
            Color::TypeName => self.type_name,
            Color::PreProc => self.pre_proc,
            Color::BadRegex => self.bad_regex,
            Color::NotCompiledRegex => self.grayed_text,
            Color::Hover => Self::darker(self.inverted_background),
        };
    }

    /// Return the colors a renderer draws in, for a terminal which can or cannot show a color
    /// given by its components as it is.
    pub fn colors(&self, true_color: bool) -> Colors {
        return Colors::builder().true_color(true_color).build();
    }
}

impl From<Color> for AnsiColor {
    fn from(color: Color) -> AnsiColor {
        return Theme::DEFAULT.of(color);
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

    /// Return the color to highlight what is written before a bar with, which is a darker one
    /// than the bar itself whether or not it is focused.
    pub fn label_highlight(focus: bool) -> Self {
        if focus {
            Self::LabelHighlight
        } else {
            Self::UnfocusedHighlight
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

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case(Color::Hover, Color::InvertedBackground; "what is under the mouse")]
    #[test_case(Color::UnfocusedHighlight, Color::InvertedBackground; "a row which is not focused")]
    #[test_case(Color::LabelHighlight, Color::Highlight; "what is written before a bar")]
    fn test_a_darker_color_is_darker_in_every_component(darker: Color, of: Color) {
        let components = |color: Color| match AnsiColor::from(color) {
            AnsiColor::Rgb { red, green, blue } => [red, green, blue],
            _ => panic!("Expected a color given by its components."),
        };

        let (darker, of) = (components(darker), components(of));

        assert!(darker
            .iter()
            .zip(of.iter())
            .all(|(darker, of)| darker <= of));
        assert_ne!(darker, of);
    }

    #[test]
    fn test_white_is_taken_to_the_gray_of_a_row_which_is_not_focused() {
        assert_eq!(
            AnsiColor::from(Color::UnfocusedHighlight),
            Theme::rgb(168, 168, 168)
        );
    }
}

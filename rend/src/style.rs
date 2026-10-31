/*!
This module contains the [`Style`] struct which is the way that text is written.
*/
use ansi::{Color, GraphicRendition, Underline};

use bitflags::bitflags;
use typed_builder::TypedBuilder;

bitflags! {
    /// The ways of styling text which are either on or off.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Attributes: u8 {
        /// Bold or bright.
        const BOLD = 0b0000_0001;
        /// Faint or dim.
        const FAINT = 0b0000_0010;
        /// Italic.
        const ITALIC = 0b0000_0100;
        /// Blinking.
        const BLINK = 0b0000_1000;
        /// The color the text is written in and the one it is written on swapped.
        const REVERSE = 0b0001_0000;
        /// Not shown at all.
        const CONCEAL = 0b0010_0000;
        /// Crossed out.
        const STRIKE = 0b0100_0000;
        /// Overlined.
        const OVERLINE = 0b1000_0000;
    }
}

/// The way that text is written: the colors it is written in and on, how it is underlined, and
/// which of the attributes are on. A color of `None` is whichever one the terminal uses when none
/// has been picked.
#[derive(TypedBuilder, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Style {
    /// The color the text is written in.
    #[builder(default)]
    color: Option<Color>,
    /// The color the text is written on.
    #[builder(default)]
    background: Option<Color>,
    /// The way the text is underlined, which is [`Underline::None`] when it is not underlined at
    /// all.
    #[builder(default = Underline::None)]
    underline: Underline,
    /// The color the text is underlined in.
    #[builder(default)]
    underline_color: Option<Color>,
    /// The attributes which are on.
    #[builder(default)]
    attributes: Attributes,
}

// NOTE: This is written out rather than derived because the way text is underlined by default is
// not underlined at all, and `Underline::default()` is a single line.
impl Default for Style {
    fn default() -> Self {
        Self {
            color: None,
            background: None,
            underline: Underline::None,
            underline_color: None,
            attributes: Attributes::empty(),
        }
    }
}

impl Style {
    /// Every attribute apart from bold and faint, along with the rendition which turns it on and
    /// the one which turns it off.
    ///
    /// Bold and faint are left out because they are the one pair which share the rendition which
    /// turns them off, so neither of them can be turned off on its own.
    const ATTRIBUTES: [(Attributes, GraphicRendition, GraphicRendition); 6] = [
        (
            Attributes::ITALIC,
            GraphicRendition::Italic,
            GraphicRendition::NotItalic,
        ),
        (
            Attributes::BLINK,
            GraphicRendition::SlowBlink,
            GraphicRendition::NotBlinking,
        ),
        (
            Attributes::REVERSE,
            GraphicRendition::Reverse,
            GraphicRendition::NotReversed,
        ),
        (
            Attributes::CONCEAL,
            GraphicRendition::Conceal,
            GraphicRendition::Reveal,
        ),
        (
            Attributes::STRIKE,
            GraphicRendition::Strike,
            GraphicRendition::NotStruck,
        ),
        (
            Attributes::OVERLINE,
            GraphicRendition::Overlined,
            GraphicRendition::NotOverlined,
        ),
    ];

    /// Bold and faint, which are the two attributes which the one rendition turns off.
    const INTENSITY: Attributes = Attributes::BOLD.union(Attributes::FAINT);

    /// Return the color the text is written in.
    pub fn color(&self) -> Option<Color> {
        return self.color;
    }

    /// Return the color the text is written on.
    pub fn background(&self) -> Option<Color> {
        return self.background;
    }

    /// Return the way the text is underlined.
    pub fn underline(&self) -> Underline {
        return self.underline;
    }

    /// Return the color the text is underlined in.
    pub fn underline_color(&self) -> Option<Color> {
        return self.underline_color;
    }

    /// Return the attributes which are on.
    pub fn attributes(&self) -> Attributes {
        return self.attributes;
    }

    /// Write the text in the given color.
    pub fn set_color(&mut self, color: Option<Color>) {
        self.color = color;
    }

    /// Write the text on the given color.
    pub fn set_background(&mut self, background: Option<Color>) {
        self.background = background;
    }

    /// Return whether the text is written in bold.
    pub fn bold(&self) -> bool {
        return self.attributes.contains(Attributes::BOLD);
    }

    /// Write the text in bold, or stop writing it in bold.
    pub fn set_bold(&mut self, bold: bool) {
        self.attributes.set(Attributes::BOLD, bold);
    }

    /// Return whether the text is written the way a terminal writes it when nothing has been
    /// picked.
    pub fn is_default(&self) -> bool {
        return *self == Self::default();
    }

    /// Apply the rendition, returning whether it is one which is known.
    ///
    /// A rendition which is not known leaves the style alone, since what it would do is not
    /// something which can be worked out.
    pub fn apply(&mut self, rendition: &GraphicRendition) -> bool {
        match rendition {
            GraphicRendition::Reset => *self = Self::default(),
            GraphicRendition::Bold => self.attributes.insert(Attributes::BOLD),
            GraphicRendition::Faint => self.attributes.insert(Attributes::FAINT),
            GraphicRendition::Italic => self.attributes.insert(Attributes::ITALIC),
            // NOTE: A single line is what `4` on its own means and no line at all is what `4:0`
            // means, so this covers being underlined and not being underlined both.
            GraphicRendition::Underline(underline) => self.underline = *underline,
            // NOTE: Blinking quickly is taken as blinking, which is what terminals make of it.
            GraphicRendition::SlowBlink | GraphicRendition::RapidBlink => {
                self.attributes.insert(Attributes::BLINK)
            }
            GraphicRendition::Reverse => self.attributes.insert(Attributes::REVERSE),
            GraphicRendition::Conceal => self.attributes.insert(Attributes::CONCEAL),
            GraphicRendition::Strike => self.attributes.insert(Attributes::STRIKE),
            GraphicRendition::Overlined => self.attributes.insert(Attributes::OVERLINE),
            GraphicRendition::NormalIntensity => self.attributes.remove(Self::INTENSITY),
            GraphicRendition::NotItalic => self.attributes.remove(Attributes::ITALIC),
            GraphicRendition::NotUnderlined => self.underline = Underline::None,
            GraphicRendition::NotBlinking => self.attributes.remove(Attributes::BLINK),
            GraphicRendition::NotReversed => self.attributes.remove(Attributes::REVERSE),
            GraphicRendition::Reveal => self.attributes.remove(Attributes::CONCEAL),
            GraphicRendition::NotStruck => self.attributes.remove(Attributes::STRIKE),
            GraphicRendition::NotOverlined => self.attributes.remove(Attributes::OVERLINE),
            GraphicRendition::Foreground(color) => self.color = Self::picked(color),
            GraphicRendition::Background(color) => self.background = Self::picked(color),
            GraphicRendition::UnderlineColor(color) => self.underline_color = Self::picked(color),
            _ => return false,
        }

        return true;
    }

    /// Return the color, which is none at all when it is the one the terminal uses when none has
    /// been picked.
    fn picked(color: &Color) -> Option<Color> {
        return match color {
            Color::Default => None,
            color => Some(*color),
        };
    }

    /// Return the renditions which put a terminal writing in the given style to writing in this
    /// one, which are none at all when they are the same.
    ///
    /// All of it is said from a reset when what the terminal is writing in is not known, since
    /// saying so is the only way of being sure of any of it.
    pub fn renditions_from(&self, style: Option<Style>) -> Vec<GraphicRendition> {
        let from: Style = match style {
            Some(from) => from,
            None => {
                let mut renditions: Vec<GraphicRendition> = vec![GraphicRendition::Reset];
                renditions.append(&mut self.renditions());
                return renditions;
            }
        };

        if from == *self {
            return Vec::new();
        }

        let mut renditions: Vec<GraphicRendition> = Vec::new();

        self.restyle_intensity(from, &mut renditions);

        for (attribute, on, off) in Self::ATTRIBUTES {
            let is_on: bool = self.attributes.contains(attribute);
            if is_on == from.attributes.contains(attribute) {
                continue;
            }
            renditions.push(match is_on {
                true => on,
                false => off,
            });
        }

        if self.underline != from.underline {
            renditions.push(match self.underline {
                Underline::None => GraphicRendition::NotUnderlined,
                underline => GraphicRendition::Underline(underline),
            });
        }
        if self.underline_color != from.underline_color {
            renditions.push(GraphicRendition::UnderlineColor(
                self.underline_color.unwrap_or(Color::Default),
            ));
        }

        // The two colors go in the one sequence, which is shorter than one sequence each.
        if self.color != from.color {
            renditions.push(GraphicRendition::Foreground(
                self.color.unwrap_or(Color::Default),
            ));
        }
        if self.background != from.background {
            renditions.push(GraphicRendition::Background(
                self.background.unwrap_or(Color::Default),
            ));
        }

        return renditions;
    }

    /// Write the renditions which put a terminal writing as boldly as the given style says onto
    /// the given renditions.
    ///
    /// Turning either bold or faint off turns both of them off, so whichever of them is still on
    /// is put back on afterwards.
    fn restyle_intensity(&self, from: Style, renditions: &mut Vec<GraphicRendition>) {
        let intensity: Attributes = self.attributes.intersection(Self::INTENSITY);
        let was: Attributes = from.attributes.intersection(Self::INTENSITY);

        if intensity == was {
            return;
        }

        if !was.difference(intensity).is_empty() {
            renditions.push(GraphicRendition::NormalIntensity);
            if intensity.contains(Attributes::BOLD) {
                renditions.push(GraphicRendition::Bold);
            }
            if intensity.contains(Attributes::FAINT) {
                renditions.push(GraphicRendition::Faint);
            }
            return;
        }

        if intensity.difference(was).contains(Attributes::BOLD) {
            renditions.push(GraphicRendition::Bold);
        }
        if intensity.difference(was).contains(Attributes::FAINT) {
            renditions.push(GraphicRendition::Faint);
        }
    }

    /// Return the renditions which put a terminal writing the way it does when nothing has been
    /// picked to writing in this style.
    fn renditions(&self) -> Vec<GraphicRendition> {
        let mut renditions: Vec<GraphicRendition> = Vec::new();

        if self.attributes.contains(Attributes::BOLD) {
            renditions.push(GraphicRendition::Bold);
        }
        if self.attributes.contains(Attributes::FAINT) {
            renditions.push(GraphicRendition::Faint);
        }
        for (attribute, on, _off) in Self::ATTRIBUTES {
            if self.attributes.contains(attribute) {
                renditions.push(on);
            }
        }

        if self.underline != Underline::None {
            renditions.push(GraphicRendition::Underline(self.underline));
        }
        if let Some(color) = self.underline_color {
            renditions.push(GraphicRendition::UnderlineColor(color));
        }
        if let Some(color) = self.color {
            renditions.push(GraphicRendition::Foreground(color));
        }
        if let Some(background) = self.background {
            renditions.push(GraphicRendition::Background(background));
        }

        return renditions;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    /// The color used for testing.
    const COLOR: Color = Color::Red;

    /// The background color used for testing.
    const BACKGROUND: Color = Color::Blue;

    /// Return the style with the given colors.
    fn style(color: Option<Color>, background: Option<Color>) -> Style {
        Style::builder().color(color).background(background).build()
    }

    /// Return the style with the given attributes on.
    fn attributed(attributes: Attributes) -> Style {
        Style::builder().attributes(attributes).build()
    }

    #[test_case(None, Style::default(), vec![GraphicRendition::Reset]; "a reset when what the terminal is writing in is not known")]
    #[test_case(None, style(Some(COLOR), None), vec![GraphicRendition::Reset, GraphicRendition::Foreground(COLOR)]; "a reset and all of the style when it is not known")]
    #[test_case(Some(Style::default()), Style::default(), vec![]; "nothing at all when they are the same")]
    #[test_case(Some(Style::default()), style(Some(COLOR), None), vec![GraphicRendition::Foreground(COLOR)]; "only the color which changed")]
    #[test_case(Some(style(Some(COLOR), None)), Style::default(), vec![GraphicRendition::Foreground(Color::Default)]; "a color going back to the default")]
    #[test_case(Some(Style::default()), style(Some(COLOR), Some(BACKGROUND)), vec![GraphicRendition::Foreground(COLOR), GraphicRendition::Background(BACKGROUND)]; "both colors in the one sequence")]
    #[test_case(Some(Style::default()), attributed(Attributes::BOLD), vec![GraphicRendition::Bold]; "an attribute going on")]
    #[test_case(Some(attributed(Attributes::BOLD)), Style::default(), vec![GraphicRendition::NormalIntensity]; "bold going off")]
    #[test_case(Some(attributed(Attributes::STRIKE)), Style::default(), vec![GraphicRendition::NotStruck]; "an attribute going off on its own")]
    #[test_case(Some(attributed(Attributes::BOLD | Attributes::FAINT)), attributed(Attributes::FAINT), vec![GraphicRendition::NormalIntensity, GraphicRendition::Faint]; "faint put back when bold goes off")]
    #[test_case(Some(attributed(Attributes::BOLD)), attributed(Attributes::BOLD | Attributes::ITALIC), vec![GraphicRendition::Italic]; "only the attribute which changed")]
    fn test_renditions_from(
        from: Option<Style>,
        style: Style,
        expected_renditions: Vec<GraphicRendition>,
    ) {
        assert_eq!(style.renditions_from(from), expected_renditions);
    }

    #[test_case(GraphicRendition::Bold, attributed(Attributes::BOLD); "bold")]
    #[test_case(GraphicRendition::Reverse, attributed(Attributes::REVERSE); "reverse")]
    #[test_case(GraphicRendition::RapidBlink, attributed(Attributes::BLINK); "blinking quickly is taken as blinking")]
    #[test_case(GraphicRendition::Foreground(COLOR), style(Some(COLOR), None); "a color")]
    #[test_case(GraphicRendition::Foreground(Color::Default), Style::default(); "a color which is the default is no color of its own")]
    #[test_case(GraphicRendition::Underline(Underline::Curly), Style::builder().underline(Underline::Curly).build(); "an underline")]
    #[test_case(GraphicRendition::Underline(Underline::None), Style::default(); "an underline turned off by its own sub-parameter")]
    fn test_apply(rendition: GraphicRendition, expected_style: Style) {
        let mut style = Style::default();

        assert_eq!(style.apply(&rendition), true);
        assert_eq!(style, expected_style);
    }

    #[test]
    fn test_a_rendition_which_is_not_known_leaves_the_style_alone() {
        let mut style = attributed(Attributes::BOLD);

        assert_eq!(style.apply(&GraphicRendition::Fraktur), false);
        assert_eq!(style, attributed(Attributes::BOLD));
    }

    #[test]
    fn test_a_reset_puts_everything_back() {
        let mut style = Style::builder()
            .color(Some(COLOR))
            .background(Some(BACKGROUND))
            .underline(Underline::Double)
            .attributes(Attributes::BOLD | Attributes::ITALIC)
            .build();

        style.apply(&GraphicRendition::Reset);

        assert_eq!(style, Style::default());
        assert_eq!(style.is_default(), true);
    }

    #[test]
    fn test_the_renditions_a_style_is_said_in_put_it_back_together() {
        let style = Style::builder()
            .color(Some(COLOR))
            .background(Some(BACKGROUND))
            .underline(Underline::Curly)
            .underline_color(Some(COLOR))
            .attributes(Attributes::BOLD | Attributes::ITALIC | Attributes::STRIKE)
            .build();

        let mut rebuilt = Style::default();
        for rendition in style.renditions_from(None) {
            assert_eq!(rebuilt.apply(&rendition), true);
        }

        assert_eq!(rebuilt, style);
    }
}

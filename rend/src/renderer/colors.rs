/*!
This module contains the [`Colors`] struct, which is the colors a [`Renderer`](super::Renderer)
draws in.
*/
use crate::style::Style;

use ansi::Color;

use typed_builder::TypedBuilder;

/// The colors a renderer draws in.
///
/// What a fabric leaves to the terminal is drawn in the colors given here instead, so that what is
/// drawn looks the same whatever the terminal is set up with. A color given by its components is
/// drawn as the nearest of the 256 in the palette on a terminal which cannot show it as it is.
#[derive(Debug, Clone, Copy, TypedBuilder)]
pub struct Colors {
    /// The color text is written in where none was picked, which is the terminal's own when there
    /// is none.
    #[builder(default)]
    foreground: Option<Color>,
    /// The color text is written on where none was picked, which is the terminal's own when there
    /// is none.
    #[builder(default)]
    background: Option<Color>,
    /// Whether the terminal can show a color given by its components as it is.
    #[builder(default = true)]
    true_color: bool,
}

impl Default for Colors {
    fn default() -> Self {
        return Self::builder().build();
    }
}

impl Colors {
    /// The amounts of a component which the colors of the cube in the palette are made of.
    const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

    /// The entry of the palette which the cube of colors starts at.
    const CUBE_START: u8 = 16;

    /// The entry of the palette which the grays start at.
    const GRAYS_START: u8 = 232;

    /// How many grays there are in the palette.
    const GRAYS: u8 = 24;

    /// Return whether every style is drawn the way it is, which is when nothing is put in place
    /// of what is left to the terminal and the terminal can show any color.
    pub fn changes_nothing(&self) -> bool {
        return self.foreground.is_none() && self.background.is_none() && self.true_color;
    }

    /// Return the style the way it is drawn.
    pub fn apply(&self, mut style: Style) -> Style {
        if style.color().is_none() {
            style.set_color(self.foreground);
        }
        if style.background().is_none() {
            style.set_background(self.background);
        }

        if !self.true_color {
            style.set_color(style.color().map(Self::nearest));
            style.set_background(style.background().map(Self::nearest));
            style.set_underline_color(style.underline_color().map(Self::nearest));
        }

        return style;
    }

    /// Return the nearest of the 256 colors of the palette to the color, which is the color
    /// itself when it is not given by its components.
    ///
    /// NOTE: Only the cube and the grays are looked through, and not the sixteen colors at the
    /// start of the palette, since those are whatever the terminal is set up with and the rest are
    /// the same everywhere.
    fn nearest(color: Color) -> Color {
        let (red, green, blue): (u8, u8, u8) = match color {
            Color::Rgb { red, green, blue } => (red, green, blue),
            color => return color,
        };

        let (red_at, green_at, blue_at) = (
            Self::nearest_in_cube(red),
            Self::nearest_in_cube(green),
            Self::nearest_in_cube(blue),
        );
        let cube: (u8, u8, u8) = (
            Self::CUBE[red_at],
            Self::CUBE[green_at],
            Self::CUBE[blue_at],
        );
        let cube_index: u8 = Self::CUBE_START + (36 * red_at + 6 * green_at + blue_at) as u8;

        // The grays go from 8 to 238 in steps of 10.
        let average: u16 = (u16::from(red) + u16::from(green) + u16::from(blue)) / 3;
        let step: u8 = (average.saturating_sub(3) / 10).min(u16::from(Self::GRAYS - 1)) as u8;
        let level: u8 = 8 + 10 * step;
        let gray: (u8, u8, u8) = (level, level, level);

        let index: u8 = match Self::distance((red, green, blue), gray)
            < Self::distance((red, green, blue), cube)
        {
            true => Self::GRAYS_START + step,
            false => cube_index,
        };

        return Color::Indexed(index);
    }

    /// Return which of the amounts of the cube is nearest to the amount.
    fn nearest_in_cube(amount: u8) -> usize {
        return Self::CUBE
            .iter()
            .enumerate()
            .min_by_key(|(_, level)| (i16::from(**level) - i16::from(amount)).unsigned_abs())
            .map_or(0, |(at, _)| at);
    }

    /// Return how far apart the two colors are, squared.
    fn distance(one: (u8, u8, u8), other: (u8, u8, u8)) -> u32 {
        let apart = |one: u8, other: u8| (i32::from(one) - i32::from(other)).pow(2) as u32;
        return apart(one.0, other.0) + apart(one.1, other.1) + apart(one.2, other.2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case(Color::Rgb { red: 0, green: 0, blue: 0 }, Color::Indexed(16); "black, which is the corner of the cube")]
    #[test_case(Color::Rgb { red: 255, green: 255, blue: 0 }, Color::Indexed(226); "yellow")]
    #[test_case(Color::Rgb { red: 0, green: 135, blue: 0 }, Color::Indexed(28); "a green which is in the cube")]
    #[test_case(Color::Rgb { red: 168, green: 168, blue: 168 }, Color::Indexed(248); "a gray, which is nearer one of the grays")]
    #[test_case(Color::Rgb { red: 30, green: 30, blue: 30 }, Color::Indexed(234); "a dark gray")]
    #[test_case(Color::Green, Color::Green; "one of the sixteen, which is left alone")]
    fn test_nearest(color: Color, expected: Color) {
        assert_eq!(Colors::nearest(color), expected);
    }

    #[test]
    fn test_only_the_default_colors_change_nothing() {
        assert!(Colors::default().changes_nothing());
        assert!(!Colors::builder()
            .true_color(false)
            .build()
            .changes_nothing());
        assert!(!Colors::builder()
            .background(Some(Color::Black))
            .build()
            .changes_nothing());
    }

    #[test]
    fn test_what_is_left_to_the_terminal_is_drawn_in_the_colors_given() {
        let foreground = Color::Rgb {
            red: 200,
            green: 200,
            blue: 200,
        };
        let background = Color::Rgb {
            red: 30,
            green: 30,
            blue: 30,
        };
        let colors = Colors::builder()
            .foreground(Some(foreground))
            .background(Some(background))
            .build();

        let style: Style = colors.apply(Style::builder().color(Some(Color::Red)).build());

        assert_eq!(style.color(), Some(Color::Red));
        assert_eq!(style.background(), Some(background));
    }

    #[test]
    fn test_a_terminal_without_true_color_is_given_the_nearest_of_the_palette() {
        let colors = Colors::builder()
            .background(Some(Color::Rgb {
                red: 30,
                green: 30,
                blue: 30,
            }))
            .true_color(false)
            .build();

        let style: Style = colors.apply(Style::default());

        assert_eq!(style.background(), Some(Color::Indexed(234)));
    }
}

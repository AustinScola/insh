/*!
This module contains the [`Style`] struct which is the colors that text is written in and on.
*/
use ansi::{Color, GraphicRendition};

use typed_builder::TypedBuilder;

/// The colors which text is written in and on, where `None` is whichever color the terminal uses
/// when none has been picked.
#[derive(TypedBuilder, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// The color the text is written in.
    #[builder(default)]
    color: Option<Color>,
    /// The color the text is written on.
    #[builder(default)]
    background: Option<Color>,
}

impl Style {
    /// Return the color the text is written in.
    pub fn color(&self) -> Option<Color> {
        return self.color;
    }

    /// Return the color the text is written on.
    pub fn background(&self) -> Option<Color> {
        return self.background;
    }

    /// Return whether the text is written in the colors which the terminal uses when none have
    /// been picked.
    pub fn is_default(&self) -> bool {
        return self.color.is_none() && self.background.is_none();
    }

    /// Return the renditions which put a terminal writing in the given style to writing in this
    /// one, which are none at all when they are the same.
    ///
    /// Both colors are given when what the terminal is writing in is not known, since saying so is
    /// the only way of being sure of either of them.
    pub fn renditions_from(&self, style: Option<Style>) -> Vec<GraphicRendition> {
        // The two colors go in the one sequence, which is shorter than one sequence each.
        let mut renditions: Vec<GraphicRendition> = Vec::with_capacity(2);

        if style.map(|style| style.color) != Some(self.color) {
            renditions.push(GraphicRendition::Foreground(
                self.color.unwrap_or(Color::Default),
            ));
        }
        if style.map(|style| style.background) != Some(self.background) {
            renditions.push(GraphicRendition::Background(
                self.background.unwrap_or(Color::Default),
            ));
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

    #[test_case(None, Style::default(), vec![GraphicRendition::Foreground(Color::Default), GraphicRendition::Background(Color::Default)]; "both colors when what the terminal is writing in is not known")]
    #[test_case(Some(Style::default()), Style::default(), vec![]; "nothing at all when they are the same")]
    #[test_case(Some(Style::default()), style(Some(COLOR), None), vec![GraphicRendition::Foreground(COLOR)]; "only the color which changed")]
    #[test_case(Some(style(Some(COLOR), None)), Style::default(), vec![GraphicRendition::Foreground(Color::Default)]; "a color going back to the default")]
    #[test_case(Some(Style::default()), style(Some(COLOR), Some(BACKGROUND)), vec![GraphicRendition::Foreground(COLOR), GraphicRendition::Background(BACKGROUND)]; "both colors in the one sequence")]
    fn test_renditions_from(
        from: Option<Style>,
        style: Style,
        expected_renditions: Vec<GraphicRendition>,
    ) {
        assert_eq!(style.renditions_from(from), expected_renditions);
    }
}

/*!
This module contains the [`Yarn`] struct which is used for representing styled text.
*/
use std::cmp::Ordering;
use std::ops::Range;

use super::cell::Cell;
use super::style::Style;

use ansi::Color;

// MAYBE TODO: Store ranges instead of using `Vec` to save memory?
/// A yarn is a string along with the way each column of it is written.
///
/// It is measured in columns of a terminal rather than in characters, which are not the same thing:
/// a letter with a combining accent on it is several characters in the one column, and a character
/// of an East Asian script is two columns. There is one [`Cell`] per column either way.
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Yarn {
    // MAYBE TODO: Store the length separately so we can represent a blank yarn without wasting mem?
    /// The cells, one for each column.
    cells: Vec<Cell>,
    // NOTE: The styles are allowed to be given for fewer columns than there are cells. A column
    // off the end of them is written the way a terminal writes when nothing has been picked.
    /// The way each column is written.
    styles: Vec<Style>,
}

impl Yarn {
    /// Return a new yarn of zero length.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Return a yarn consisting of unstylized spaces of the given length.
    pub fn blank(len: usize) -> Self {
        Self {
            cells: vec![Cell::BLANK; len],
            ..Default::default()
        }
    }

    /// How many columns the dots standing for the cut off part take up.
    const ELLIPSIS_LEN: usize = 3;

    /// Return a yarn with string centered and truncated with dots if the string is longer than the
    /// the length.
    pub fn center(string: &str, len: usize) -> Self {
        if len == 0 {
            return Yarn::default();
        }

        let mut cells: Vec<Cell> = Cell::all(string);

        match cells.len().cmp(&len) {
            Ordering::Greater => {
                if len <= Self::ELLIPSIS_LEN {
                    return Yarn::from(vec![Cell::Char('.'); len]);
                }

                Cell::truncate(&mut cells, len - Self::ELLIPSIS_LEN);
                cells.resize(len, Cell::Char('.'));

                return Yarn::from(cells);
            }
            Ordering::Less => {
                let before_len: usize = (len - cells.len()) / 2;

                let mut centered: Vec<Cell> = vec![Cell::BLANK; before_len];
                centered.append(&mut cells);
                centered.resize(len, Cell::BLANK);

                return Yarn::from(centered);
            }
            Ordering::Equal => {
                return Yarn::from(cells);
            }
        };
    }

    /// Return how many columns the yarn takes up.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Return whether the yarn is empty.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Add the other yarn to the end of this one and return the new yarn.
    // Should this be called `extend`? The `std::iter::Extend` trait doesn't return `Self`?
    #[allow(dead_code)]
    pub fn concat(mut self, other: Self) -> Self {
        let len_before: usize = self.len();
        self.cells.extend(other.cells);

        if !other.styles.is_empty() {
            self.styles.resize(len_before, Style::default());
            self.styles.extend(other.styles);
        }

        self
    }

    /// Change the length of the yarn to the `new_size`.
    pub fn resize(&mut self, new_len: usize) {
        let len = self.len();
        match len.cmp(&new_len) {
            Ordering::Greater => {
                self.truncate(new_len);
            }
            Ordering::Less => {
                self.cells.resize(new_len, Cell::BLANK);
            }
            Ordering::Equal => {}
        }
    }

    /// Shortens the yarn to the given length.
    ///
    /// If the yarn is already shorter than the `new_len` then this has no effect.
    pub fn truncate(&mut self, new_len: usize) {
        Cell::truncate(&mut self.cells, new_len);
        self.styles.truncate(new_len);
    }

    /// Pad on both sides so that the contents are centered and the length is equal `new_len`.
    ///
    /// If the `new_len` is less than the current length, then panic (for now).
    #[allow(dead_code)]
    pub fn pad(&mut self, new_len: usize) {
        let len = self.len();
        match new_len.cmp(&len) {
            Ordering::Greater => {
                let difference = new_len - len;
                let left_pad = difference / 2;
                let right_pad = difference - left_pad;
                self.cells = [
                    vec![Cell::BLANK; left_pad],
                    self.cells.to_owned(),
                    vec![Cell::BLANK; right_pad],
                ]
                .concat();
                self.styles = [vec![Style::default(); left_pad], self.styles.to_owned()].concat();
            }
            Ordering::Less => {
                panic!("Cannot pad a yarn to a smaller length.")
            }
            Ordering::Equal => {}
        }
    }

    /// Set the text color of the entire yarn to the `color`.
    pub fn color(&mut self, color: Color) {
        self.restyle(0..self.len(), |style| style.set_color(Some(color)));
    }

    /// Change the color of all text before the given position.
    pub fn color_before(&mut self, color: Color, position: usize) {
        self.restyle(0..position.min(self.len()), |style| {
            style.set_color(Some(color))
        });
    }

    /// Change the color of all text after (and including) the given position.
    ///
    /// The text before the position keeps the color it already has (if it has one).
    pub fn color_after(&mut self, color: Color, position: usize) {
        let len: usize = self.len();
        self.restyle(position.min(len)..len, |style| style.set_color(Some(color)));
    }

    /// Set the background color of the entire yarn to the `color`.
    pub fn background(&mut self, color: Color) {
        self.restyle(0..self.len(), |style| style.set_background(Some(color)));
    }

    /// Write the whole of the yarn in bold.
    pub fn bold(&mut self) {
        self.restyle(0..self.len(), |style| style.set_bold(true));
    }

    /// Write every column of the range the given way.
    pub fn style(&mut self, style: Style, columns: Range<usize>) {
        let len: usize = self.len();
        self.restyle(columns.start.min(len)..columns.end.min(len), |written| {
            *written = style
        });
    }

    /// Change the way every column of the range is written.
    ///
    /// The styles are only given for as many columns as have been styled, so they are filled out
    /// as far as the range before any of them is changed.
    fn restyle(&mut self, columns: Range<usize>, restyle: impl Fn(&mut Style)) {
        if columns.is_empty() {
            return;
        }

        if self.styles.len() < columns.end {
            self.styles.resize(columns.end, Style::default());
        }

        for style in &mut self.styles[columns] {
            restyle(style);
        }
    }

    /// Return the cells of the yarn, one for each column.
    pub fn cells(&self) -> &Vec<Cell> {
        &self.cells
    }

    /// Return the way each column of the yarn is written, which may be given for fewer columns
    /// than there are cells.
    pub fn styles(&self) -> &Vec<Style> {
        &self.styles
    }
}

impl From<String> for Yarn {
    fn from(string: String) -> Self {
        Self::from(string.as_str())
    }
}

impl From<&str> for Yarn {
    fn from(string: &str) -> Self {
        Yarn {
            cells: Cell::all(string),
            ..Default::default()
        }
    }
}

impl From<Vec<Cell>> for Yarn {
    fn from(cells: Vec<Cell>) -> Self {
        Yarn {
            cells,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    /// The color used for testing.
    const COLOR: Color = Color::Red;

    /// Another color used for testing.
    const OTHER_COLOR: Color = Color::Blue;

    /// Return the color each column of the yarn is written in.
    fn colors(yarn: &Yarn) -> Vec<Option<Color>> {
        return (0..yarn.len())
            .map(|column| {
                yarn.styles()
                    .get(column)
                    .copied()
                    .unwrap_or_default()
                    .color()
            })
            .collect();
    }

    /// Return a yarn with the text before the position colored the other color.
    fn colored_before(string: &str, position: usize) -> Yarn {
        let mut yarn: Yarn = Yarn::from(string);
        yarn.color_before(OTHER_COLOR, position);
        yarn
    }

    /// Return a yarn of the text with all of it colored.
    fn colored(string: &str, color: Color) -> Yarn {
        let mut yarn: Yarn = Yarn::from(string);
        yarn.color(color);
        yarn
    }

    #[test_case(Yarn::from("foobar"), 3, vec![None, None, None, Some(COLOR), Some(COLOR), Some(COLOR)]; "text which is not colored")]
    #[test_case(colored_before("foobar", 3), 3, vec![Some(OTHER_COLOR), Some(OTHER_COLOR), Some(OTHER_COLOR), Some(COLOR), Some(COLOR), Some(COLOR)]; "text which is colored before the position")]
    #[test_case(colored_before("foobar", 6), 3, vec![Some(OTHER_COLOR), Some(OTHER_COLOR), Some(OTHER_COLOR), Some(COLOR), Some(COLOR), Some(COLOR)]; "text which is colored after the position")]
    #[test_case(Yarn::from("foobar"), 0, vec![Some(COLOR); 6]; "the position of the first character")]
    #[test_case(Yarn::from("foobar"), 6, vec![None; 6]; "the position after the last character")]
    #[test_case(Yarn::from("foobar"), 9, vec![None; 6]; "a position past the end of the text")]
    #[test_case(Yarn::new(), 0, vec![]; "an empty yarn")]
    fn test_color_after(mut yarn: Yarn, position: usize, expected_colors: Vec<Option<Color>>) {
        yarn.color_after(COLOR, position);

        assert_eq!(colors(&yarn), expected_colors);
    }

    #[test]
    fn test_coloring_the_text_and_the_background_keeps_both() {
        let mut yarn = colored("ab", COLOR);
        yarn.background(OTHER_COLOR);

        for column in 0..yarn.len() {
            let style: Style = yarn.styles()[column];
            assert_eq!(style.color(), Some(COLOR));
            assert_eq!(style.background(), Some(OTHER_COLOR));
        }
    }

    #[test_case("", 0, Yarn::new(); "an empty string and no length")]
    #[test_case("", 3, Yarn::from("   "); "an empty string and some length")]
    #[test_case("foo", 3, Yarn::from("foo"); "a string that just fits")]
    #[test_case("foo", 5, Yarn::from(" foo "); "a string is centered")]
    #[test_case("foo", 6, Yarn::from(" foo  "); "a string is centered and breaks ties leftwards")]
    #[test_case("foobar", 5, Yarn::from("fo..."); "a string is truncated with dots")]
    #[test_case("foobar", 2, Yarn::from(".."); "dot truncation can handle lengths less than 3")]
    fn test_center(string: &str, len: usize, expected_result: Yarn) {
        let result: Yarn = Yarn::center(string, len);

        assert_eq!(result, expected_result);
    }

    /// Return a yarn of one blank column written the given way.
    fn spot(style: Option<Style>) -> Yarn {
        Yarn {
            cells: vec![Cell::BLANK; 1],
            styles: style.into_iter().collect(),
        }
    }

    #[test_case(Yarn::new(), Yarn::new(), Yarn::new(); "an empty yarn with an empty yarn is an empty yarn")]
    #[test_case(Yarn::new(), spot(None), spot(None); "an empty yarn with a one space yarn is a one space yarn")]
    #[test_case(spot(None), Yarn::new(), spot(None); "a one space yarn with an empty yarn is a one space yarn")]
    #[test_case(spot(None), spot(Some(Style::builder().color(Some(Color::Black)).build())), Yarn {cells: vec![Cell::BLANK; 2], styles: vec![Style::default(), Style::builder().color(Some(Color::Black)).build()]}; "concatenating two one space yarns preserves how they are written")]
    fn test_concat(yarn: Yarn, other: Yarn, expected_yarn: Yarn) {
        let result: Yarn = yarn.concat(other);

        assert_eq!(result, expected_yarn);
    }

    #[test_case("abc", 3; "plain characters")]
    #[test_case("e\u{301}", 1; "a character with a combining accent")]
    #[test_case("🦀😀", 4; "two wide characters")]
    #[test_case("a🦀b", 4; "a wide character in among narrow ones")]
    fn test_a_yarn_is_as_long_as_the_columns_it_takes_up(string: &str, len: usize) {
        assert_eq!(Yarn::from(string).len(), len);
    }

    #[test]
    fn test_resizing_a_yarn_which_ends_in_a_wide_character_does_not_overrun() {
        // Cutting between the two halves of the wide character has to leave a blank behind, or the
        // whole of it would be written in the one column which is left and push the row along.
        let mut yarn = Yarn::from("a🦀b");
        yarn.resize(2);

        assert_eq!(yarn.len(), 2);
        assert_eq!(yarn.cells(), &vec![Cell::Char('a'), Cell::BLANK]);
    }

    #[test]
    fn test_a_yarn_is_resized_to_the_columns_asked_for_whatever_is_in_it() {
        for string in ["abc", "🦀😀", "a🦀b", "e\u{301}x"] {
            let mut yarn = Yarn::from(string);
            yarn.resize(10);

            assert_eq!(yarn.len(), 10, "resizing {:?}", string);
        }
    }
}

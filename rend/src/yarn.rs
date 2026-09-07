/*!
This module contains the [`Yarn`] struct which is used for representing styled text.
*/
use std::cmp::Ordering;

use super::cell::Cell;

use ansi::Color;

// MAYBE TODO: Store ranges instead of using `Vec` to save memory?
/// A yarn is a string with text colors and background colors.
///
/// It is measured in columns of a terminal rather than in characters, which are not the same thing:
/// a letter with a combining accent on it is several characters in the one column, and a character
/// of an East Asian script is two columns. There is one [`Cell`] per column either way.
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Yarn {
    // MAYBE TODO: Store the length separately so we can represent a blank yarn without wasting mem?
    /// The cells, one for each column.
    cells: Vec<Cell>,
    // NOTE: The style vectors are Allowed to be shorter than the number of characters.
    /// The colors of the text.
    colors: Vec<Option<Color>>,
    /// The background colors of the text.
    backgrounds: Vec<Option<Color>>,
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

        if !other.colors.is_empty() {
            self.colors.resize(len_before, None);
            self.colors.extend(other.colors);
        }

        if !other.backgrounds.is_empty() {
            self.backgrounds.resize(len_before, None);
            self.backgrounds.extend(other.backgrounds);
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
        self.colors.truncate(new_len);
        self.backgrounds.truncate(new_len);
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
                self.colors = [vec![None; left_pad], self.colors.to_owned()].concat();
                self.backgrounds = [vec![None; left_pad], self.backgrounds.to_owned()].concat();
            }
            Ordering::Less => {
                panic!("Cannot pad a yarn to a smaller length.")
            }
            Ordering::Equal => {}
        }
    }

    /// Set the text color of the entire yarn to the `color`.
    pub fn color(&mut self, color: Color) {
        self.colors = vec![Some(color); self.len()];
    }

    /// Change the color of all text before the given position.
    pub fn color_before(&mut self, color: Color, position: usize) {
        if self.colors.len() < position {
            self.colors.clear();
            self.colors.resize(position, Some(color));
        } else {
            for index in 0..position {
                self.colors[index] = Some(color);
            }
        }
    }

    /// Change the color of all text after (and including) the given position.
    ///
    /// The text before the position keeps the color it already has (if it has one).
    pub fn color_after(&mut self, color: Color, position: usize) {
        let num_chars: usize = self.cells.len();

        if self.colors.len() < num_chars {
            self.colors.resize(num_chars, None);
        }

        for index in position..num_chars {
            self.colors[index] = Some(color);
        }
    }

    /// Set the background color of the entire yarn to the `color`.
    pub fn background(&mut self, color: Color) {
        self.backgrounds = vec![Some(color); self.len()];
    }

    /// Return the cells of the yarn, one for each column.
    pub fn cells(&self) -> &Vec<Cell> {
        &self.cells
    }

    /// Return the text colors of the yarn.
    pub fn colors(&self) -> &Vec<Option<Color>> {
        &self.colors
    }

    /// Return the background colors of the yarn.
    pub fn backgrounds(&self) -> &Vec<Option<Color>> {
        &self.backgrounds
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

    /// Return a yarn with the text before the position colored the other color.
    fn colored_before(string: &str, position: usize) -> Yarn {
        let mut yarn: Yarn = Yarn::from(string);
        yarn.color_before(OTHER_COLOR, position);
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

        assert_eq!(yarn.colors, expected_colors);
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

    #[test_case(Yarn::new(), Yarn::new(), Yarn::new(); "an empty yarn with an empty yarn is an empty yarn")]
    #[test_case(Yarn::new(), Yarn {cells: vec![Cell::BLANK; 1], ..Default::default()}, Yarn {cells: vec![Cell::BLANK; 1], ..Default::default()}; "an empty yarn with a one space yarn is a one space yarn")]
    #[test_case(Yarn {cells: vec![Cell::BLANK; 1], ..Default::default()}, Yarn::new(), Yarn {cells: vec![Cell::BLANK; 1], ..Default::default()}; "a one space yarn with an empty yarn is a one space yarn")]
    #[test_case(Yarn {cells: vec![Cell::BLANK; 1], ..Default::default()}, Yarn {cells: vec![Cell::BLANK; 1], colors: vec![Some(Color::Black)], ..Default::default()}, Yarn {cells: vec![Cell::BLANK; 2], colors: vec![None, Some(Color::Black)], ..Default::default()}; "concatenating two one space yarns preserves colors")]
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

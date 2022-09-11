use crossterm::style::Color as CrosstermColor;
use std::cmp::Ordering;

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Yarn {
    // MAYBE TODO: Store the length seperatley so we can represent blank yarn without vec manip?
    characters: Vec<char>,

    // NOTE: The style vectors are Allowed to be shorter than the number of characters.
    colors: Vec<Option<CrosstermColor>>,
    backgrounds: Vec<Option<CrosstermColor>>,
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
        let characters = vec![' '; len];
        Self {
            characters,
            ..Default::default()
        }
    }

    /// Return a yarn with string centered and truncated with dots if the string is longer than the
    /// the length.
    pub fn center(string: &str, len: usize) -> Self {
        if len == 0 {
            return Yarn::default();
        }

        match string.len().cmp(&len) {
            Ordering::Greater => {
                if len <= 3 {
                    return Yarn::from(vec!['.'; len]);
                }
                let mut characters: Vec<char> = Vec::with_capacity(len);
                characters.extend(string.chars().take(len - 3));
                characters.append(&mut vec!['.'; 3]);
                return Yarn::from(characters);
            }
            Ordering::Less => {
                let mut characters: Vec<char> = Vec::with_capacity(len);

                let before_len: usize = (len - string.len()) / 2;
                characters.append(&mut vec![' '; before_len]);

                characters.extend(string.chars());

                let after_len: usize = len - string.len() - before_len;
                characters.append(&mut vec![' '; after_len]);

                return Yarn::from(characters);
            }
            Ordering::Equal => {
                return Yarn::from(string);
            }
        };
    }

    pub fn len(&self) -> usize {
        self.characters.len()
    }

    /// Add the other yarn to the end of this one and return the new yarn.
    // Should this be called `extend`? The `std::iter::Extend` trait doesn't return `Self`?
    #[allow(dead_code)]
    pub fn concat(mut self, other: Self) -> Self {
        let len_before: usize = self.len();
        self.characters.extend(other.characters);

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

    #[allow(dead_code)]
    pub fn write_string(&mut self, position: usize, string: &str) {
        let characters: Vec<char> = string.chars().collect();

        let before = &self.characters[0..position];
        let after = &self.characters[(position + characters.len())..];
        self.characters = vec![before, &characters, after].concat();
    }

    pub fn resize(&mut self, new_len: usize) {
        let len = self.len();
        match len.cmp(&new_len) {
            Ordering::Greater => {
                self.truncate(new_len);
            }
            Ordering::Less => {
                self.characters.extend(vec![' '; new_len - len]);
            }
            Ordering::Equal => {}
        }
    }

    pub fn truncate(&mut self, new_len: usize) {
        self.characters.truncate(new_len);
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
                self.characters = [
                    vec![' '; left_pad],
                    self.characters.to_owned(),
                    vec![' '; right_pad],
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

    pub fn color(&mut self, color: CrosstermColor) {
        self.colors = vec![Some(color); self.len()];
    }

    /// Change the color of all text before the given position.
    pub fn color_before(&mut self, color: CrosstermColor, position: usize) {
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
    pub fn color_after(&mut self, color: CrosstermColor, position: usize) {
        let num_chars: usize = self.characters.len();
        if self.colors.len() < num_chars {
            for index in position..self.colors.len() {
                self.colors[index] = Some(color);
            }
            self.colors.resize(num_chars, Some(color));
        } else {
            for index in position..num_chars {
                self.colors[index] = Some(color);
            }
        }
    }

    pub fn background(&mut self, color: CrosstermColor) {
        self.backgrounds = vec![Some(color); self.len()];
    }

    pub fn characters(&self) -> &Vec<char> {
        &self.characters
    }

    pub fn colors(&self) -> &Vec<Option<CrosstermColor>> {
        &self.colors
    }

    pub fn backgrounds(&self) -> &Vec<Option<CrosstermColor>> {
        &self.backgrounds
    }
}

impl From<String> for Yarn {
    fn from(string: String) -> Self {
        let characters: Vec<char> = string.chars().collect();
        Yarn {
            characters,
            ..Default::default()
        }
    }
}

impl From<&str> for Yarn {
    fn from(string: &str) -> Self {
        let characters: Vec<char> = string.chars().collect();
        Yarn {
            characters,
            ..Default::default()
        }
    }
}

impl From<Vec<char>> for Yarn {
    fn from(characters: Vec<char>) -> Self {
        Yarn {
            characters,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

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
    #[test_case(Yarn::new(), Yarn {characters: vec![' '; 1], ..Default::default()}, Yarn {characters: vec![' '; 1], ..Default::default()}; )]
    #[test_case(Yarn {characters: vec![' '; 1], ..Default::default()}, Yarn::new(), Yarn {characters: vec![' '; 1], ..Default::default()}; )]
    #[test_case(Yarn {characters: vec![' '; 1], ..Default::default()}, Yarn {characters: vec![' '; 1], colors: vec![Some(CrosstermColor::Black)], ..Default::default()}, Yarn {characters: vec![' '; 2], colors: vec![None, Some(CrosstermColor::Black)], ..Default::default()}; )]
    fn test_concat(yarn: Yarn, other: Yarn, expected_yarn: Yarn) {
        let result: Yarn = yarn.concat(other);

        assert_eq!(result, expected_yarn);
    }
}

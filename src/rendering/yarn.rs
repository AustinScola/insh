use crossterm::style::Color;
use std::cmp::Ordering;

#[derive(Default)]
pub struct Yarn {
    // MAYBE TODO: Store the length seperatley so we can represent blank yarn without vec manip?
    characters: Vec<char>,

    // NOTE: The style vectors are Allowed to be shorter than the number of characters.
    colors: Vec<Option<Color>>,
    backgrounds: Vec<Option<Color>>,
}

impl Yarn {
    pub fn len(&self) -> usize {
        self.characters.len()
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

    pub fn color(&mut self, color: Color) {
        self.colors = vec![Some(color); self.len()];
    }

    pub fn background(&mut self, color: Color) {
        self.backgrounds = vec![Some(color); self.len()];
    }

    pub fn characters(&self) -> &Vec<char> {
        &self.characters
    }

    pub fn colors(&self) -> &Vec<Option<Color>> {
        &self.colors
    }

    pub fn backgrounds(&self) -> &Vec<Option<Color>> {
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

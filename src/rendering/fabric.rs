use super::{Location, Size, Yarn};

use std::cmp::Ordering;

use crossterm::style::Color;
use itertools::izip;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Fabric {
    size: Size,
    characters: Vec<Vec<char>>,

    // TODO: Use ranges to store this!
    colors: Vec<Vec<Option<Color>>>,
    backgrounds: Vec<Vec<Option<Color>>>,
}

impl Fabric {
    pub fn new(size: Size) -> Self {
        let characters = vec![vec![' '; size.columns]; size.rows];
        let colors = vec![vec![]; size.rows];
        let backgrounds = vec![vec![]; size.rows];
        Fabric {
            size,
            characters,
            colors,
            backgrounds,
        }
    }

    /// Return the size of the fabric.
    #[allow(dead_code)]
    pub fn size(&self) -> Size {
        self.size
    }

    #[allow(dead_code)]
    pub fn write(&mut self, string: String, location: Location) {
        let row = location.row;
        let mut column = location.column;
        for character in string.chars() {
            self.characters[row][column] = character;
            column += 1;
        }
    }

    pub fn characters(&self) -> &Vec<Vec<char>> {
        &self.characters
    }

    pub fn colors(&self) -> &Vec<Vec<Option<Color>>> {
        &self.colors
    }

    pub fn backgrounds(&self) -> &Vec<Vec<Option<Color>>> {
        &self.backgrounds
    }

    /// Vertically pad the fabric to `new_rows` by adding rows above and below.
    ///
    /// If the new number of rows is less than the current rows, then panic (for now).
    pub fn pad(&mut self, new_rows: usize) {
        match new_rows.cmp(&self.size.rows) {
            Ordering::Greater => {
                let difference = new_rows - self.size.rows;

                let top_pad_rows = difference / 2;
                let bottom_pad_rows = difference - top_pad_rows;

                self.characters = [
                    vec![vec![' '; self.size.columns]; top_pad_rows],
                    self.characters.to_owned(),
                    vec![vec![' '; self.size.columns]; bottom_pad_rows],
                ]
                .concat();
                self.colors = [
                    vec![vec![None; self.size.columns]; top_pad_rows],
                    self.colors.to_owned(),
                    vec![vec![None; self.size.columns]; bottom_pad_rows],
                ]
                .concat();
                self.backgrounds = [
                    vec![vec![None; self.size.columns]; top_pad_rows],
                    self.backgrounds.to_owned(),
                    vec![vec![None; self.size.columns]; bottom_pad_rows],
                ]
                .concat();
            }
            Ordering::Less => {
                panic!("Cannot pad a yarn to smaller than the current rows.")
            }
            Ordering::Equal => {}
        }
    }

    /// Verically pad the fabric to the new number of rows by adding rows below.
    ///
    /// Panic if the new number of rows is less than the current number of rows.
    pub fn pad_bottom(&mut self, new_rows: usize) {
        match new_rows.cmp(&self.size.rows) {
            Ordering::Greater => {
                let difference: usize = new_rows - self.size.rows;
                let columns: usize = self.size.columns;

                self.size = Size::new(new_rows, columns);
                self.characters.extend(vec![vec![' '; columns]; difference]);
                self.colors.extend(vec![vec![]; difference]);
                self.backgrounds.extend(vec![vec![]; difference]);
            }
            Ordering::Less => {
                panic!("Cannot pad a yarn to smaller than the current rows.")
            }
            Ordering::Equal => {}
        }
    }

    pub fn quilt_bottom(mut self, other: Fabric) -> Fabric {
        for (row, row_colors, row_backgrounds) in
            izip!(other.characters, other.colors, other.backgrounds)
        {
            self.characters.push(row.to_vec());
            self.colors.push(row_colors.to_vec());
            self.backgrounds.push(row_backgrounds.to_vec());
        }

        self.size.rows += other.size.rows;

        self
    }
}

impl From<Vec<Yarn>> for Fabric {
    fn from(rows: Vec<Yarn>) -> Self {
        let row_count: usize;
        let column_count: usize;
        {
            let max_column_count: Option<usize> = rows.iter().map(|row| row.len()).max();
            match max_column_count {
                Some(max_column_count) => {
                    row_count = rows.len();
                    column_count = max_column_count;
                }
                None => {
                    row_count = 0;
                    column_count = 0;
                }
            }
        }
        let size: Size = Size::new(row_count, column_count);

        // TODO: Only use one iteration here and don't clone?
        let characters: Vec<Vec<char>> = rows.iter().map(|row| row.characters().clone()).collect();
        let colors: Vec<Vec<Option<Color>>> = rows.iter().map(|row| row.colors().clone()).collect();
        let backgrounds: Vec<Vec<Option<Color>>> =
            rows.iter().map(|row| row.backgrounds().clone()).collect();

        Fabric {
            size,
            characters,
            colors,
            backgrounds,
        }
    }
}

impl From<Yarn> for Fabric {
    fn from(row: Yarn) -> Self {
        let size: Size;
        {
            let columns = row.len();
            size = Size::new(1, columns);
        }

        let characters = vec![row.characters().to_vec()];
        let colors = vec![row.colors().to_vec()];
        let backgrounds = vec![row.backgrounds().to_vec()];

        Fabric {
            size,
            characters,
            colors,
            backgrounds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case(Fabric::new(Size::new(1, 1)), 2, Fabric::new(Size::new(2, 1)))]
    fn test_pad_bottom(mut fabric: Fabric, new_rows: usize, expected: Fabric) {
        fabric.pad_bottom(new_rows);

        assert_eq!(fabric, expected);
    }

    #[test_case(
        Fabric::new(Size::new(2, 3)),
        Fabric::new(Size::new(1, 3)),
        Fabric::new(Size::new(3, 3))
    )]
    fn test_quilt_bottom(fabric: Fabric, other: Fabric, expected: Fabric) {
        let result = fabric.quilt_bottom(other);

        assert_eq!(result, expected);
    }
}

/*!
This module contains the [`Fabric`] struct which is used for representing a 2D rectangle of styled
text.
*/
use std::cmp::Ordering;
use std::ops::Range;

use super::{Cell, Row, Size, Spot, Yarn};

use ansi::Color;

use itertools::izip;

// MAYBE TODO: Use ranges for storage to save memory when elements are sparse?
/// A 2D rectangle of styled text.
#[derive(PartialEq, Eq, Debug, Clone, Default)]
pub struct Fabric {
    /// The size of the fabric.
    size: Size,
    /// The cells of the text, one for each column of each row.
    characters: Vec<Vec<Cell>>,
    /// The text colors.
    colors: Vec<Vec<Option<Color>>>,
    /// The background colors of the text.
    backgrounds: Vec<Vec<Option<Color>>>,
    /// Which columns of which rows are written in bold.
    bolds: Vec<Vec<bool>>,
}

impl Fabric {
    /// Return a new fabric with the given `size`.
    pub fn new(size: Size) -> Self {
        let characters = vec![vec![Cell::BLANK; size.columns]; size.rows];
        let colors = vec![vec![]; size.rows];
        let backgrounds = vec![vec![]; size.rows];
        let bolds = vec![vec![]; size.rows];
        Fabric {
            size,
            characters,
            colors,
            backgrounds,
            bolds,
        }
    }

    /// Return a fabric with the string centered both vertically and horizontally.
    ///
    /// If the string is wider than the size, then it is truncated and dots are added.
    pub fn center(string: &str, size: Size) -> Self {
        if size.rows == 0 || size.columns == 0 {
            return Fabric::new(size);
        }
        let mut yarns: Vec<Yarn> = Vec::with_capacity(size.rows);

        let rows_before: usize = (size.rows - 1) / 2;
        yarns.append(&mut vec![Yarn::blank(size.columns); rows_before]);

        let center_row: Yarn = Yarn::center(string, size.columns);
        yarns.push(center_row);

        let rows_after: usize = size.rows - 1 - rows_before;
        yarns.append(&mut vec![Yarn::blank(size.columns); rows_after]);

        Fabric::from(yarns)
    }

    /// Return the size of the fabric.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Return the given row.
    ///
    /// A fabric is a rectangle even though the rows it is woven from need not all be as long as
    /// each other, so a row which is not there at all is one of nothing but blanks, and so is the
    /// end of one which does not reach across.
    pub fn row(&self, row: usize) -> Row<'_> {
        let cells: &[Cell] = match self.characters.get(row) {
            Some(cells) => cells,
            None => return Row::builder().cells(&[]).build(),
        };

        return Row::builder()
            .cells(cells)
            .colors(Self::row_of(&self.colors, row))
            .backgrounds(Self::row_of(&self.backgrounds, row))
            .bolds(Self::bolds_of(&self.bolds, row))
            .build();
    }

    /// Return what is in the given column of the given row.
    pub fn spot(&self, row: usize, column: usize) -> Spot<'_> {
        return self.row(row).spot(column);
    }

    /// Return the given row of the colors, which is no colors at all when it is not there.
    fn row_of(colors: &[Vec<Option<Color>>], row: usize) -> &[Option<Color>] {
        return match colors.get(row) {
            Some(colors) => colors,
            None => &[],
        };
    }

    /// Return the given row of the bolds, which is none of them when it is not there.
    fn bolds_of(bolds: &[Vec<bool>], row: usize) -> &[bool] {
        return match bolds.get(row) {
            Some(bolds) => bolds,
            None => &[],
        };
    }

    /// Move the rows of the band the given number of rows up, or down when that is negative, and
    /// bring blank rows in behind them.
    pub fn scroll(&mut self, band: Range<usize>, rows: isize) {
        let blank: Vec<Cell> = vec![Cell::BLANK; self.size.columns];

        Self::scroll_rows(&mut self.characters, &band, rows, blank);
        Self::scroll_rows(&mut self.colors, &band, rows, Vec::new());
        Self::scroll_rows(&mut self.backgrounds, &band, rows, Vec::new());
        Self::scroll_rows(&mut self.bolds, &band, rows, Vec::new());
    }

    /// Move the rows of the band the given number of rows up, or down when that is negative, and
    /// bring the given blank row in behind them.
    fn scroll_rows<Line: Clone>(all: &mut [Line], band: &Range<usize>, rows: isize, blank: Line) {
        let band: Range<usize> = band.start..band.end.min(all.len());
        let up: bool = rows > 0;
        let rows: usize = rows.unsigned_abs().min(band.end.saturating_sub(band.start));

        // NOTE: The rows are swapped rather than moved because the ones which are swapped into the
        // far end of the band are the ones which are about to be blanked anyway.
        match up {
            true => {
                for row in band.start..(band.end - rows) {
                    all.swap(row, row + rows);
                }
                all[(band.end - rows)..band.end].fill(blank);
            }
            false => {
                for row in ((band.start + rows)..band.end).rev() {
                    all.swap(row, row - rows);
                }
                all[band.start..(band.start + rows)].fill(blank);
            }
        }
    }

    /// Return the cells composing the fabric, one for each column of each row.
    pub fn cells(&self) -> &Vec<Vec<Cell>> {
        &self.characters
    }

    /// Return the text colors.
    pub fn colors(&self) -> &Vec<Vec<Option<Color>>> {
        &self.colors
    }

    /// Return the background colors of the text.
    pub fn backgrounds(&self) -> &Vec<Vec<Option<Color>>> {
        &self.backgrounds
    }

    /// Return which columns of which rows are written in bold.
    pub fn bolds(&self) -> &Vec<Vec<bool>> {
        &self.bolds
    }

    /// Vertically pad the fabric to `new_rows` by adding rows above and below.
    ///
    /// If the new number of rows is less than the current rows, then panic (for now).
    #[allow(dead_code)]
    pub fn pad(&mut self, new_rows: usize) {
        match new_rows.cmp(&self.size.rows) {
            Ordering::Greater => {
                let difference = new_rows - self.size.rows;
                self.size = Size::new(new_rows, self.size.columns);

                let top_pad_rows = difference / 2;
                let bottom_pad_rows = difference - top_pad_rows;

                self.characters = [
                    vec![vec![Cell::BLANK; self.size.columns]; top_pad_rows],
                    self.characters.to_owned(),
                    vec![vec![Cell::BLANK; self.size.columns]; bottom_pad_rows],
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
                self.bolds = [
                    vec![vec![false; self.size.columns]; top_pad_rows],
                    self.bolds.to_owned(),
                    vec![vec![false; self.size.columns]; bottom_pad_rows],
                ]
                .concat();
            }
            Ordering::Less => {
                panic!("Cannot pad a yarn to smaller than the current rows.")
            }
            Ordering::Equal => {}
        }
    }

    /// Vertically pad the fabric to the new number of rows by adding rows below.
    ///
    /// Panic if the new number of rows is less than the current number of rows.
    pub fn pad_bottom(&mut self, new_rows: usize) {
        match new_rows.cmp(&self.size.rows) {
            Ordering::Greater => {
                let difference: usize = new_rows - self.size.rows;
                let columns: usize = self.size.columns;

                self.size = Size::new(new_rows, columns);
                self.characters
                    .extend(vec![vec![Cell::BLANK; columns]; difference]);
                self.colors.extend(vec![vec![]; difference]);
                self.backgrounds.extend(vec![vec![]; difference]);
                self.bolds.extend(vec![vec![]; difference]);
            }
            Ordering::Less => {
                panic!("Cannot pad a yarn to smaller than the current rows.")
            }
            Ordering::Equal => {}
        }
    }

    /// Combine this fabric with another adding the contents of the other fabric to the bottom of
    /// this one.
    pub fn quilt_bottom(mut self, other: Fabric) -> Fabric {
        for (row, row_colors, row_backgrounds, row_bolds) in izip!(
            other.characters,
            other.colors,
            other.backgrounds,
            other.bolds
        ) {
            self.characters.push(row.to_vec());
            self.colors.push(row_colors.to_vec());
            self.backgrounds.push(row_backgrounds.to_vec());
            self.bolds.push(row_bolds.to_vec());
        }

        self.size.rows += other.size.rows;

        // NOTE: A fabric is a rectangle as wide as the widest thing in it, and the columns past
        // the end of a row which does not reach across are blanks. Taking the wider of the two is
        // what keeps the rows of the other one from being cut off at the width of this one.
        self.size.columns = self.size.columns.max(other.size.columns);

        self
    }

    /// Combine this fabric with another putting the contents of the other fabric to the right of
    /// this one.
    ///
    /// This is what puts two panes side by side.
    pub fn quilt_right(mut self, other: Fabric) -> Fabric {
        let rows: usize = self.size.rows.max(other.size.rows);
        let columns: usize = self.size.columns;

        self.characters.resize(rows, Vec::new());
        self.colors.resize(rows, Vec::new());
        self.backgrounds.resize(rows, Vec::new());
        self.bolds.resize(rows, Vec::new());

        for row in 0..rows {
            // A row is only as long as the text in it, and the columns past the end are blanks. It
            // has to be filled out to the full width first, or the other fabric would start part
            // way into this one on every row which does not reach across.
            self.characters[row].resize(columns, Cell::BLANK);
            if row < other.size.rows {
                let mut characters: Vec<Cell> = other.characters[row].clone();
                characters.resize(other.size.columns, Cell::BLANK);
                self.characters[row].append(&mut characters);
            }

            // An empty row of colors means that nothing in the row is colored, so it is left empty
            // rather than filled with nothing, which is what a fabric built from text looks like.
            let other_colors: &[Option<Color>] = match row < other.size.rows {
                true => &other.colors[row],
                false => &[],
            };
            if !self.colors[row].is_empty() || !other_colors.is_empty() {
                self.colors[row].resize(columns, None);
                let mut colors: Vec<Option<Color>> = other_colors.to_vec();
                colors.resize(other.size.columns, None);
                self.colors[row].append(&mut colors);
            }

            let other_backgrounds: &[Option<Color>] = match row < other.size.rows {
                true => &other.backgrounds[row],
                false => &[],
            };
            if !self.backgrounds[row].is_empty() || !other_backgrounds.is_empty() {
                self.backgrounds[row].resize(columns, None);
                let mut backgrounds: Vec<Option<Color>> = other_backgrounds.to_vec();
                backgrounds.resize(other.size.columns, None);
                self.backgrounds[row].append(&mut backgrounds);
            }

            let other_bolds: &[bool] = match row < other.size.rows {
                true => &other.bolds[row],
                false => &[],
            };
            if !self.bolds[row].is_empty() || !other_bolds.is_empty() {
                self.bolds[row].resize(columns, false);
                let mut bolds: Vec<bool> = other_bolds.to_vec();
                bolds.resize(other.size.columns, false);
                self.bolds[row].append(&mut bolds);
            }
        }

        self.size.rows = rows;
        self.size.columns = columns + other.size.columns;

        self
    }
}

impl From<Vec<&str>> for Fabric {
    fn from(strings: Vec<&str>) -> Self {
        let yarns: Vec<Yarn> = strings.into_iter().map(Yarn::from).collect();
        Fabric::from(yarns)
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
        let characters: Vec<Vec<Cell>> = rows.iter().map(|row| row.cells().clone()).collect();
        let colors: Vec<Vec<Option<Color>>> = rows.iter().map(|row| row.colors().clone()).collect();
        let backgrounds: Vec<Vec<Option<Color>>> =
            rows.iter().map(|row| row.backgrounds().clone()).collect();
        let bolds: Vec<Vec<bool>> = rows.iter().map(|row| row.bolds().clone()).collect();

        Fabric {
            size,
            characters,
            colors,
            backgrounds,
            bolds,
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

        let characters = vec![row.cells().to_vec()];
        let colors = vec![row.colors().to_vec()];
        let backgrounds = vec![row.backgrounds().to_vec()];
        let bolds = vec![row.bolds().to_vec()];

        Fabric {
            size,
            characters,
            colors,
            backgrounds,
            bolds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case(0..4, 1, Fabric::from(vec!["cd", "ef", "gh", "  "]); "every row up one")]
    #[test_case(0..4, -1, Fabric::from(vec!["  ", "ab", "cd", "ef"]); "every row down one")]
    #[test_case(0..4, 2, Fabric::from(vec!["ef", "gh", "  ", "  "]); "every row up two")]
    #[test_case(1..3, 1, Fabric::from(vec!["ab", "ef", "  ", "gh"]); "only the rows between the first and the last")]
    #[test_case(0..4, 9, Fabric::from(vec!["  ", "  ", "  ", "  "]); "further than there are rows to move")]
    fn test_scroll(band: Range<usize>, rows: isize, expected: Fabric) {
        let mut fabric = Fabric::from(vec!["ab", "cd", "ef", "gh"]);

        fabric.scroll(band, rows);

        assert_eq!(fabric, expected);
    }

    #[test_case("foo", Size::default(), Fabric::default(); "zero size")]
    #[test_case("foo", Size::new(1, 3), Fabric::from(vec!["foo"]); "text just fits")]
    #[test_case("foo", Size::new(3, 3), Fabric::from(vec!["   ", "foo", "   "]); "text is in the center vertically")]
    #[test_case("foo", Size::new(4, 3), Fabric::from(vec!["   ", "foo", "   ", "   "]); "text is in the center vertically and breaks ties vertially upwards")]
    #[test_case("foo", Size::new(1, 5), Fabric::from(vec![" foo "]); "text is in the centered horizontally")]
    #[test_case("foo", Size::new(1, 6), Fabric::from(vec![" foo  "]); "text is in the centered horizontally and breaks ties leftwards")]
    #[test_case("foobar", Size::new(1, 5), Fabric::from(vec!["fo..."]); "text that doesn't fit is truncated with dots")]
    fn test_center(string: &str, size: Size, expected_result: Fabric) {
        let result: Fabric = Fabric::center(string, size);

        assert_eq!(result, expected_result);
    }

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
    #[test_case(
        Fabric::from(vec!["ab"]),
        Fabric::from(vec!["cdef"]),
        Fabric::from(vec!["ab", "cdef"])
    )]
    fn test_quilt_bottom(fabric: Fabric, other: Fabric, expected: Fabric) {
        let result = fabric.quilt_bottom(other);

        assert_eq!(result, expected);
    }

    #[test_case(
        Fabric::from(vec!["ab", "cd"]),
        Fabric::from(vec!["ef", "gh"]),
        Fabric::from(vec!["abef", "cdgh"]);
        "two of the same size"
    )]
    // The rows past the end of the other fabric are left as they are. A row which does not reach
    // across is blank from there on, so there is nothing to add to it.
    #[test_case(
        Fabric::from(vec!["ab", "cd"]),
        Fabric::from(vec!["ef"]),
        Fabric::from(vec!["abef", "cd"]);
        "the right one is shorter"
    )]
    #[test_case(
        Fabric::from(vec!["ab"]),
        Fabric::from(vec!["ef", "gh"]),
        Fabric::from(vec!["abef", "  gh"]);
        "the left one is shorter"
    )]
    #[test_case(
        Fabric::from(vec!["ab", "c"]),
        Fabric::from(vec!["ef", "gh"]),
        Fabric::from(vec!["abef", "c gh"]);
        "a row of the left one does not reach across"
    )]
    #[test_case(
        Fabric::new(Size::new(2, 2)),
        Fabric::from(vec!["ef", "gh"]),
        Fabric::from(vec!["  ef", "  gh"]);
        "the left one is blank"
    )]
    fn test_quilt_right(fabric: Fabric, other: Fabric, expected: Fabric) {
        let result = fabric.quilt_right(other);

        assert_eq!(result, expected);
    }

    // Padding used to add the rows without saying that it had, which left whoever drew the fabric
    // painting only the rows it started with and leaving whatever was under the rest on screen.
    #[test_case(2, 5; "padding out")]
    #[test_case(2, 3; "padding a little")]
    #[test_case(2, 2; "padding to the size it already is")]
    fn test_pad_says_how_big_it_is(rows: usize, new_rows: usize) {
        let mut fabric = Fabric::new(Size::new(rows, 4));

        fabric.pad(new_rows);

        assert_eq!(fabric.size().rows, new_rows);
        assert_eq!(fabric.cells().len(), new_rows);
    }

    #[test_case(2, 5; "padding out")]
    #[test_case(2, 2; "padding to the size it already is")]
    fn test_pad_bottom_says_how_big_it_is(rows: usize, new_rows: usize) {
        let mut fabric = Fabric::new(Size::new(rows, 4));

        fabric.pad_bottom(new_rows);

        assert_eq!(fabric.size().rows, new_rows);
        assert_eq!(fabric.cells().len(), new_rows);
    }
}

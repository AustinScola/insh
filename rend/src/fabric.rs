/*!
This module contains the [`Fabric`] struct which is used for representing a 2D rectangle of styled
text.
*/
use std::cmp::Ordering;
use std::ops::Range;

use super::{Cell, Location, Row, Size, Spot, Style, Yarn};

// MAYBE TODO: Use ranges for storage to save memory when elements are sparse?
/// A 2D rectangle of styled text.
#[derive(PartialEq, Eq, Debug, Clone, Default)]
pub struct Fabric {
    /// The size of the fabric.
    size: Size,
    /// The cells of the text, one for each column of each row.
    characters: Vec<Vec<Cell>>,
    /// The way each column of each row is written.
    styles: Vec<Vec<Style>>,
    /// Where the cursor is shown, which is nowhere at all when it is not shown.
    cursor: Option<Location>,
}

impl Fabric {
    /// Return a new fabric with the given `size`.
    pub fn new(size: Size) -> Self {
        let characters = vec![vec![Cell::BLANK; size.columns]; size.rows];
        let styles = vec![vec![]; size.rows];
        Fabric {
            size,
            characters,
            styles,
            cursor: None,
        }
    }

    /// Return where the cursor is shown, which is nowhere at all when it is not shown.
    pub fn cursor(&self) -> Option<Location> {
        return self.cursor;
    }

    /// Show the cursor in the given spot, or nowhere at all when there is none.
    pub fn set_cursor(&mut self, cursor: Option<Location>) {
        self.cursor = cursor;
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
            .styles(Self::row_of(&self.styles, row))
            .build();
    }

    /// Return what is in the given column of the given row.
    pub fn spot(&self, row: usize, column: usize) -> Spot<'_> {
        return self.row(row).spot(column);
    }

    /// Return the given row of the styles, which is none at all when it is not there.
    fn row_of(styles: &[Vec<Style>], row: usize) -> &[Style] {
        return match styles.get(row) {
            Some(styles) => styles,
            None => &[],
        };
    }

    /// Move the rows of the band the given number of rows up, or down when that is negative, and
    /// bring blank rows in behind them.
    pub fn scroll(&mut self, band: Range<usize>, rows: isize) {
        let blank: Vec<Cell> = vec![Cell::BLANK; self.size.columns];

        Self::scroll_rows(&mut self.characters, &band, rows, blank);
        Self::scroll_rows(&mut self.styles, &band, rows, Vec::new());
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

    /// Write every column of the range of the row the given way, leaving what is written in them
    /// as it is.
    ///
    /// Nothing is written for a row or for columns which are not there.
    pub fn style(&mut self, style: Style, row: usize, columns: Range<usize>) {
        let columns: Range<usize> =
            columns.start.min(self.size.columns)..columns.end.min(self.size.columns);
        if columns.is_empty() {
            return;
        }

        let styles: &mut Vec<Style> = match self.styles.get_mut(row) {
            Some(styles) => styles,
            None => return,
        };

        // NOTE: The styles are only kept for as many columns as have been written in some way, so
        // they are filled out as far as the range before any of them is changed.
        if styles.len() < columns.end {
            styles.resize(columns.end, Style::default());
        }

        for written in &mut styles[columns] {
            *written = style;
        }
    }

    /// Return what is written in the range of the row.
    ///
    /// A column which carries on the cluster in the column before it adds nothing, since the whole
    /// cluster is already there. Nothing is returned for a row which is not there.
    pub fn text(&self, row: usize, columns: Range<usize>) -> String {
        let characters: &Vec<Cell> = match self.characters.get(row) {
            Some(characters) => characters,
            None => return String::new(),
        };

        let columns: Range<usize> =
            columns.start.min(characters.len())..columns.end.min(characters.len());
        if columns.is_empty() {
            return String::new();
        }

        let mut text = String::new();
        for cell in &characters[columns] {
            match cell {
                Cell::Char(character) => text.push(*character),
                Cell::Cluster(cluster) => text.push_str(cluster),
                Cell::Continuation => {}
            }
        }

        return text;
    }

    /// Return the cells composing the fabric, one for each column of each row.
    pub fn cells(&self) -> &Vec<Vec<Cell>> {
        &self.characters
    }

    /// Return the way each column of each row is written, which may be given for fewer columns
    /// than there are cells.
    pub fn styles(&self) -> &Vec<Vec<Style>> {
        &self.styles
    }

    /// Vertically pad the fabric to `new_rows` by adding rows above and below.
    ///
    /// If the new number of rows is less than the current rows, then panic (for now).
    #[allow(dead_code)]
    pub fn pad(&mut self, new_rows: usize) {
        match new_rows.cmp(&self.size.rows) {
            Ordering::Greater => {
                let difference = new_rows - self.size.rows;

                let top_pad_rows = difference / 2;
                let bottom_pad_rows = difference - top_pad_rows;

                self.characters = [
                    vec![vec![Cell::BLANK; self.size.columns]; top_pad_rows],
                    self.characters.to_owned(),
                    vec![vec![Cell::BLANK; self.size.columns]; bottom_pad_rows],
                ]
                .concat();
                self.styles = [
                    vec![Vec::new(); top_pad_rows],
                    self.styles.to_owned(),
                    vec![Vec::new(); bottom_pad_rows],
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
                self.styles.extend(vec![vec![]; difference]);
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
        // NOTE: Only one cursor is ever shown, so the one which is already there is kept and the
        // other one is only taken when there is none. Nothing which is put together has a cursor
        // in more than one of its parts, since only the pane which is focused shows one.
        self.cursor = self.cursor.or_else(|| {
            other.cursor.map(|at| {
                Location::builder()
                    .row(at.row + self.size.rows)
                    .column(at.column)
                    .build()
            })
        });

        for (row, row_styles) in other.characters.into_iter().zip(other.styles) {
            self.characters.push(row);
            self.styles.push(row_styles);
        }

        self.size.rows += other.size.rows;

        // NOTE: A fabric is a rectangle as wide as the widest thing in it, and the columns past
        // the end of a row which does not reach across are blanks. Taking the wider of the two is
        // what keeps the rows of the other one from being cut off at the width of this one.
        self.size.columns = self.size.columns.max(other.size.columns);

        self
    }

    /// Combine this fabric with another by putting the other one along the right of this one.
    ///
    /// The taller of the two says how many rows there are, and whichever of them falls short of
    /// that is blank for the rest of them.
    pub fn quilt_right(mut self, other: Fabric) -> Fabric {
        let rows: usize = self.size.rows.max(other.size.rows);
        let columns: usize = self.size.columns;

        // NOTE: Only one cursor is ever shown, the same way as when one fabric goes below another.
        self.cursor = self.cursor.or_else(|| {
            other.cursor.map(|at| {
                Location::builder()
                    .row(at.row)
                    .column(at.column + columns)
                    .build()
            })
        });

        // NOTE: A row is allowed to stop short of the width of the fabric, so the rows on the left
        // are filled out before the ones on the right go onto the end of them. Without that, the
        // columns on the right would land wherever the row on the left happened to stop.
        self.characters.resize(rows, Vec::new());
        self.styles.resize(rows, Vec::new());
        for cells in &mut self.characters {
            cells.resize(columns, Cell::BLANK);
        }

        for (row, (cells, styles)) in other.characters.into_iter().zip(other.styles).enumerate() {
            self.characters[row].extend(cells);

            if !styles.is_empty() {
                self.styles[row].resize(columns, Style::default());
                self.styles[row].extend(styles);
            }
        }

        self.size = Size::new(rows, columns + other.size.columns);

        self
    }

    /// Put the other fabric onto this one with its top left corner in the given spot.
    ///
    /// Whatever of it would fall outside this fabric is left off, and this fabric stays the size
    /// it already was.
    pub fn blit(&mut self, other: &Fabric, at: Location) {
        let columns: usize = self.size.columns;
        if at.row >= self.size.rows || at.column >= columns {
            return;
        }

        let width: usize = other.size.columns.min(columns - at.column);
        let after: usize = columns - at.column - width;

        // The cursor of the fabric which is put on shows through, so long as it lands somewhere
        // which is not left off.
        if let Some(cursor) = other.cursor {
            if cursor.row < other.size.rows.min(self.size.rows - at.row) && cursor.column < width {
                self.cursor = Some(
                    Location::builder()
                        .row(at.row + cursor.row)
                        .column(at.column + cursor.column)
                        .build(),
                );
            }
        }

        for row in 0..other.size.rows {
            let onto: usize = at.row + row;
            if onto >= self.size.rows {
                break;
            }

            let mut cells: Vec<Cell> = self.characters[onto].clone();
            cells.resize(columns, Cell::BLANK);
            let mut styles: Vec<Style> = self.styles[onto].clone();
            styles.resize(columns, Style::default());

            // NOTE: A wide cluster which either edge of the other fabric cuts through is blanked
            // rather than left with only half of itself written over, which would push the rest of
            // the row along. Shortening the cells from either end is what already knows to do that.
            let mut before: Vec<Cell> = cells.clone();
            Cell::truncate(&mut before, at.column);

            let mut middle: Vec<Cell> = (0..other.size.columns)
                .map(|column| other.spot(row, column).cell().clone())
                .collect();
            Cell::truncate(&mut middle, width);

            let mut behind: Vec<Cell> = cells;
            Cell::keep_last(&mut behind, after);

            let mut written: Vec<Cell> = before;
            written.extend(middle);
            written.extend(behind);
            self.characters[onto] = written;

            let mut restyled: Vec<Style> = styles[..at.column].to_vec();
            restyled.extend((0..width).map(|column| other.spot(row, column).style()));
            restyled.extend_from_slice(&styles[columns - after..]);
            self.styles[onto] = restyled;
        }
    }

    /// Return the part of the fabric of the given size with its top left corner in the given spot.
    ///
    /// A spot outside the fabric is blank, so the part which is taken is always the size which was
    /// asked for however little of the fabric it covers.
    pub fn crop(&self, at: Location, size: Size) -> Fabric {
        let mut characters: Vec<Vec<Cell>> = Vec::with_capacity(size.rows);
        let mut styles: Vec<Vec<Style>> = Vec::with_capacity(size.rows);

        for row in 0..size.rows {
            let from: usize = at.row + row;

            let mut cells: Vec<Cell> = Vec::with_capacity(size.columns);
            let mut row_styles: Vec<Style> = Vec::with_capacity(size.columns);
            for column in 0..size.columns {
                let spot: Spot = self.spot(from, at.column + column);
                cells.push(spot.cell().clone());
                row_styles.push(spot.style());
            }

            // NOTE: A wide cluster which either edge cuts through is blanked, the same way as when
            // one fabric is put onto another.
            if cells.first().is_some_and(Cell::is_continuation) {
                cells[0] = Cell::BLANK;
            }
            if cells.last().is_some_and(|cell| cell.width() > 1) {
                *cells.last_mut().unwrap() = Cell::BLANK;
            }

            characters.push(cells);
            styles.push(row_styles);
        }

        // The cursor is only shown when it is in the part which is taken.
        let cursor: Option<Location> = self
            .cursor
            .filter(|cursor| {
                (cursor.row >= at.row && cursor.row < at.row + size.rows)
                    && (cursor.column >= at.column && cursor.column < at.column + size.columns)
            })
            .map(|cursor| {
                Location::builder()
                    .row(cursor.row - at.row)
                    .column(cursor.column - at.column)
                    .build()
            });

        return Fabric {
            size,
            characters,
            styles,
            cursor,
        };
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
        let styles: Vec<Vec<Style>> = rows.iter().map(|row| row.styles().clone()).collect();

        Fabric {
            size,
            characters,
            styles,
            cursor: None,
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
        let styles = vec![row.styles().to_vec()];

        Fabric {
            size,
            characters,
            styles,
            cursor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ansi::Color;

    use test_case::test_case;

    #[test_case(0, 0..3, "abc"; "the whole of a row")]
    #[test_case(0, 1..3, "bc"; "the rest of a row")]
    #[test_case(1, 0..2, "de"; "the start of another row")]
    #[test_case(0, 2..2, ""; "no columns at all")]
    #[test_case(0, 1..9, "bc"; "further than there are columns")]
    #[test_case(9, 0..3, ""; "a row which is not there")]
    #[test_case(0, 3..1, ""; "columns the wrong way round")]
    fn test_text(row: usize, columns: Range<usize>, expected: &str) {
        let fabric = Fabric::from(vec!["abc", "def"]);

        assert_eq!(fabric.text(row, columns), expected);
    }

    #[test_case(0, 0..2, vec![Some(Color::Red), Some(Color::Red), None]; "the start of a row")]
    #[test_case(0, 1..3, vec![None, Some(Color::Red), Some(Color::Red)]; "the rest of a row")]
    #[test_case(0, 1..9, vec![None, Some(Color::Red), Some(Color::Red)]; "further than there are columns")]
    #[test_case(0, 2..2, vec![None, None, None]; "no columns at all")]
    fn test_style(row: usize, columns: Range<usize>, expected: Vec<Option<Color>>) {
        let mut background = Style::default();
        background.set_background(Some(Color::Red));
        let mut fabric = Fabric::from(vec!["abc", "def"]);

        fabric.style(background, row, columns);

        let backgrounds: Vec<Option<Color>> = (0..3)
            .map(|column| fabric.spot(row, column).style().background())
            .collect();
        assert_eq!(backgrounds, expected);
    }

    #[test]
    fn test_styling_a_row_which_is_not_there_does_nothing() {
        let mut background = Style::default();
        background.set_background(Some(Color::Red));
        let mut fabric = Fabric::from(vec!["abc", "def"]);
        let before: Fabric = fabric.clone();

        fabric.style(background, 9, 0..3);

        assert_eq!(fabric, before);
    }

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

    /// Return what each row of the fabric is written with, which is what it looks like on a screen.
    fn shown(fabric: &Fabric) -> Vec<String> {
        return (0..fabric.size().rows)
            .map(|row| {
                (0..fabric.size().columns)
                    .map(|column| fabric.spot(row, column).cell().to_string())
                    .collect()
            })
            .collect();
    }

    #[test_case(Fabric::from(vec!["ab", "cd"]), Fabric::from(vec!["ef", "gh"]), vec!["abef", "cdgh"]; "two fabrics of the same size")]
    #[test_case(Fabric::from(vec!["ab", "cd"]), Fabric::from(vec!["e"]), vec!["abe", "cd "]; "the one on the right being shorter")]
    #[test_case(Fabric::from(vec!["a"]), Fabric::from(vec!["bc", "de"]), vec!["abc", " de"]; "the one on the left being shorter")]
    #[test_case(Fabric::from(vec!["ab", "c"]), Fabric::from(vec!["de", "fg"]), vec!["abde", "c fg"]; "a row which stops short of the width filled out first")]
    fn test_quilt_right(fabric: Fabric, other: Fabric, expected: Vec<&str>) {
        let result: Fabric = fabric.quilt_right(other);

        assert_eq!(shown(&result), expected);
        assert_eq!(result.size().columns, expected[0].chars().count());
    }

    /// Return the spot at the given row and column.
    fn at(row: usize, column: usize) -> Location {
        Location::builder().row(row).column(column).build()
    }

    #[test_case(Fabric::from(vec!["XY"]), at(0, 0), vec!["XYcd", "efgh"]; "in the corner")]
    #[test_case(Fabric::from(vec!["XY"]), at(1, 2), vec!["abcd", "efXY"]; "partway in")]
    #[test_case(Fabric::from(vec!["XY", "ZW"]), at(0, 3), vec!["abcX", "efgZ"]; "what hangs off the right left off")]
    #[test_case(Fabric::from(vec!["XY", "ZW"]), at(1, 0), vec!["abcd", "XYgh"]; "what hangs off the bottom left off")]
    #[test_case(Fabric::from(vec!["XY"]), at(0, 9), vec!["abcd", "efgh"]; "past the right hand edge altogether")]
    fn test_blit(other: Fabric, at: Location, expected: Vec<&str>) {
        let mut fabric = Fabric::from(vec!["abcd", "efgh"]);

        fabric.blit(&other, at);

        assert_eq!(shown(&fabric), expected);
        assert_eq!(fabric.size(), Size::new(2, 4));
    }

    #[test]
    fn test_blitting_over_half_of_a_wide_cluster_blanks_the_rest_of_it() {
        // The crab is written in the second and third columns, so putting something over the third
        // has to take the whole of it away rather than leave its first half behind.
        let mut fabric = Fabric::from(vec!["a🦀b"]);

        fabric.blit(&Fabric::from(vec!["XY"]), at(0, 2));

        assert_eq!(shown(&fabric), vec!["a XY"]);
    }

    #[test]
    fn test_a_wide_cluster_which_hangs_off_the_edge_is_blanked() {
        let mut fabric = Fabric::from(vec!["abcd"]);

        fabric.blit(&Fabric::from(vec!["🦀"]), at(0, 3));

        assert_eq!(shown(&fabric), vec!["abc "]);
    }

    #[test]
    fn test_blitting_keeps_how_the_columns_around_it_are_written() {
        let mut yarn = Yarn::from("abcd");
        yarn.color(Color::Red);
        let mut fabric = Fabric::from(yarn);

        let mut written = Yarn::from("X");
        written.color(Color::Blue);
        fabric.blit(&Fabric::from(written), at(0, 1));

        let colors: Vec<Option<Color>> = (0..4)
            .map(|column| fabric.spot(0, column).style().color())
            .collect();
        assert_eq!(
            colors,
            vec![
                Some(Color::Red),
                Some(Color::Blue),
                Some(Color::Red),
                Some(Color::Red)
            ]
        );
    }

    #[test_case(at(0, 0), Size::new(1, 2), vec!["ab"]; "the corner")]
    #[test_case(at(1, 2), Size::new(1, 2), vec!["gh"]; "partway in")]
    #[test_case(at(0, 3), Size::new(2, 2), vec!["d ", "h "]; "past the right hand edge is blank")]
    #[test_case(at(2, 0), Size::new(1, 2), vec!["  "]; "past the bottom is blank")]
    fn test_crop(at: Location, size: Size, expected: Vec<&str>) {
        let fabric = Fabric::from(vec!["abcd", "efgh"]);

        let cropped: Fabric = fabric.crop(at, size);

        assert_eq!(shown(&cropped), expected);
        assert_eq!(cropped.size(), size);
    }

    #[test_case(at(0, 2), vec![" b"]; "a wide cluster cut through at the start")]
    #[test_case(at(0, 0), vec!["a "]; "a wide cluster cut through at the end")]
    fn test_cropping_through_a_wide_cluster_blanks_it(at: Location, expected: Vec<&str>) {
        let fabric = Fabric::from(vec!["a🦀b"]);

        let cropped: Fabric = fabric.crop(at, Size::new(1, 2));

        assert_eq!(shown(&cropped), expected);
    }
}

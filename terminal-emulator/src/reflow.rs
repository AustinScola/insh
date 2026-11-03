/*!
This module contains the [`Reflow`] struct, which wraps rows which were written for one width again
for another.
*/
use crate::grid::Line;

use rend::{Cell, Location, Style};

/// Wraps rows which were written for one width again for another.
///
/// The rows which were wrapped are put back together into the lines they were wrapped from, and
/// those are wrapped again at the new width, which is what a terminal which goes to the trouble does
/// when it changes size. A program which redraws itself after being told the size has changed
/// counts on it: a shell works out how far up its prompt is from how long the line is, and goes up
/// too far or not far enough on a terminal which leaves the rows as they were.
pub struct Reflow {
    /// How many columns each row is wrapped at.
    columns: usize,
}

impl Reflow {
    /// Return the rows wrapped again at the given width, moving each of the spots along with
    /// whatever is written in it.
    ///
    /// A spot is a row of the rows given and a column of it, and is moved to the row of the rows
    /// given back which that column ends up in. Whatever is in a line after the last column which
    /// has anything written in it is let go of, apart from the columns up to any of the spots, so
    /// that a spot is never left further past the end of what it was in than it was.
    pub fn of(rows: Vec<Line>, columns: usize, spots: &mut [Location]) -> Vec<Line> {
        let reflow = Self {
            columns: columns.max(1),
        };

        // NOTE: The spots are matched to the rows they were in before any of them moved, since one
        // which has been moved already could otherwise be taken for one in a row further down.
        let was: Vec<Location> = spots.to_vec();

        let mut wrapped: Vec<Line> = Vec::new();
        let mut first: usize = 0;
        while first < rows.len() {
            // The rows of the line are the first one and every one which the one before it carries
            // on into.
            let mut last: usize = first;
            while last + 1 < rows.len() && rows[last].is_wrapped() {
                last += 1;
            }

            reflow.line(&rows[first..=last], first, &was, spots, &mut wrapped);
            first = last + 1;
        }

        return wrapped;
    }

    /// Wrap the line which the rows are the parts of, the first of which is the given row, onto the
    /// end of the rows which have been wrapped already, moving the spots which were in it.
    fn line(
        &self,
        rows: &[Line],
        first: usize,
        was: &[Location],
        spots: &mut [Location],
        wrapped: &mut Vec<Line>,
    ) {
        let mut cells: Vec<Cell> = Vec::new();
        let mut styles: Vec<Style> = Vec::new();
        // Where along the line each of the spots in it is, which is nothing for one in another.
        let mut along: Vec<Option<usize>> = vec![None; spots.len()];

        for (number, row) in rows.iter().enumerate() {
            for (spot, along) in was.iter().zip(along.iter_mut()) {
                if spot.row == first + number {
                    *along = Some(cells.len() + spot.column.min(row.columns().saturating_sub(1)));
                }
            }

            cells.extend_from_slice(row.cells());
            styles.extend_from_slice(row.styles());
        }

        // NOTE: A row is filled out to its width with blanks, so whatever comes after the last of
        // what is written is left behind. Keeping them would wrap a short line onto a row of its
        // own whenever the screen is made narrower than it was.
        let written: usize = cells
            .iter()
            .rposition(|cell| *cell != Cell::BLANK)
            .map_or(0, |last| last + 1);
        let kept: usize = along
            .iter()
            .flatten()
            .copied()
            .fold(written, usize::max)
            .max(1)
            .min(cells.len().max(1));
        cells.resize(kept, Cell::BLANK);
        styles.resize(kept, Style::default());

        let mut row: (Vec<Cell>, Vec<Style>) = (Vec::new(), Vec::new());
        let mut column: usize = 0;
        while column < kept {
            let width: usize = Self::width(&cells, column);

            // A cluster which does not fit in what is left of the row goes onto the next one,
            // the way it does when it is written.
            if !row.0.is_empty() && row.0.len() + width > self.columns {
                wrapped.push(self.row(std::mem::take(&mut row), true));
            }

            for (spot, along) in spots.iter_mut().zip(along.iter()) {
                if let Some(along) = along.filter(|along| (column..column + width).contains(along))
                {
                    spot.row = wrapped.len();
                    spot.column = (row.0.len() + along - column).min(self.columns - 1);
                }
            }

            // NOTE: A cluster wider than the row is, which only a row of one column has, is blanked
            // rather than cut in half.
            match width > self.columns {
                true => {
                    row.0.push(Cell::BLANK);
                    row.1.push(styles[column]);
                }
                false => {
                    row.0.extend_from_slice(&cells[column..column + width]);
                    row.1.extend_from_slice(&styles[column..column + width]);
                }
            }

            column += width;
        }

        // NOTE: A spot just past the end of what is written, which is where the cursor is once a
        // line has been written, is left just past the end of it again rather than being given a
        // blank of its own to sit on, which would make a row of its own when it lands on the edge.
        for (spot, along) in spots.iter_mut().zip(along.iter()) {
            if along.is_some_and(|along| along >= kept) {
                spot.row = wrapped.len();
                spot.column = row.0.len().min(self.columns - 1);
            }
        }

        wrapped.push(self.row(row, false));
    }

    /// Return how many columns the cluster which starts in the given column is written across.
    fn width(cells: &[Cell], column: usize) -> usize {
        let mut end: usize = column + 1;
        while end < cells.len() && cells[end].is_continuation() {
            end += 1;
        }
        return end - column;
    }

    /// Return the row of the cells, filled out to the width with blanks.
    fn row(&self, (mut cells, mut styles): (Vec<Cell>, Vec<Style>), wrapped: bool) -> Line {
        cells.resize(self.columns, Cell::BLANK);
        styles.resize(self.columns, Style::default());
        return Line::of(cells, styles, wrapped);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    /// Return the row of the text, which carries on into the next one when it is wrapped.
    fn line(string: &str, wrapped: bool) -> Line {
        let cells: Vec<Cell> = Cell::all(string);
        let styles: Vec<Style> = vec![Style::default(); cells.len()];
        return Line::of(cells, styles, wrapped);
    }

    /// Return what each row is showing, with a backslash after each one which is wrapped.
    fn shown(rows: &[Line]) -> Vec<String> {
        return rows
            .iter()
            .map(|row| {
                let mut shown: String = row.text(0..row.columns());
                if row.is_wrapped() {
                    shown.push('\\');
                }
                shown
            })
            .collect();
    }

    #[test_case(&[("abcd", true), ("ef  ", false)], 6, &["abcdef"]; "a wrapped line made wider")]
    #[test_case(&[("abcd", true), ("ef  ", false)], 3, &["abc\\", "def"]; "a wrapped line made narrower")]
    #[test_case(&[("abcd", false), ("ef  ", false)], 6, &["abcd  ", "ef    "]; "two lines which were not wrapped stay apart")]
    #[test_case(&[("abcdef", false)], 4, &["abcd\\", "ef  "]; "a line which no longer fits")]
    #[test_case(&[("ab  ", false)], 1, &["a\\", "b"]; "the blanks after what is written are left behind")]
    #[test_case(&[("    ", false)], 2, &["  "]; "a blank line is still a row")]
    #[test_case(&[("abc🦀", false)], 4, &["abc \\", "🦀  "]; "a wide cluster which does not fit goes onto the next row")]
    fn test_of(rows: &[(&str, bool)], columns: usize, expected: &[&str]) {
        let rows: Vec<Line> = rows
            .iter()
            .map(|(string, wrapped)| line(string, *wrapped))
            .collect();

        let wrapped: Vec<Line> = Reflow::of(rows, columns, &mut []);

        assert_eq!(shown(&wrapped), expected);
    }

    #[test_case(Location { row: 1, column: 1 }, 3, Location { row: 1, column: 2 }; "along with what is written in it")]
    #[test_case(Location { row: 2, column: 0 }, 3, Location { row: 2, column: 0 }; "on a line of its own")]
    #[test_case(Location { row: 1, column: 3 }, 6, Location { row: 1, column: 1 }; "past the end of what is written")]
    fn test_of_moves_the_spots(spot: Location, columns: usize, expected: Location) {
        let rows: Vec<Line> = vec![line("abcd", true), line("ef  ", false), line("gh  ", false)];
        let mut spots: [Location; 1] = [spot];

        Reflow::of(rows, columns, &mut spots);

        assert_eq!(spots[0], expected);
    }
}

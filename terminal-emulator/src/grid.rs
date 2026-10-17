/*!
This module contains the [`Grid`] struct which is the rows of a screen, and the [`Line`] struct
which is one of them.
*/
use std::collections::BTreeSet;
use std::ops::Range;

use rend::{Cell, Fabric, Size, Style, Yarn};

/// One row of a screen.
///
/// Every column of it is written in, unlike a [`Yarn`], because a program writes wherever it likes
/// and there is no such thing as a column of a screen which is not there.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Line {
    /// What is written in each column.
    cells: Vec<Cell>,
    /// The way each column is written.
    styles: Vec<Style>,
}

impl Line {
    /// Return a row of the given width with nothing written in it.
    pub fn blank(columns: usize, style: Style) -> Self {
        return Self {
            cells: vec![Cell::BLANK; columns],
            styles: vec![style; columns],
        };
    }

    /// Return how many columns the row has.
    pub fn columns(&self) -> usize {
        return self.cells.len();
    }

    /// Return what is written in the column.
    pub fn cell(&self, column: usize) -> &Cell {
        return self.cells.get(column).unwrap_or(&Cell::BLANK);
    }

    /// Write the cluster in the column, in the way given.
    ///
    /// A cluster which is two columns wide takes the column after it as well, and whichever
    /// cluster was written in either of them is taken away whole rather than being left with only
    /// part of itself written over.
    pub fn set(&mut self, column: usize, cell: Cell, style: Style) {
        let columns: usize = self.columns();
        if column >= columns {
            return;
        }

        let width: usize = cell.width().max(1);
        for offset in 0..width.min(columns - column) {
            self.clear_cluster(column + offset);
        }

        self.cells[column] = cell;
        self.styles[column] = style;
        for offset in 1..width.min(columns - column) {
            self.cells[column + offset] = Cell::Continuation;
            self.styles[column + offset] = style;
        }
    }

    /// Add the character onto the cluster which is written in the column.
    ///
    /// This is what a combining accent does: it is written in no columns of its own and belongs to
    /// whichever cluster was written last.
    pub fn append(&mut self, column: usize, character: char) {
        let head: usize = match self.head(column) {
            Some(head) => head,
            None => return,
        };

        let mut cluster: String = match &self.cells[head] {
            Cell::Char(written) => written.to_string(),
            Cell::Cluster(written) => written.to_string(),
            Cell::Continuation => return,
        };
        cluster.push(character);

        self.cells[head] = Cell::Cluster(cluster.into_boxed_str());
    }

    /// Blank the columns, writing them the way given.
    pub fn erase(&mut self, columns: Range<usize>, style: Style) {
        let columns: Range<usize> = columns.start..columns.end.min(self.columns());
        if columns.is_empty() {
            return;
        }

        self.clear_cluster(columns.start);
        self.clear_cluster(columns.end - 1);

        for column in columns {
            self.cells[column] = Cell::BLANK;
            self.styles[column] = style;
        }
    }

    /// Make room for the given number of blank columns at the column, pushing what is written from
    /// there on along and off the end of the row.
    pub fn insert(&mut self, column: usize, count: usize, style: Style) {
        let columns: usize = self.columns();
        if column >= columns {
            return;
        }
        let count: usize = count.min(columns - column);

        // NOTE: A wide cluster which starts in the column before this one has its second half
        // pushed along while its first half stays put, so it is taken away whole instead.
        if self.cells[column].is_continuation() {
            self.clear_cluster(column);
        }

        self.cells[column..].rotate_right(count);
        self.styles[column..].rotate_right(count);
        self.erase(column..column + count, style);

        // NOTE: A wide cluster which was pushed along may have been pushed half off the end of the
        // row, leaving only its first column written in.
        if self.cells[columns - 1].width() > 1 {
            self.cells[columns - 1] = Cell::BLANK;
        }
    }

    /// Take the given number of columns out at the column, pulling what is written after them back
    /// and bringing blank columns in at the end of the row.
    pub fn delete(&mut self, column: usize, count: usize, style: Style) {
        let columns: usize = self.columns();
        if column >= columns {
            return;
        }
        let count: usize = count.min(columns - column);

        // NOTE: A wide cluster which starts in the column before this one has its second half
        // taken out while its first half stays put, so it is taken away whole instead.
        if self.cells[column].is_continuation() {
            self.clear_cluster(column);
        }

        self.cells[column..].rotate_left(count);
        self.styles[column..].rotate_left(count);
        self.erase(columns - count..columns, style);

        // NOTE: What was pulled back into the column may be the second half of a wide cluster
        // whose first half was taken out.
        if self.cells[column].is_continuation() {
            self.cells[column] = Cell::BLANK;
        }
    }

    /// Change how many columns the row has, blanking any which it gains.
    pub fn resize(&mut self, columns: usize, style: Style) {
        // NOTE: Shortening the cells is what knows to blank a wide cluster whose second column has
        // been cut off, and it leaves them alone when they are being made longer.
        Cell::truncate(&mut self.cells, columns);

        self.cells.resize(columns, Cell::BLANK);
        self.styles.resize(columns, style);
    }

    /// Return the column which the cluster written in the given column starts in.
    fn head(&self, column: usize) -> Option<usize> {
        if column >= self.columns() {
            return None;
        }

        let mut head: usize = column;
        while head > 0 && self.cells[head].is_continuation() {
            head -= 1;
        }

        return Some(head);
    }

    /// Blank the whole of the cluster which is written in the column.
    ///
    /// Writing over only part of a wide cluster would leave the rest of it behind, which would
    /// push everything after it along by a column.
    fn clear_cluster(&mut self, column: usize) {
        let head: usize = match self.head(column) {
            Some(head) => head,
            None => return,
        };

        let mut end: usize = head + 1;
        while end < self.columns() && self.cells[end].is_continuation() {
            end += 1;
        }

        for cell in &mut self.cells[head..end] {
            *cell = Cell::BLANK;
        }
    }
}

impl From<&Line> for Yarn {
    fn from(line: &Line) -> Self {
        let mut yarn = Yarn::from(line.cells.clone());
        for (column, style) in line.styles.iter().enumerate() {
            yarn.style(*style, column..column + 1);
        }
        return yarn;
    }
}

/// The rows of a screen.
#[derive(Debug, Clone)]
pub struct Grid {
    /// The rows.
    lines: Vec<Line>,
    /// How many columns each of them has.
    columns: usize,
    /// Which rows have been written on since they were last asked for.
    ///
    /// This is what keeps a screen from having to be sent somewhere else whole every time a
    /// program writes one character on it.
    damaged: BTreeSet<usize>,
}

// NOTE: Which rows have been written on lately is not part of what a screen is showing, so two of
// them showing the same thing are the same whether or not the same rows are waiting to be sent.
impl PartialEq for Grid {
    fn eq(&self, other: &Self) -> bool {
        return self.lines == other.lines && self.columns == other.columns;
    }
}

impl Eq for Grid {}

impl Grid {
    /// Return a screen of the given size with nothing written on it.
    pub fn blank(size: Size) -> Self {
        return Self {
            lines: vec![Line::blank(size.columns, Style::default()); size.rows],
            columns: size.columns,
            // Nothing has been sent anywhere yet, so all of it is waiting to be.
            damaged: (0..size.rows).collect(),
        };
    }

    /// Return which rows have been written on since they were last asked for, and take them as
    /// having been sent.
    pub fn take_damage(&mut self) -> BTreeSet<usize> {
        return std::mem::take(&mut self.damaged);
    }

    /// Take every row as having been written on.
    pub fn damage_all(&mut self) {
        self.damaged = (0..self.lines.len()).collect();
    }

    /// Take the rows as having been written on.
    fn damage(&mut self, rows: Range<usize>) {
        self.damaged
            .extend(rows.start..rows.end.min(self.lines.len()));
    }

    /// Return the size of the screen.
    pub fn size(&self) -> Size {
        return Size::new(self.lines.len(), self.columns);
    }

    /// Return the given row.
    pub fn line(&self, row: usize) -> &Line {
        return &self.lines[row];
    }

    /// Return the given row to be written on.
    ///
    /// The row is taken as having been written on whether it is or not, since there is no telling
    /// from here what is done with it.
    pub fn line_mut(&mut self, row: usize) -> &mut Line {
        self.damaged.insert(row);
        return &mut self.lines[row];
    }

    /// Blank the rows, writing them the way given.
    pub fn erase(&mut self, rows: Range<usize>, style: Style) {
        let columns: usize = self.columns;
        for row in rows.start..rows.end.min(self.lines.len()) {
            self.lines[row].erase(0..columns, style);
        }
        self.damage(rows);
    }

    /// Move the rows of the region the given number of rows up, or down when that is negative,
    /// bringing blank rows in behind them, and return the rows which were pushed off the top.
    ///
    /// Only rows pushed off the top of the whole screen are given back, since those are the ones
    /// which have scrolled away rather than ones a program is moving about within a region.
    pub fn scroll(&mut self, region: Range<usize>, rows: isize, style: Style) -> Vec<Line> {
        let region: Range<usize> = region.start..region.end.min(self.lines.len());
        if region.is_empty() || rows == 0 {
            return Vec::new();
        }

        let moved: usize = rows.unsigned_abs().min(region.end - region.start);
        let blank: Line = Line::blank(self.columns, style);
        self.damage(region.clone());

        if rows > 0 {
            let off: Vec<Line> = match region.start == 0 {
                true => self.lines[region.start..region.start + moved].to_vec(),
                false => Vec::new(),
            };

            self.lines[region.clone()].rotate_left(moved);
            for row in (region.end - moved)..region.end {
                self.lines[row] = blank.clone();
            }

            return off;
        }

        self.lines[region.clone()].rotate_right(moved);
        for row in region.start..(region.start + moved) {
            self.lines[row] = blank.clone();
        }

        return Vec::new();
    }

    /// Change the size of the screen, blanking whatever it gains, and return the rows which
    /// scrolled off the top of it, oldest first.
    ///
    /// A screen which loses rows loses the ones below the cursor first, since nothing has been
    /// written in them. Whatever is still too many comes off the top and scrolls away, so that
    /// what was written last is what stays on the screen. The rows which come off the bottom are
    /// not given back: they come after the ones still on the screen, and what scrolls off the top
    /// comes before them, so there is nowhere in the history they could go.
    pub fn resize(&mut self, size: Size, style: Style, cursor_row: usize) -> Vec<Line> {
        let scrolled: Vec<Line> = match self.lines.len().checked_sub(size.rows) {
            Some(fewer) if fewer > 0 => {
                let below: usize = self.lines.len().saturating_sub(cursor_row + 1);
                let from_bottom: usize = fewer.min(below);

                self.lines.truncate(self.lines.len() - from_bottom);
                // NOTE: What scrolls off the top is taken before the rows are made narrower, so
                // that the history keeps the whole of what was written in them.
                self.lines.drain(..fewer - from_bottom).collect()
            }
            _ => Vec::new(),
        };

        for line in &mut self.lines {
            line.resize(size.columns, style);
        }
        self.columns = size.columns;

        self.lines
            .resize(size.rows, Line::blank(size.columns, style));

        // Every row of a screen which has changed size has to be sent again.
        self.damage_all();

        return scrolled;
    }
}

impl From<&Grid> for Fabric {
    fn from(grid: &Grid) -> Self {
        return Fabric::from(grid.lines.iter().map(Yarn::from).collect::<Vec<Yarn>>());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    /// Return the row of the text.
    fn line(string: &str) -> Line {
        let mut line = Line::blank(Cell::columns(string), Style::default());
        for (column, cell) in Cell::all(string).into_iter().enumerate() {
            if !cell.is_continuation() {
                line.set(column, cell, Style::default());
            }
        }
        return line;
    }

    /// Return what the row is showing.
    fn shown(line: &Line) -> String {
        return (0..line.columns())
            .map(|column| line.cell(column).to_string())
            .collect();
    }

    #[test_case("abcd", 1, 1, "a cd"; "one column")]
    #[test_case("abcd", 0, 2, "  cd"; "more than one")]
    #[test_case("abcd", 2, 9, "ab"; "further than the end of the row")]
    fn test_erase(string: &str, from: usize, count: usize, expected: &str) {
        let mut line = line(string);

        line.erase(from..from + count, Style::default());

        assert_eq!(shown(&line), format!("{:width$}", expected, width = 4));
    }

    #[test_case("abcd", 1, 1, "a bc"; "one column")]
    #[test_case("abcd", 0, 2, "  ab"; "more than one")]
    #[test_case("abcd", 2, 9, "ab"; "more than there is room for")]
    fn test_insert(string: &str, at: usize, count: usize, expected: &str) {
        let mut line = line(string);

        line.insert(at, count, Style::default());

        assert_eq!(shown(&line), format!("{:width$}", expected, width = 4));
    }

    #[test_case("abcd", 1, 1, "acd"; "one column")]
    #[test_case("abcd", 0, 2, "cd"; "more than one")]
    #[test_case("abcd", 2, 9, "ab"; "more than there is left")]
    fn test_delete(string: &str, at: usize, count: usize, expected: &str) {
        let mut line = line(string);

        line.delete(at, count, Style::default());

        assert_eq!(shown(&line), format!("{:width$}", expected, width = 4));
    }

    #[test]
    fn test_a_wide_cluster_takes_the_column_after_it() {
        let mut line = Line::blank(4, Style::default());

        line.set(1, Cell::Cluster("🦀".into()), Style::default());

        assert_eq!(shown(&line), " 🦀 ");
        assert_eq!(line.cell(2), &Cell::Continuation);
    }

    #[test]
    fn test_writing_over_half_of_a_wide_cluster_takes_all_of_it_away() {
        let mut line = Line::blank(4, Style::default());
        line.set(1, Cell::Cluster("🦀".into()), Style::default());

        line.set(2, Cell::Char('x'), Style::default());

        assert_eq!(shown(&line), "  x ");
    }

    /// Return a row of four columns with a crab written in the middle two.
    fn crab() -> Line {
        let mut line = Line::blank(4, Style::default());
        line.set(0, Cell::Char('a'), Style::default());
        line.set(1, Cell::Cluster("🦀".into()), Style::default());
        line.set(3, Cell::Char('b'), Style::default());
        return line;
    }

    #[test]
    fn test_making_room_in_the_middle_of_a_wide_cluster_takes_it_away() {
        let mut line = crab();

        // The room is made where the second half of the crab is, which would leave its first half
        // behind on its own.
        line.insert(2, 1, Style::default());

        assert_eq!(shown(&line), "a   ");
    }

    #[test]
    fn test_taking_the_second_half_of_a_wide_cluster_out_takes_all_of_it() {
        let mut line = crab();

        line.delete(2, 1, Style::default());

        // The whole crab goes, and then the one column it was asked to take out is taken out, so
        // the blank its first half left behind stays where it was.
        assert_eq!(shown(&line), "a b ");
    }

    #[test]
    fn test_a_wide_cluster_pushed_half_off_the_end_is_taken_away() {
        let mut line = Line::blank(4, Style::default());
        line.set(2, Cell::Cluster("🦀".into()), Style::default());

        line.insert(0, 1, Style::default());

        assert_eq!(shown(&line), "    ");
    }

    #[test]
    fn test_a_combining_accent_belongs_to_the_cluster_before_it() {
        let mut line = Line::blank(2, Style::default());
        line.set(0, Cell::Char('e'), Style::default());

        line.append(0, '\u{301}');

        assert_eq!(line.cell(0), &Cell::Cluster("e\u{301}".into()));
        assert_eq!(line.columns(), 2);
    }

    #[test]
    fn test_the_rows_which_scroll_off_the_top_are_given_back() {
        let mut grid = Grid::blank(Size::new(3, 2));
        grid.line_mut(0).set(0, Cell::Char('a'), Style::default());
        grid.line_mut(1).set(0, Cell::Char('b'), Style::default());

        let off: Vec<Line> = grid.scroll(0..3, 1, Style::default());

        assert_eq!(off.len(), 1);
        assert_eq!(shown(&off[0]), "a ");
        assert_eq!(shown(grid.line(0)), "b ");
        assert_eq!(shown(grid.line(2)), "  ");
    }

    #[test]
    fn test_rows_moved_about_within_a_region_have_not_scrolled_away() {
        let mut grid = Grid::blank(Size::new(4, 2));
        grid.line_mut(1).set(0, Cell::Char('a'), Style::default());

        // The region starts below the top of the screen, so nothing left the screen at all.
        let off: Vec<Line> = grid.scroll(1..4, 1, Style::default());

        assert_eq!(off, vec![]);
    }

    #[test]
    fn test_scrolling_down_brings_blank_rows_in_at_the_top() {
        let mut grid = Grid::blank(Size::new(3, 2));
        grid.line_mut(0).set(0, Cell::Char('a'), Style::default());

        grid.scroll(0..3, -1, Style::default());

        assert_eq!(shown(grid.line(0)), "  ");
        assert_eq!(shown(grid.line(1)), "a ");
    }
}

/*!
This module contains the [`Cell`] enum, which is what one column of a terminal holds.
*/
use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// What one column of a terminal holds.
///
/// A column holds a whole grapheme cluster rather than a character, because a letter with a
/// combining accent on it is several characters written in the one column. Going the other way, a
/// cluster can be two columns wide, which the ones for the East Asian scripts and most emoji are,
/// and the second of those columns is a [`Continuation`](Self::Continuation).
///
/// Keeping one of these per column rather than one per cluster is what lets everything which lays
/// text out go on counting in columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    /// A column holding one character.
    Char(char),
    /// A column holding a grapheme cluster of more than one character, such as a letter with a
    /// combining accent on it or an emoji built out of several code points.
    Cluster(Box<str>),
    /// The second column of the cluster in the column before it, which is two columns wide.
    ///
    /// Nothing is written for it, because writing that cluster already moved the cursor past it.
    Continuation,
}

impl Cell {
    /// A column with nothing in it.
    pub const BLANK: Self = Self::Char(' ');

    /// Return the cells which the string takes up, which is one for each column it is written in.
    ///
    /// A cluster which is written in no columns at all is left out, and one which is written in
    /// more than one is followed by a [`Continuation`](Self::Continuation) for each column after
    /// the first.
    pub fn all(string: &str) -> Vec<Self> {
        let mut cells: Vec<Self> = Vec::with_capacity(string.len());

        for cluster in string.graphemes(true) {
            let width: usize = UnicodeWidthStr::width(cluster);
            if width == 0 {
                continue;
            }

            let mut characters = cluster.chars();
            let cell: Self = match (characters.next(), characters.next()) {
                (Some(character), None) => Self::Char(character),
                _ => Self::Cluster(cluster.into()),
            };

            cells.push(cell);
            for _ in 1..width {
                cells.push(Self::Continuation);
            }
        }

        cells
    }

    /// Return how many columns the string is written in, which is how many cells it takes.
    ///
    /// This is the same as the length of [`all`](Self::all) without building the cells, for when
    /// only the width is wanted. Measuring a string any other way — in characters or in bytes —
    /// does not line up with the columns a yarn is counted in.
    pub fn columns(string: &str) -> usize {
        UnicodeWidthStr::width(string)
    }

    /// Return how many columns the cell is written in.
    pub fn width(&self) -> usize {
        match self {
            Self::Char(character) => UnicodeWidthChar::width(*character).unwrap_or(0),
            Self::Cluster(cluster) => UnicodeWidthStr::width(&**cluster),
            Self::Continuation => 0,
        }
    }

    /// Shorten the cells to the given number of columns.
    ///
    /// The cluster in the last column is replaced with a blank if it is two columns wide, because
    /// the column its second half was in has been cut off and writing it in the one column left
    /// would push everything after it along.
    pub fn truncate(cells: &mut Vec<Self>, len: usize) {
        if len >= cells.len() {
            return;
        }

        cells.truncate(len);

        if let Some(last) = cells.last_mut() {
            if last.width() > 1 {
                *last = Self::BLANK;
            }
        }
    }

    /// Return whether the cell is the second column of a wide cluster.
    pub fn is_continuation(&self) -> bool {
        matches!(self, Self::Continuation)
    }

    /// Keep only the last so many columns of the cells.
    ///
    /// The cluster in the first column is replaced with a blank if it is the second half of one,
    /// because the column its first half was in has been cut off.
    pub fn keep_last(cells: &mut Vec<Self>, len: usize) {
        if len >= cells.len() {
            return;
        }

        cells.drain(..cells.len() - len);

        if let Some(first) = cells.first_mut() {
            if *first == Self::Continuation {
                *first = Self::BLANK;
            }
        }
    }
}

impl Display for Cell {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Char(character) => formatter.write_char(*character),
            Self::Cluster(cluster) => formatter.write_str(cluster),
            Self::Continuation => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test]
    fn test_a_string_of_plain_characters_is_one_cell_each() {
        assert_eq!(
            Cell::all("abc"),
            vec![Cell::Char('a'), Cell::Char('b'), Cell::Char('c')]
        );
    }

    #[test]
    fn test_a_character_with_a_combining_accent_is_one_cell() {
        // An `e` and a combining acute accent, which are written in the one column.
        let cells = Cell::all("e\u{301}");

        assert_eq!(cells, vec![Cell::Cluster("e\u{301}".into())]);
        assert_eq!(cells[0].width(), 1);
    }

    #[test_case("🦀"; "an emoji")]
    #[test_case("😀"; "another emoji")]
    fn test_a_wide_cluster_takes_up_two_cells(string: &str) {
        let cells = Cell::all(string);

        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].width(), 2);
        assert_eq!(cells[1], Cell::Continuation);
    }

    #[test]
    fn test_a_cluster_written_in_no_columns_is_left_out() {
        // A zero width joiner on its own has nothing to join and nowhere to go.
        assert_eq!(Cell::all("\u{200d}"), vec![]);
    }

    #[test]
    fn test_nothing_is_written_for_a_continuation() {
        assert_eq!(Cell::Continuation.to_string(), "");
        assert_eq!(Cell::Char('a').to_string(), "a");
        assert_eq!(Cell::Cluster("e\u{301}".into()).to_string(), "e\u{301}");
    }

    #[test_case("abc"; "plain characters")]
    #[test_case("e\u{301}"; "a character with a combining accent")]
    #[test_case("a🦀b"; "a wide character in among narrow ones")]
    #[test_case("\u{200d}"; "a cluster written in no columns")]
    #[test_case(""; "nothing at all")]
    fn test_the_columns_of_a_string_are_how_many_cells_it_takes(string: &str) {
        assert_eq!(Cell::columns(string), Cell::all(string).len());
    }

    #[test]
    fn test_cutting_a_wide_cluster_in_half_leaves_a_blank() {
        let mut cells = Cell::all("a🦀b");
        assert_eq!(cells.len(), 4);

        // Cut between the two halves of the wide one.
        Cell::truncate(&mut cells, 2);

        assert_eq!(cells, vec![Cell::Char('a'), Cell::BLANK]);
    }

    #[test]
    fn test_cutting_after_a_wide_cluster_leaves_it_alone() {
        let mut cells = Cell::all("a🦀b");

        Cell::truncate(&mut cells, 3);

        assert_eq!(cells.len(), 3);
        assert_eq!(cells[1].width(), 2);
        assert_eq!(cells[2], Cell::Continuation);
    }
}

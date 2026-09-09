/*!
This module contains the [`Text`] struct which is text sent to a terminal along with how many
columns of it the text is written in.
*/
use crate::cell::Cell;

/// Text which is sent to a terminal, along with how many columns of it the text is written in.
///
/// The columns are kept alongside the text because how far a run of text moves the cursor along is
/// asked for over and over while a frame is being shortened, and because measuring two runs once
/// they have been joined together is not the same as adding up what each of them measures.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Text {
    /// What is written.
    string: String,
    /// How many columns it is written in.
    columns: usize,
}

impl Text {
    /// Return what is written.
    pub fn string(&self) -> &str {
        return &self.string;
    }

    /// Return how many columns it is written in.
    pub fn columns(&self) -> usize {
        return self.columns;
    }

    /// Return how many bytes it is sent as.
    pub fn bytes(&self) -> usize {
        return self.string.len();
    }

    /// Return whether there is nothing to write.
    pub fn is_empty(&self) -> bool {
        return self.columns == 0 && self.string.is_empty();
    }

    /// Make room for the given number of columns, which take at least that many bytes to write.
    pub fn reserve(&mut self, columns: usize) {
        self.string.reserve(columns);
    }

    /// Add what is in the cell onto the end of the text.
    ///
    /// A cell which is the second column of the cluster before it has nothing written for it, but
    /// the cluster was written in that column as well as the one before, so the columns still go
    /// up by one for it.
    pub fn push(&mut self, cell: &Cell) {
        match cell {
            Cell::Char(character) => self.string.push(*character),
            Cell::Cluster(cluster) => self.string.push_str(cluster),
            Cell::Continuation => {}
        }

        self.columns += cell.width();
    }

    /// Add the other text onto the end of this one.
    pub fn append(&mut self, text: &Self) {
        self.string.push_str(&text.string);
        self.columns += text.columns;
    }
}

impl From<&str> for Text {
    fn from(string: &str) -> Self {
        Text {
            string: String::from(string),
            columns: Cell::columns(string),
        }
    }
}

impl From<&[Cell]> for Text {
    fn from(cells: &[Cell]) -> Self {
        let mut text = Text::default();
        for cell in cells {
            text.push(cell);
        }
        return text;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case("abc", 3; "plain characters")]
    #[test_case("a🦀b", 4; "a wide character in among narrow ones")]
    #[test_case("e\u{301}", 1; "a character with a combining accent")]
    #[test_case("", 0; "nothing at all")]
    fn test_text_is_measured_in_the_columns_it_is_written_in(
        string: &str,
        expected_columns: usize,
    ) {
        let text = Text::from(string);

        assert_eq!(text.columns(), expected_columns);
        assert_eq!(text.string(), string);
        assert_eq!(text.bytes(), string.len());
    }

    #[test_case(vec![Cell::Char('a'), Cell::Char('b')], "ab", 2; "plain characters")]
    #[test_case(Cell::all("🦀"), "🦀", 2; "a wide cluster and the column it continues into")]
    #[test_case(vec![], "", 0; "no cells at all")]
    fn test_from_cells(cells: Vec<Cell>, expected_string: &str, expected_columns: usize) {
        let text = Text::from(&cells[..]);

        assert_eq!(text.string(), expected_string);
        assert_eq!(text.columns(), expected_columns);
    }

    #[test_case("ab", "cd", "abcd", 4; "plain characters")]
    #[test_case("a", "🦀", "a🦀", 3; "text which is wider than it is long")]
    #[test_case("", "ab", "ab", 2; "onto nothing at all")]
    fn test_append(string: &str, other: &str, expected_string: &str, expected_columns: usize) {
        let mut text = Text::from(string);

        text.append(&Text::from(other));

        assert_eq!(text.string(), expected_string);
        assert_eq!(text.columns(), expected_columns);
    }
}

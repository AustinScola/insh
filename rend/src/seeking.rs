//! Contains the [`Seeking`] struct, which is a phrase being looked for in rows of cells.

use crate::Cell;

/// A phrase being looked for in rows of cells.
///
/// Case is ignored unless the phrase has a capital letter in it, the way tmux and vim with
/// smartcase have it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seeking {
    /// The characters of the phrase.
    phrase: Vec<char>,
    /// Whether a letter matches whichever case it is written in.
    ignoring_case: bool,
}

impl From<&str> for Seeking {
    fn from(phrase: &str) -> Self {
        return Self {
            phrase: phrase.chars().collect(),
            ignoring_case: !phrase.chars().any(char::is_uppercase),
        };
    }
}

impl Seeking {
    /// Return the columns of the row which the phrase starts in, from first to last.
    ///
    /// The phrase is only found within the row, and never carries on from one row onto the next.
    pub fn columns_in(&self, cells: &[Cell]) -> Vec<usize> {
        if self.phrase.is_empty() {
            return Vec::new();
        }

        // NOTE: Each character is taken along with the column it is written in, which is the same
        // column for every character of a cluster and leaves out the column a wide one runs into.
        let written: Vec<(char, usize)> = cells
            .iter()
            .enumerate()
            .flat_map(|(column, cell)| -> Vec<(char, usize)> {
                match cell {
                    Cell::Char(character) => vec![(*character, column)],
                    Cell::Cluster(cluster) => cluster
                        .chars()
                        .map(|character| (character, column))
                        .collect(),
                    Cell::Continuation => Vec::new(),
                }
            })
            .collect();

        let mut columns: Vec<usize> = Vec::new();
        for start in 0..written.len() {
            let found: bool = self.phrase.iter().enumerate().all(|(along, wanted)| {
                written
                    .get(start + along)
                    .is_some_and(|(character, _)| self.matches(*character, *wanted))
            });

            let column: usize = written[start].1;
            if found && columns.last() != Some(&column) {
                columns.push(column);
            }
        }

        return columns;
    }

    /// Return whether the character written is the one which is wanted.
    fn matches(&self, written: char, wanted: char) -> bool {
        return match self.ignoring_case {
            true => written.to_lowercase().eq(wanted.to_lowercase()),
            false => written == wanted,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case("ab", "xabyab", vec![1, 4]; "every place it is")]
    #[test_case("ab", "xAByAb", vec![1, 4]; "whatever the case when it has no capitals")]
    #[test_case("Ab", "xAbyab", vec![1]; "only that case when it has a capital")]
    #[test_case("aa", "aaa", vec![0, 1]; "overlapping")]
    #[test_case("", "abc", vec![]; "nothing at all")]
    #[test_case("z", "abc", vec![]; "nowhere")]
    #[test_case("b", "😀b", vec![2]; "after a wide character")]
    fn test_columns_in(phrase: &str, row: &str, expected: Vec<usize>) {
        assert_eq!(Seeking::from(phrase).columns_in(&Cell::all(row)), expected);
    }
}

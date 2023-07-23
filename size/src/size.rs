/*!
This module contains the [`Size`] struct which is used for representing a 2D size.
*/

/// A 2D size.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Default)]
pub struct Size {
    /// The number of rows.
    pub rows: usize,
    /// The number of columns.
    pub columns: usize,
}

impl Size {
    /// Return a new [`Size`].
    pub fn new(rows: usize, columns: usize) -> Self {
        Size { rows, columns }
    }
}

impl From<(u16, u16)> for Size {
    fn from(tuple: (u16, u16)) -> Self {
        let columns: usize = tuple.0.into();
        let rows: usize = tuple.1.into();
        Size { columns, rows }
    }
}

/*!
This module contains the [`Location`] struct which is used to represent 2D locations.
*/

/// A 2D location.
#[derive(Default)]
pub struct Location {
    /// The vertical component of the location.
    pub row: usize,
    /// The horizontal component of the location.
    pub column: usize,
}

impl Location {
    /// Return a new location.
    #[allow(dead_code)]
    pub fn new(row: usize, column: usize) -> Self {
        Location { row, column }
    }
}

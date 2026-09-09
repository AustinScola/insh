/*!
This module contains the [`Location`] struct which is used to represent 2D locations.
*/
use typed_builder::TypedBuilder;

/// A 2D location.
#[derive(TypedBuilder, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// The vertical component of the location.
    pub row: usize,
    /// The horizontal component of the location.
    pub column: usize,
}

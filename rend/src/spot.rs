/*!
This module contains the [`Spot`] struct which is what one column of a [`Fabric`](crate::Fabric)
holds.
*/
use super::cell::Cell;
use super::style::Style;

use typed_builder::TypedBuilder;

/// What a column with nothing in it holds.
static BLANK: Cell = Cell::BLANK;

/// What one column of a fabric holds: what is written there and the colors it is written in and
/// on.
#[derive(TypedBuilder, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spot<'fabric> {
    /// What is written in the column.
    cell: &'fabric Cell,
    /// The colors it is written in and on.
    #[builder(default)]
    style: Style,
}

impl Spot<'_> {
    /// Return a column with nothing in it, written in the colors which the terminal uses when none
    /// have been picked.
    pub fn blank() -> Spot<'static> {
        return Spot::builder().cell(&BLANK).build();
    }

    /// Return what is written in the column.
    pub fn cell(&self) -> &Cell {
        return self.cell;
    }

    /// Return the colors it is written in and on.
    pub fn style(&self) -> Style {
        return self.style;
    }
}

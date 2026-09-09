/*!
This module contains the [`Row`] struct which is one row of a [`Fabric`](crate::Fabric).
*/
use super::cell::Cell;
use super::spot::Spot;
use super::style::Style;

use ansi::Color;

use typed_builder::TypedBuilder;

/// One row of a fabric.
///
/// A row is taken once and then asked about a column at a time, which is what keeps looking one up
/// from having to find the row it is in first.
#[derive(TypedBuilder, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row<'fabric> {
    /// What is written in each column.
    cells: &'fabric [Cell],
    /// The colors the text is written in.
    #[builder(default = &[])]
    colors: &'fabric [Option<Color>],
    /// The colors the text is written on.
    #[builder(default = &[])]
    backgrounds: &'fabric [Option<Color>],
}

impl<'fabric> Row<'fabric> {
    /// Return what is in the given column, which is a blank with no colors of its own for a column
    /// off the end of the row.
    pub fn spot(&self, column: usize) -> Spot<'fabric> {
        let cell: &'fabric Cell = match self.cells.get(column) {
            Some(cell) => cell,
            None => return Spot::blank(),
        };

        // NOTE: The colors are allowed to be given for fewer columns than there are cells, and a
        // column off the end of them has no color of its own.
        let style: Style = Style::builder()
            .color(self.colors.get(column).copied().flatten())
            .background(self.backgrounds.get(column).copied().flatten())
            .build();

        return Spot::builder().cell(cell).style(style).build();
    }
}

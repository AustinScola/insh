/*!
This module contains the [`Engine`] enum which is how much of a fabric is drawn.
*/

/// How much of a fabric is drawn.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// Every column of every row is drawn, whatever the terminal is already showing.
    Full,
    /// Only the columns which the terminal is not already showing are drawn.
    #[default]
    Incremental,
}

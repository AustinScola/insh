/*!
This module contains the [`Cursor`] struct which is where a program is writing on a screen.
*/
use rend::{Location, Style};

/// Where a program is writing on a screen and the way it is writing there.
///
/// These are saved and put back together, so they are the one thing rather than two: a program
/// which saves the cursor and writes in another color expects to be writing in the first one
/// again once it has put the cursor back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    /// Where it is.
    pub at: Location,
    /// The way what is written there is written.
    pub style: Style,
    /// Whether the next cluster written goes onto the row below first.
    ///
    /// A cluster written into the last column of a row leaves the cursor in that column rather
    /// than past it, since there is no column past it to be in, and the next one wraps. Without
    /// this, a row which is written right the way across would wrap a column too early.
    pub wrapping: bool,
}

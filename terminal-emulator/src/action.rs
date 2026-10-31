/*!
This module contains the [`Action`] enum which is one thing a program has said to do.
*/
use ansi::{AnsiEscapeSequence, ControlCharacter, ControlFunction};

/// One thing which the bytes a program wrote say to do.
///
/// A sequence which is not one of the control functions which are known is kept as the sequence
/// itself rather than being thrown away, the same way as in `ansi`, so that nothing a program says
/// goes missing without a screen having had the chance to make something of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Write the character where the cursor is.
    Print(char),
    /// One of the controls which are written as a character below the printable ones.
    Control(ControlCharacter),
    /// A control function.
    Function(ControlFunction),
    /// A sequence which is not one of the control functions which are known.
    Sequence(AnsiEscapeSequence),
}

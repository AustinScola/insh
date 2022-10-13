/*!
This module contains an enum [`ASCII`] for different ASCII codes.
*/
use std::fmt::{Display, Formatter, Result as FmtResult};

/// ASCII codes.
#[repr(u8)]
pub enum ASCII {
    /// The code for a bell.
    Bell = 0x7,
}

impl Display for ASCII {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        // ASCII is always valid UTF-8 and so we don't need to check it.
        let bytes = &[*self as u8];
        let string: &str = unsafe { std::str::from_utf8_unchecked(bytes) };

        write!(formatter, "{}", string)
    }
}

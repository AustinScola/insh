/*!
The controls which are written as one of the characters below the printable ones.
*/

/// A control which is written as one of the characters below the printable ones.
///
/// These are the C0 set of ECMA-48 clause 5.2, along with delete, which belongs to no set at all.
/// Only the ones a terminal makes something of are named; the rest are
/// [`Unknown`](Self::Unknown), which keeps the byte rather than throwing it away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCharacter {
    /// Nothing at all (`0x00`).
    Null,
    /// Ring the bell (`0x07`).
    Bell,
    /// Move the cursor back one column (`0x08`).
    Backspace,
    /// Move the cursor on to the next tab stop (`0x09`).
    CharacterTabulation,
    /// Move the cursor down a row (`0x0a`).
    LineFeed,
    /// Move the cursor on to the next vertical tab stop (`0x0b`).
    LineTabulation,
    /// Move the cursor on to the next page, which a terminal takes as a line feed (`0x0c`).
    FormFeed,
    /// Move the cursor to the first column of the row it is in (`0x0d`).
    CarriageReturn,
    /// Write what follows in the character set designated as the second one (`0x0e`).
    ///
    /// NOTE: ECMA-48 calls this locking-shift one and the terminals which send it call it shift
    /// out.
    LockingShiftOne,
    /// Write what follows in the character set designated as the first one (`0x0f`).
    ///
    /// NOTE: ECMA-48 calls this locking-shift zero and the terminals which send it call it shift
    /// in.
    LockingShiftZero,
    /// Throw away the sequence which is being read (`0x18`).
    Cancel,
    /// Throw away the sequence which is being read, which was wrong (`0x1a`).
    Substitute,
    /// The start of an escape sequence (`0x1b`).
    Escape,
    /// Nothing at all, which is what rubbing a character out left behind (`0x7f`).
    Delete,
    /// One of the controls which a terminal makes nothing of.
    Unknown(u8),
}

impl ControlCharacter {
    /// The byte just past the last of the controls which come below the printable characters.
    const PRINTABLE: u8 = 0x20;

    /// The byte which rubbing a character out left behind.
    const DELETE: u8 = 0x7f;

    /// Return whether the byte is a control rather than part of a printable character.
    pub fn is_control(byte: u8) -> bool {
        return byte < Self::PRINTABLE || byte == Self::DELETE;
    }
}

impl From<u8> for ControlCharacter {
    fn from(byte: u8) -> Self {
        return match byte {
            0x00 => Self::Null,
            0x07 => Self::Bell,
            0x08 => Self::Backspace,
            0x09 => Self::CharacterTabulation,
            0x0a => Self::LineFeed,
            0x0b => Self::LineTabulation,
            0x0c => Self::FormFeed,
            0x0d => Self::CarriageReturn,
            0x0e => Self::LockingShiftOne,
            0x0f => Self::LockingShiftZero,
            0x18 => Self::Cancel,
            0x1a => Self::Substitute,
            0x1b => Self::Escape,
            0x7f => Self::Delete,
            byte => Self::Unknown(byte),
        };
    }
}

impl From<ControlCharacter> for u8 {
    fn from(control: ControlCharacter) -> Self {
        return match control {
            ControlCharacter::Null => 0x00,
            ControlCharacter::Bell => 0x07,
            ControlCharacter::Backspace => 0x08,
            ControlCharacter::CharacterTabulation => 0x09,
            ControlCharacter::LineFeed => 0x0a,
            ControlCharacter::LineTabulation => 0x0b,
            ControlCharacter::FormFeed => 0x0c,
            ControlCharacter::CarriageReturn => 0x0d,
            ControlCharacter::LockingShiftOne => 0x0e,
            ControlCharacter::LockingShiftZero => 0x0f,
            ControlCharacter::Cancel => 0x18,
            ControlCharacter::Substitute => 0x1a,
            ControlCharacter::Escape => 0x1b,
            ControlCharacter::Delete => 0x7f,
            ControlCharacter::Unknown(byte) => byte,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case(0x00, ControlCharacter::Null; "null")]
    #[test_case(0x07, ControlCharacter::Bell; "the bell")]
    #[test_case(0x0a, ControlCharacter::LineFeed; "a line feed")]
    #[test_case(0x0d, ControlCharacter::CarriageReturn; "a carriage return")]
    #[test_case(0x1b, ControlCharacter::Escape; "an escape")]
    #[test_case(0x7f, ControlCharacter::Delete; "a delete")]
    #[test_case(0x01, ControlCharacter::Unknown(0x01); "one a terminal makes nothing of")]
    fn test_a_control_is_read_from_the_byte_it_is_written_as(
        byte: u8,
        expected_control: ControlCharacter,
    ) {
        assert_eq!(ControlCharacter::from(byte), expected_control);
    }

    #[test]
    fn test_every_control_is_written_as_the_byte_it_was_read_from() {
        for byte in (0..=ControlCharacter::PRINTABLE).chain([ControlCharacter::DELETE]) {
            assert_eq!(u8::from(ControlCharacter::from(byte)), byte);
        }
    }

    #[test_case(0x00, true; "the first control")]
    #[test_case(0x1f, true; "the last control below the printable characters")]
    #[test_case(0x20, false; "a space, which is the first printable character")]
    #[test_case(b'a', false; "a letter")]
    #[test_case(0x7f, true; "a delete")]
    #[test_case(0x80, false; "a byte which carries part of a character")]
    fn test_which_bytes_are_controls(byte: u8, expected: bool) {
        assert_eq!(ControlCharacter::is_control(byte), expected);
    }
}

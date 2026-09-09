/*!
This module contains the [`Instruction`] enum which is one of the things sent to a terminal to draw
a frame.
*/
use super::text::Text;

use ansi::ControlFunction;

/// One of the things which is sent to a terminal to draw a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    /// A control function, which says where what follows goes or how it is written.
    Function(ControlFunction),
    /// Text, which is written where the cursor is and moves it along.
    Text(Text),
}

impl Instruction {
    /// Write the bytes which the instruction is sent as onto the end of the given ones.
    pub fn write(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Function(function) => bytes.extend(Vec::from(function)),
            Self::Text(text) => bytes.extend_from_slice(text.string().as_bytes()),
        }
    }

    /// Return how many bytes the instruction is sent as.
    pub fn bytes(&self) -> usize {
        return match self {
            Self::Function(function) => Self::function_bytes(function),
            Self::Text(text) => text.bytes(),
        };
    }

    /// Return how many bytes the control function is sent as.
    ///
    /// The ones which a frame is drawn with are counted rather than written, because how long one
    /// of them would be is asked over and over while a frame is being shortened and writing one
    /// means putting a string together.
    fn function_bytes(function: &ControlFunction) -> usize {
        // The escape, the bracket and the byte on the end of a control sequence.
        const SEQUENCE: usize = 3;

        return match function {
            ControlFunction::CursorPosition { row, column } => {
                SEQUENCE
                    + match column {
                        // A parameter which is the one it would be anyway is left out, and so is
                        // the separator before a last one which is left out.
                        1 => Self::parameter_bytes(*row, 1),
                        column => Self::parameter_bytes(*row, 1) + 1 + Self::digits(*column),
                    }
            }
            ControlFunction::CursorColumn(count)
            | ControlFunction::CursorForward(count)
            | ControlFunction::CursorBack(count)
            | ControlFunction::CursorDown(count)
            | ControlFunction::CursorNextLine(count)
            | ControlFunction::ScrollUp(count)
            | ControlFunction::ScrollDown(count) => SEQUENCE + Self::parameter_bytes(*count, 1),
            ControlFunction::EraseInLine(erase) => {
                SEQUENCE + Self::parameter_bytes(u16::from(*erase), 0)
            }
            ControlFunction::SetScrollingRegion { top, bottom } => {
                SEQUENCE
                    + match bottom {
                        None => Self::parameter_bytes(*top, 1),
                        Some(bottom) => Self::parameter_bytes(*top, 1) + 1 + Self::digits(*bottom),
                    }
            }
            function => Vec::from(function).len(),
        };
    }

    /// Return how many bytes the parameter of a control sequence is sent as, which is none at all
    /// when it is the one the sequence would have anyway.
    fn parameter_bytes(value: u16, default: u16) -> usize {
        return match value == default {
            true => 0,
            false => Self::digits(value),
        };
    }

    /// Return how many digits the number is written in.
    fn digits(value: u16) -> usize {
        return match value {
            0..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            _ => 5,
        };
    }
}

impl From<ControlFunction> for Instruction {
    fn from(function: ControlFunction) -> Self {
        Instruction::Function(function)
    }
}

impl From<Text> for Instruction {
    fn from(text: Text) -> Self {
        Instruction::Text(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ansi::{Color, EraseInLine, GraphicRendition};

    use test_case::test_case;

    #[test_case(Instruction::from(ControlFunction::CursorPosition { row: 1, column: 1 }), 3; "a cursor position with both of its parameters left out")]
    #[test_case(Instruction::from(ControlFunction::CursorPosition { row: 2, column: 1 }), 4; "a cursor position with only the row")]
    #[test_case(Instruction::from(ControlFunction::CursorPosition { row: 1, column: 5 }), 5; "a cursor position with only the column")]
    #[test_case(Instruction::from(ControlFunction::CursorPosition { row: 10, column: 5 }), 7; "a cursor position with both of its parameters")]
    #[test_case(Instruction::from(ControlFunction::CursorPosition { row: 100, column: 1000 }), 11; "a cursor position off the end of a big screen")]
    #[test_case(Instruction::from(ControlFunction::CursorColumn(1)), 3; "the first column")]
    #[test_case(Instruction::from(ControlFunction::CursorColumn(9)), 4; "a column")]
    #[test_case(Instruction::from(ControlFunction::CursorForward(1)), 3; "one column along")]
    #[test_case(Instruction::from(ControlFunction::CursorForward(10)), 5; "some columns along")]
    #[test_case(Instruction::from(ControlFunction::CursorBack(2)), 4; "some columns back")]
    #[test_case(Instruction::from(ControlFunction::CursorDown(3)), 4; "some rows down")]
    #[test_case(Instruction::from(ControlFunction::CursorNextLine(1)), 3; "the next row")]
    #[test_case(Instruction::from(ControlFunction::ScrollUp(1)), 3; "scrolling up a row")]
    #[test_case(Instruction::from(ControlFunction::ScrollDown(2)), 4; "scrolling down some rows")]
    #[test_case(Instruction::from(ControlFunction::EraseInLine(EraseInLine::ToEnd)), 3; "erasing the rest of a row")]
    #[test_case(Instruction::from(ControlFunction::SetScrollingRegion { top: 1, bottom: None }), 3; "the rows scrolling happens between put back")]
    #[test_case(Instruction::from(ControlFunction::SetScrollingRegion { top: 2, bottom: Some(23) }), 7; "the rows scrolling happens between")]
    #[test_case(Instruction::from(ControlFunction::SelectGraphicRendition(vec![GraphicRendition::Reset])), 3; "a reset")]
    #[test_case(Instruction::from(ControlFunction::SelectGraphicRendition(vec![GraphicRendition::Foreground(Color::Red), GraphicRendition::Background(Color::Blue)])), 8; "both colors")]
    #[test_case(Instruction::from(Text::from("abc")), 3; "text")]
    #[test_case(Instruction::from(Text::from("🦀")), 4; "text of more bytes than columns")]
    fn test_bytes(instruction: Instruction, expected_bytes: usize) {
        let mut written: Vec<u8> = Vec::new();
        instruction.write(&mut written);

        assert_eq!(instruction.bytes(), expected_bytes);
        assert_eq!(written.len(), expected_bytes);
    }
}

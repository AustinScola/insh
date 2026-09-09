/*!
This module contains the [`Frame`] struct which is everything sent to a terminal to draw one
fabric.
*/
use super::instruction::Instruction;

/// Everything which is sent to a terminal to draw one fabric.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    /// The instructions, in the order they are sent.
    instructions: Vec<Instruction>,
}

impl Frame {
    /// Return the instructions, in the order they are sent.
    pub fn instructions(&self) -> &Vec<Instruction> {
        return &self.instructions;
    }

    /// Return how many bytes the frame is sent as.
    pub fn bytes(&self) -> usize {
        return self
            .instructions
            .iter()
            .map(|instruction| instruction.bytes())
            .sum();
    }
}

impl From<Vec<Instruction>> for Frame {
    fn from(instructions: Vec<Instruction>) -> Self {
        Frame { instructions }
    }
}

impl From<Frame> for Vec<Instruction> {
    fn from(frame: Frame) -> Self {
        frame.instructions
    }
}

impl From<&Frame> for Vec<u8> {
    fn from(frame: &Frame) -> Self {
        let mut bytes: Vec<u8> = Vec::with_capacity(frame.bytes());
        for instruction in &frame.instructions {
            instruction.write(&mut bytes);
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::renderer::text::Text;

    use ansi::ControlFunction;

    use test_case::test_case;

    #[test_case(vec![], ""; "nothing at all")]
    #[test_case(vec![Instruction::from(Text::from("ab"))], "ab"; "only text")]
    #[test_case(
        vec![
            Instruction::from(ControlFunction::CursorPosition { row: 2, column: 1 }),
            Instruction::from(Text::from("ab")),
        ],
        "\x1b[2Hab";
        "text written where the cursor was moved to"
    )]
    fn test_the_bytes_of_a_frame_are_the_bytes_of_its_instructions(
        instructions: Vec<Instruction>,
        expected_bytes: &str,
    ) {
        let frame = Frame::from(instructions);

        assert_eq!(Vec::from(&frame), expected_bytes.as_bytes());
        assert_eq!(frame.bytes(), expected_bytes.len());
    }
}

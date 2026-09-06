/*!
Parsing of the escape sequences defined by ECMA-48, which both the terminal and the programs
running in it send.

This is only the structure of a sequence and not what it means: a sequence is taken apart into its
private marker, parameters, intermediates, final byte and string payload, and working out that, say,
a control sequence ending in `H` moves the cursor is left to whatever is interpreting them. Knowing
where a sequence ends is enough to pass one through without understanding it, which is what matters
for anything sitting between a terminal and a program.

Only the seven bit forms are parsed. The eight bit forms of the C1 controls (a control sequence
introduced by `0x9b` rather than by `ESC [`, for example) are not, because those bytes are part of a
character when the terminal is in UTF-8 mode, which is the only mode this handles.
*/

use std::mem;

/// The escape character, which begins every escape sequence.
pub const ESCAPE: u8 = 0x1b;

/// The bell character, which terminates an operating system command.
pub const BELL: u8 = 0x07;

/// The string terminator, which ends every sequence carrying a string.
pub const STRING_TERMINATOR: &[u8] = b"\x1b\\";

/// The sequences which a terminal wraps pasted text in when it has been asked to, so that the text
/// can be told apart from text which is typed.
///
/// Asking it to is turning on [`Mode::BracketedPaste`](crate::Mode::BracketedPaste).
pub struct BracketedPaste;

impl BracketedPaste {
    /// Marks the start of pasted text.
    pub const START: &'static [u8] = b"\x1b[200~";
    /// Marks the end of pasted text.
    pub const END: &'static [u8] = b"\x1b[201~";
}

/// An escape sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnsiEscapeSequence {
    /// A control sequence: `ESC [`, then a control.
    ControlSequence(ControlSequence),
    /// An operating system command: `ESC ]`, then a string. These are what set the title of the
    /// window and read and write the clipboard, among other things.
    OperatingSystemCommand(Vec<u8>),
    /// A device control string: `ESC P`, then a control, then a string.
    DeviceControlString(DeviceControlString),
    /// An application program command: `ESC _`, then a string.
    ApplicationProgramCommand(Vec<u8>),
    /// A privacy message: `ESC ^`, then a string.
    PrivacyMessage(Vec<u8>),
    /// A start of string: `ESC X`, then a string.
    StartOfString(Vec<u8>),
    /// Single shift two: `ESC N`, then the byte which it shifts.
    SingleShiftTwo(u8),
    /// Single shift three: `ESC O`, then the byte which it shifts.
    SingleShiftThree(u8),
    /// Any other escape sequence: `ESC`, then intermediates, then a final byte. These are the
    /// short ones, such as `ESC c` to reset the terminal or `ESC ( B` to pick a character set.
    Escape {
        /// The bytes between the escape and the final byte, each of which is `0x20` to `0x2f`.
        intermediates: Vec<u8>,
        /// The byte which says what the sequence is, which is `0x30` to `0x7e`.
        final_byte: u8,
    },
}

/// The private marker, parameters, intermediates and final byte which a control sequence is made
/// of. The header of a device control string is made of the same parts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlSequence {
    /// The byte before the parameters which marks the sequence as private to a terminal rather
    /// than standard, if there is one. It is one of `<`, `=`, `>` or `?`.
    pub private: Option<u8>,
    /// The parameters, which are separated by semicolons.
    pub parameters: Vec<Parameter>,
    /// The bytes between the parameters and the final byte, each of which is `0x20` to `0x2f`.
    pub intermediates: Vec<u8>,
    /// The byte which says what the sequence is, which is `0x40` to `0x7e`.
    pub final_byte: u8,
}

impl ControlSequence {
    /// Return the value of the parameter at the given position, or `None` if there is no parameter
    /// there or it was left out so that the default for the sequence is meant.
    pub fn parameter(&self, position: usize) -> Option<u16> {
        self.parameters
            .get(position)
            .and_then(|parameter| parameter.value)
    }

    /// Return the value of the parameter at the given position, or the given default if there is
    /// no parameter there or it was left out.
    pub fn parameter_or(&self, position: usize, default: u16) -> u16 {
        self.parameter(position).unwrap_or(default)
    }

    /// Return the value of the parameter at the given position, or the given default if there is
    /// no parameter there or it was left out or it is a zero. Most of the sequences which take a
    /// count treat a zero as if it were a one.
    pub fn count(&self, position: usize, default: u16) -> u16 {
        match self.parameter(position) {
            Some(0) | None => default,
            Some(value) => value,
        }
    }
}

/// A control sequence parameter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parameter {
    /// The value, which is `None` when it was left out so that the default for the sequence is
    /// meant.
    pub value: Option<u16>,
    /// The sub-parameters, which follow the value separated by colons. A colour given as its
    /// components is the one of these which is used at all commonly.
    pub subs: Vec<Option<u16>>,
}

/// A device control string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceControlString {
    /// The parameters and final byte which say what the string is for.
    pub control: ControlSequence,
    /// The bytes between the control and the string terminator.
    pub payload: Vec<u8>,
}

impl From<&AnsiEscapeSequence> for Vec<u8> {
    fn from(sequence: &AnsiEscapeSequence) -> Self {
        match sequence {
            AnsiEscapeSequence::ControlSequence(control) => {
                let mut bytes: Self = vec![ESCAPE, b'['];
                control.write(&mut bytes);
                bytes
            }
            AnsiEscapeSequence::DeviceControlString(string) => {
                let mut bytes: Self = vec![ESCAPE, b'P'];
                string.control.write(&mut bytes);
                bytes.extend_from_slice(&string.payload);
                bytes.extend_from_slice(STRING_TERMINATOR);
                bytes
            }
            // NOTE: A bell ends an operating system command in the one byte where a string
            // terminator takes two, and every terminal which understands these understands it.
            AnsiEscapeSequence::OperatingSystemCommand(payload) => {
                let mut bytes: Self = vec![ESCAPE, b']'];
                bytes.extend_from_slice(payload);
                bytes.push(BELL);
                bytes
            }
            AnsiEscapeSequence::ApplicationProgramCommand(payload) => {
                AnsiEscapeSequence::write_string(b'_', payload)
            }
            AnsiEscapeSequence::PrivacyMessage(payload) => {
                AnsiEscapeSequence::write_string(b'^', payload)
            }
            AnsiEscapeSequence::StartOfString(payload) => {
                AnsiEscapeSequence::write_string(b'X', payload)
            }
            AnsiEscapeSequence::SingleShiftTwo(shifted) => vec![ESCAPE, b'N', *shifted],
            AnsiEscapeSequence::SingleShiftThree(shifted) => vec![ESCAPE, b'O', *shifted],
            AnsiEscapeSequence::Escape {
                intermediates,
                final_byte,
            } => {
                let mut bytes: Self = vec![ESCAPE];
                bytes.extend_from_slice(intermediates);
                bytes.push(*final_byte);
                bytes
            }
        }
    }
}

impl AnsiEscapeSequence {
    /// Return the bytes of a sequence which carries a string, given the byte which introduces it.
    fn write_string(introducer: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![ESCAPE, introducer];
        bytes.extend_from_slice(payload);
        bytes.extend_from_slice(STRING_TERMINATOR);
        bytes
    }
}

impl ControlSequence {
    /// Write the private marker, parameters, intermediates and final byte onto the given bytes.
    fn write(&self, bytes: &mut Vec<u8>) {
        if let Some(private) = self.private {
            bytes.push(private);
        }

        for (position, parameter) in self.parameters.iter().enumerate() {
            if position > 0 {
                bytes.push(b';');
            }
            parameter.write(bytes);
        }

        bytes.extend_from_slice(&self.intermediates);
        bytes.push(self.final_byte);
    }
}

impl Parameter {
    /// Write the value and sub-parameters onto the given bytes. A value which is not there is
    /// written as nothing at all, which is how a parameter says to use the default.
    fn write(&self, bytes: &mut Vec<u8>) {
        if let Some(value) = self.value {
            bytes.extend_from_slice(value.to_string().as_bytes());
        }

        for sub in &self.subs {
            bytes.push(b':');
            if let Some(sub) = sub {
                bytes.extend_from_slice(sub.to_string().as_bytes());
            }
        }
    }
}

/// An escape sequence along with the number of bytes which it was parsed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAnsiEscapeSequence {
    pub sequence: AnsiEscapeSequence,
    pub len: usize,
}

impl TryFrom<&[u8]> for ParsedAnsiEscapeSequence {
    type Error = AnsiEscapeSequenceParseError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.first() != Some(&ESCAPE) {
            return Err(AnsiEscapeSequenceParseError::NotAnEscapeSequence);
        }

        let byte: u8 = match bytes.get(1) {
            Some(byte) => *byte,
            None => {
                return Err(AnsiEscapeSequenceParseError::Need(2));
            }
        };

        match byte {
            b'[' => {
                let (control, len) = Self::parse_control(bytes, 2)?;
                Ok(Self {
                    sequence: AnsiEscapeSequence::ControlSequence(control),
                    len,
                })
            }
            b'P' => {
                let (control, payload_start) = Self::parse_control(bytes, 2)?;
                let (payload, len) = Self::parse_string(bytes, payload_start, false)?;
                Ok(Self {
                    sequence: AnsiEscapeSequence::DeviceControlString(DeviceControlString {
                        control,
                        payload,
                    }),
                    len,
                })
            }
            // NOTE: The bell terminating a string is an extension which only applies to operating
            // system commands, and is what most of the programs which send one use.
            b']' => {
                let (payload, len) = Self::parse_string(bytes, 2, true)?;
                Ok(Self {
                    sequence: AnsiEscapeSequence::OperatingSystemCommand(payload),
                    len,
                })
            }
            b'_' => {
                let (payload, len) = Self::parse_string(bytes, 2, false)?;
                Ok(Self {
                    sequence: AnsiEscapeSequence::ApplicationProgramCommand(payload),
                    len,
                })
            }
            b'^' => {
                let (payload, len) = Self::parse_string(bytes, 2, false)?;
                Ok(Self {
                    sequence: AnsiEscapeSequence::PrivacyMessage(payload),
                    len,
                })
            }
            b'X' => {
                let (payload, len) = Self::parse_string(bytes, 2, false)?;
                Ok(Self {
                    sequence: AnsiEscapeSequence::StartOfString(payload),
                    len,
                })
            }
            b'N' | b'O' => {
                let shifted: u8 = match bytes.get(2) {
                    Some(shifted) => *shifted,
                    None => {
                        return Err(AnsiEscapeSequenceParseError::Need(3));
                    }
                };
                let sequence: AnsiEscapeSequence = match byte {
                    b'N' => AnsiEscapeSequence::SingleShiftTwo(shifted),
                    _ => AnsiEscapeSequence::SingleShiftThree(shifted),
                };
                Ok(Self { sequence, len: 3 })
            }
            _ => Self::parse_escape(bytes),
        }
    }
}

impl ParsedAnsiEscapeSequence {
    /// The bytes which a sequence may use as intermediates.
    const INTERMEDIATES: std::ops::RangeInclusive<u8> = 0x20..=0x2f;

    /// Parse the private marker, parameters, intermediates and final byte which start at the given
    /// position, returning them along with the position just past the final byte.
    fn parse_control(
        bytes: &[u8],
        start: usize,
    ) -> Result<(ControlSequence, usize), AnsiEscapeSequenceParseError> {
        let mut position: usize = start;

        // The private marker comes before the parameters when there is one.
        let private: Option<u8> = match bytes.get(position) {
            Some(byte @ (b'<' | b'=' | b'>' | b'?')) => {
                position += 1;
                Some(*byte)
            }
            Some(_) => None,
            None => {
                return Err(AnsiEscapeSequenceParseError::Need(position + 1));
            }
        };

        let parameters_start: usize = position;
        let mut parameters: Vec<Parameter> = Vec::new();
        let mut parameter: Parameter = Parameter::default();
        let mut value: Option<u16> = None;
        // Whether what is being read is a sub-parameter rather than the value of a parameter.
        let mut sub: bool = false;

        loop {
            let byte: u8 = match bytes.get(position) {
                Some(byte) => *byte,
                None => {
                    return Err(AnsiEscapeSequenceParseError::Need(position + 1));
                }
            };

            match byte {
                b'0'..=b'9' => {
                    value = Some(
                        value
                            .unwrap_or(0)
                            .saturating_mul(10)
                            .saturating_add(u16::from(byte - b'0')),
                    );
                }
                b':' => {
                    if sub {
                        parameter.subs.push(value.take());
                    } else {
                        parameter.value = value.take();
                        sub = true;
                    }
                }
                b';' => {
                    if sub {
                        parameter.subs.push(value.take());
                    } else {
                        parameter.value = value.take();
                    }
                    parameters.push(mem::take(&mut parameter));
                    sub = false;
                }
                _ => {
                    break;
                }
            }

            position += 1;
        }

        // The parameter which is left over after the last separator, if there were any parameters.
        if position > parameters_start {
            if sub {
                parameter.subs.push(value);
            } else {
                parameter.value = value;
            }
            parameters.push(parameter);
        }

        let intermediates_start: usize = position;
        loop {
            match bytes.get(position) {
                Some(byte) if Self::INTERMEDIATES.contains(byte) => {
                    position += 1;
                }
                Some(_) => {
                    break;
                }
                None => {
                    return Err(AnsiEscapeSequenceParseError::Need(position + 1));
                }
            }
        }
        let intermediates: Vec<u8> = bytes[intermediates_start..position].to_vec();

        let final_byte: u8 = match bytes.get(position) {
            Some(byte) if (0x40..=0x7e).contains(byte) => *byte,
            // Anything else in the place of the final byte means the sequence is malformed.
            Some(_) => {
                return Err(AnsiEscapeSequenceParseError::Unrecognized(position + 1));
            }
            None => {
                return Err(AnsiEscapeSequenceParseError::Need(position + 1));
            }
        };

        let control = ControlSequence {
            private,
            parameters,
            intermediates,
            final_byte,
        };

        Ok((control, position + 1))
    }

    /// Parse the string which starts at the given position, returning it along with the number of
    /// bytes of the whole sequence which it is the end of.
    fn parse_string(
        bytes: &[u8],
        start: usize,
        bell_terminates: bool,
    ) -> Result<(Vec<u8>, usize), AnsiEscapeSequenceParseError> {
        let mut position: usize = start;

        loop {
            let byte: u8 = match bytes.get(position) {
                Some(byte) => *byte,
                None => {
                    return Err(AnsiEscapeSequenceParseError::NeedTerminator);
                }
            };

            match byte {
                ESCAPE => {
                    return match bytes.get(position + 1) {
                        // The string terminator, which is an escape and a backslash.
                        Some(b'\\') => Ok((bytes[start..position].to_vec(), position + 2)),
                        // NOTE: An escape which does not begin the terminator ends the string as
                        // well, because it is the start of another sequence and so the terminator
                        // is never going to turn up. Waiting for one would mean a string which was
                        // cut short swallowed everything after it.
                        Some(_) => Ok((bytes[start..position].to_vec(), position)),
                        None => Err(AnsiEscapeSequenceParseError::NeedTerminator),
                    };
                }
                BELL if bell_terminates => {
                    return Ok((bytes[start..position].to_vec(), position + 1));
                }
                _ => {
                    position += 1;
                }
            }
        }
    }

    /// Parse an escape sequence which is made of nothing but intermediates and a final byte.
    fn parse_escape(bytes: &[u8]) -> Result<Self, AnsiEscapeSequenceParseError> {
        let mut position: usize = 1;

        loop {
            match bytes.get(position) {
                Some(byte) if Self::INTERMEDIATES.contains(byte) => {
                    position += 1;
                }
                Some(_) => {
                    break;
                }
                None => {
                    return Err(AnsiEscapeSequenceParseError::Need(position + 1));
                }
            }
        }
        let intermediates: Vec<u8> = bytes[1..position].to_vec();

        let final_byte: u8 = match bytes.get(position) {
            Some(byte) if (0x30..=0x7e).contains(byte) => *byte,
            Some(_) => {
                return Err(AnsiEscapeSequenceParseError::Unrecognized(position + 1));
            }
            None => {
                return Err(AnsiEscapeSequenceParseError::Need(position + 1));
            }
        };

        Ok(Self {
            sequence: AnsiEscapeSequence::Escape {
                intermediates,
                final_byte,
            },
            len: position + 1,
        })
    }
}

/// A problem with parsing an escape sequence.
#[derive(Debug, PartialEq, Eq)]
pub enum AnsiEscapeSequenceParseError {
    /// The bytes do not begin with an escape.
    NotAnEscapeSequence,
    /// The given number of bytes are needed before the sequence can be parsed.
    Need(usize),
    /// The string which the sequence carries has not been terminated yet, and how many more bytes
    /// that takes is not known.
    NeedTerminator,
    /// The given number of bytes are the start of a sequence which is malformed and have to be
    /// skipped before parsing is tried again.
    Unrecognized(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    /// Return the sequence which the bytes are parsed as, requiring that all of them are used.
    fn parse(bytes: &[u8]) -> AnsiEscapeSequence {
        let parsed = ParsedAnsiEscapeSequence::try_from(bytes).unwrap();
        assert_eq!(parsed.len, bytes.len());
        parsed.sequence
    }

    /// Return the control sequence which the bytes are parsed as.
    fn parse_control_sequence(bytes: &[u8]) -> ControlSequence {
        match parse(bytes) {
            AnsiEscapeSequence::ControlSequence(control) => control,
            sequence => panic!("Expected a control sequence but got {:?}.", sequence),
        }
    }

    #[test]
    fn test_a_control_sequence_with_nothing_but_a_final_byte_is_parsed() {
        let control = parse_control_sequence(b"\x1b[H");

        assert_eq!(control.private, None);
        assert!(control.parameters.is_empty());
        assert!(control.intermediates.is_empty());
        assert_eq!(control.final_byte, b'H');
    }

    #[test]
    fn test_the_parameters_of_a_control_sequence_are_parsed() {
        let control = parse_control_sequence(b"\x1b[38;5;196m");

        assert_eq!(control.parameter(0), Some(38));
        assert_eq!(control.parameter(1), Some(5));
        assert_eq!(control.parameter(2), Some(196));
        assert_eq!(control.final_byte, b'm');
    }

    #[test]
    fn test_a_parameter_which_is_left_out_of_a_control_sequence_has_no_value() {
        let control = parse_control_sequence(b"\x1b[;5H");

        assert_eq!(control.parameters.len(), 2);
        assert_eq!(control.parameter(0), None);
        assert_eq!(control.parameter(1), Some(5));
    }

    #[test]
    fn test_the_sub_parameters_of_a_control_sequence_are_parsed() {
        let control = parse_control_sequence(b"\x1b[38:2::255:0:0m");

        assert_eq!(control.parameters.len(), 1);
        assert_eq!(control.parameters[0].value, Some(38));
        assert_eq!(
            control.parameters[0].subs,
            vec![Some(2), None, Some(255), Some(0), Some(0)]
        );
    }

    #[test]
    fn test_the_private_marker_of_a_control_sequence_is_parsed() {
        let control = parse_control_sequence(b"\x1b[?1049h");

        assert_eq!(control.private, Some(b'?'));
        assert_eq!(control.parameter(0), Some(1049));
        assert_eq!(control.final_byte, b'h');
    }

    #[test]
    fn test_the_intermediates_of_a_control_sequence_are_parsed() {
        let control = parse_control_sequence(b"\x1b[4 q");

        assert_eq!(control.parameter(0), Some(4));
        assert_eq!(control.intermediates, vec![b' ']);
        assert_eq!(control.final_byte, b'q');
    }

    #[test_case(b"\x1b]0;a title\x07", b"0;a title"; "terminated by a bell")]
    #[test_case(b"\x1b]0;a title\x1b\\", b"0;a title"; "terminated by a string terminator")]
    fn test_an_operating_system_command_is_parsed(bytes: &[u8], payload: &[u8]) {
        assert_eq!(
            parse(bytes),
            AnsiEscapeSequence::OperatingSystemCommand(payload.to_vec())
        );
    }

    #[test]
    fn test_a_device_control_string_is_parsed() {
        let sequence = parse(b"\x1bP1$r0m\x1b\\");

        match sequence {
            AnsiEscapeSequence::DeviceControlString(string) => {
                assert_eq!(string.control.parameter(0), Some(1));
                assert_eq!(string.control.intermediates, vec![b'$']);
                assert_eq!(string.control.final_byte, b'r');
                assert_eq!(string.payload, b"0m");
            }
            sequence => panic!("Expected a device control string but got {:?}.", sequence),
        }
    }

    #[test_case(b"\x1b_a payload\x1b\\", AnsiEscapeSequence::ApplicationProgramCommand(b"a payload".to_vec()); "an application program command")]
    #[test_case(b"\x1b^a payload\x1b\\", AnsiEscapeSequence::PrivacyMessage(b"a payload".to_vec()); "a privacy message")]
    #[test_case(b"\x1bXa payload\x1b\\", AnsiEscapeSequence::StartOfString(b"a payload".to_vec()); "a start of string")]
    #[test_case(b"\x1bNA", AnsiEscapeSequence::SingleShiftTwo(b'A'); "single shift two")]
    #[test_case(b"\x1bOA", AnsiEscapeSequence::SingleShiftThree(b'A'); "single shift three")]
    fn test_a_sequence_is_parsed(bytes: &[u8], sequence: AnsiEscapeSequence) {
        assert_eq!(parse(bytes), sequence);
    }

    #[test_case(b"\x1bc", &[], b'c'; "resetting the terminal")]
    #[test_case(b"\x1b7", &[], b'7'; "saving the cursor")]
    #[test_case(b"\x1b(B", b"(", b'B'; "picking a character set")]
    #[test_case(b"\x1b#8", b"#", b'8'; "filling the screen")]
    fn test_a_short_escape_sequence_is_parsed(bytes: &[u8], intermediates: &[u8], final_byte: u8) {
        assert_eq!(
            parse(bytes),
            AnsiEscapeSequence::Escape {
                intermediates: intermediates.to_vec(),
                final_byte,
            }
        );
    }

    #[test]
    fn test_a_sequence_is_parsed_from_the_bytes_it_needs_and_no_more() {
        let parsed = ParsedAnsiEscapeSequence::try_from(&b"\x1b[1;5Hrest"[..]).unwrap();

        assert_eq!(parsed.len, 6);
    }

    #[test_case(b"\x1b"; "an escape")]
    #[test_case(b"\x1b["; "a control sequence with nothing after the introducer")]
    #[test_case(b"\x1b[1;5"; "a control sequence without its final byte")]
    #[test_case(b"\x1b[1 "; "a control sequence without its final byte after an intermediate")]
    #[test_case(b"\x1bN"; "a single shift without the byte it shifts")]
    #[test_case(b"\x1b("; "a short sequence without its final byte")]
    fn test_more_bytes_are_needed(bytes: &[u8]) {
        assert_eq!(
            ParsedAnsiEscapeSequence::try_from(bytes),
            Err(AnsiEscapeSequenceParseError::Need(bytes.len() + 1))
        );
    }

    #[test_case(b"\x1b]0;a title"; "an operating system command")]
    #[test_case(b"\x1bP1$r0m"; "a device control string")]
    #[test_case(b"\x1b]0;a title\x1b"; "an operating system command with half of a terminator")]
    fn test_a_terminator_is_needed(bytes: &[u8]) {
        assert_eq!(
            ParsedAnsiEscapeSequence::try_from(bytes),
            Err(AnsiEscapeSequenceParseError::NeedTerminator)
        );
    }

    #[test]
    fn test_a_string_which_is_cut_short_by_another_sequence_does_not_swallow_it() {
        let parsed = ParsedAnsiEscapeSequence::try_from(&b"\x1b]0;a title\x1b[H"[..]).unwrap();

        assert_eq!(
            parsed.sequence,
            AnsiEscapeSequence::OperatingSystemCommand(b"0;a title".to_vec())
        );
        assert_eq!(parsed.len, 11);
    }

    #[test_case(b"\x1b[1\x07"; "a control sequence with a control character for a final byte")]
    #[test_case(b"\x1b\x07"; "a short sequence with a control character for a final byte")]
    fn test_a_malformed_sequence_is_skipped(bytes: &[u8]) {
        assert_eq!(
            ParsedAnsiEscapeSequence::try_from(bytes),
            Err(AnsiEscapeSequenceParseError::Unrecognized(bytes.len()))
        );
    }

    #[test_case(b"\x1b[H"; "a control sequence with nothing but a final byte")]
    #[test_case(b"\x1b[38;5;196m"; "a control sequence with parameters")]
    #[test_case(b"\x1b[;5H"; "a control sequence with a parameter left out")]
    #[test_case(b"\x1b[38:2::255:0:0m"; "a control sequence with sub-parameters")]
    #[test_case(b"\x1b[?1049h"; "a control sequence with a private marker")]
    #[test_case(b"\x1b[4 q"; "a control sequence with an intermediate")]
    #[test_case(b"\x1b]0;a title\x07"; "an operating system command")]
    #[test_case(b"\x1bP1$r0m\x1b\\"; "a device control string")]
    #[test_case(b"\x1b_a payload\x1b\\"; "an application program command")]
    #[test_case(b"\x1b^a payload\x1b\\"; "a privacy message")]
    #[test_case(b"\x1bXa payload\x1b\\"; "a start of string")]
    #[test_case(b"\x1bNA"; "single shift two")]
    #[test_case(b"\x1bOA"; "single shift three")]
    #[test_case(b"\x1bc"; "a short sequence")]
    #[test_case(b"\x1b(B"; "a short sequence with an intermediate")]
    fn test_a_sequence_is_written_back_out_as_the_bytes_it_was_parsed_from(bytes: &[u8]) {
        assert_eq!(Vec::<u8>::from(&parse(bytes)), bytes);
    }

    #[test]
    fn test_a_string_terminator_is_written_back_out_as_a_bell() {
        // The only sequence which is not written back out byte for byte is an operating system
        // command, because a bell says the same thing in one less byte.
        assert_eq!(
            Vec::<u8>::from(&parse(b"\x1b]0;a title\x1b\\")),
            b"\x1b]0;a title\x07"
        );
    }

    #[test]
    fn test_bytes_which_do_not_begin_with_an_escape_are_not_a_sequence() {
        assert_eq!(
            ParsedAnsiEscapeSequence::try_from(&b"j"[..]),
            Err(AnsiEscapeSequenceParseError::NotAnEscapeSequence)
        );
    }
}

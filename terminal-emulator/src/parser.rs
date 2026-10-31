/*!
This module contains the [`Parser`] struct which turns the bytes a program writes into the things
they say to do.
*/
use std::str;

use super::action::Action;

use ansi::{
    AnsiEscapeSequence, AnsiEscapeSequenceParseError, ControlCharacter, ParsedAnsiEscapeSequence,
    ESCAPE,
};

/// Turns the bytes which a program writes into the things they say to do.
///
/// The bytes arrive from a pseudoterminal however many at a time, so a character or a sequence may
/// be split across two of them. Whatever is left over at the end of one lot is kept and read again
/// once the next lot has arrived.
#[derive(Debug, Default)]
pub struct Parser {
    /// The bytes which have arrived and have not been made sense of yet.
    bytes: Vec<u8>,
}

impl Parser {
    /// The most bytes which are kept waiting for the rest of a sequence to arrive.
    ///
    /// A control string has no length of its own and only ends when its terminator arrives, so a
    /// program which sends one and never ends it would otherwise have a screen holding on to
    /// however much it cared to write. Past this the escape is given up on and the bytes are read
    /// as what they are instead.
    const MOST_PENDING: usize = 4096;

    /// The most bytes which a character is written in.
    const LONGEST_CHARACTER: usize = 4;

    /// Take the bytes and return what they say to do.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Action> {
        self.bytes.extend_from_slice(bytes);

        let mut actions: Vec<Action> = Vec::new();
        let mut position: usize = 0;

        while position < self.bytes.len() {
            let pending: &[u8] = &self.bytes[position..];

            let read: Option<(Action, usize)> = match pending[0] {
                ESCAPE => match Self::escape(pending) {
                    Some(read) => Some(read),
                    // The rest of it has not arrived yet, unless so much has gone by waiting for
                    // it that it is not worth waiting any longer.
                    None if pending.len() < Self::MOST_PENDING => break,
                    None => Some((Action::Control(ControlCharacter::Escape), 1)),
                },
                byte if ControlCharacter::is_control(byte) => {
                    Some((Action::Control(ControlCharacter::from(byte)), 1))
                }
                _ => {
                    Self::character(pending).map(|(character, len)| (Action::Print(character), len))
                }
            };

            match read {
                Some((action, len)) => {
                    actions.push(action);
                    position += len;
                }
                // The rest of the character has not arrived yet.
                None => break,
            }
        }

        self.bytes.drain(..position);

        return actions;
    }

    /// Return whether anything is being waited on.
    pub fn is_pending(&self) -> bool {
        return !self.bytes.is_empty();
    }

    /// Return what the escape sequence the bytes start with says to do along with how many bytes
    /// it was written in, which is nothing at all when the rest of it has not arrived.
    ///
    /// A sequence which is not one at all is read as the bytes it is written in rather than being
    /// waited on, since no amount of waiting would make one of it.
    fn escape(bytes: &[u8]) -> Option<(Action, usize)> {
        return match ParsedAnsiEscapeSequence::try_from(bytes) {
            Ok(parsed) => {
                let action: Action = match parsed.sequence.control_function() {
                    Some(function) => Action::Function(function),
                    None => Action::Sequence(parsed.sequence),
                };
                Some((action, parsed.len))
            }
            Err(AnsiEscapeSequenceParseError::Need(_))
            | Err(AnsiEscapeSequenceParseError::NeedTerminator) => None,
            Err(AnsiEscapeSequenceParseError::Unrecognized(len)) => Some((
                Action::Sequence(AnsiEscapeSequence::Escape {
                    intermediates: Vec::new(),
                    final_byte: bytes.get(1).copied().unwrap_or(0),
                }),
                len,
            )),
            Err(AnsiEscapeSequenceParseError::NotAnEscapeSequence) => {
                Some((Action::Control(ControlCharacter::Escape), 1))
            }
        };
    }

    /// Return the character the bytes start with along with how many bytes it is written in, which
    /// is nothing at all when the rest of it has not arrived.
    ///
    /// Bytes which do not spell a character are read one at a time as the character which stands
    /// for one that could not be read, so that a program writing rubbish moves the screen along
    /// rather than wedging it.
    fn character(bytes: &[u8]) -> Option<(char, usize)> {
        let len: usize = Self::character_len(bytes[0]);

        if len == 1 && bytes[0] >= 0x80 {
            return Some((char::REPLACEMENT_CHARACTER, 1));
        }
        if bytes.len() < len {
            return None;
        }

        return match str::from_utf8(&bytes[..len]) {
            Ok(string) => string
                .chars()
                .next()
                .map(|character| (character, len))
                .or(Some((char::REPLACEMENT_CHARACTER, 1))),
            // The bytes which follow do not carry the character on, so it is not one.
            Err(_) => Some((char::REPLACEMENT_CHARACTER, 1)),
        };
    }

    /// Return how many bytes the character which starts with the given byte is written in, which
    /// is one for a byte which does not start one at all.
    fn character_len(lead: u8) -> usize {
        return match lead {
            0x00..=0x7f => 1,
            // NOTE: The two bytes below these start characters which are written in more bytes
            // than they need, which is not allowed, and the ones above the last of them start
            // characters past the last one there is.
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => Self::LONGEST_CHARACTER,
            _ => 1,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ansi::{Color, ControlFunction, GraphicRendition};

    use test_case::test_case;

    /// Return what the bytes say to do, all of them arriving at once.
    fn feed(bytes: &[u8]) -> Vec<Action> {
        return Parser::default().feed(bytes);
    }

    /// Return the actions which write the string.
    fn printed(string: &str) -> Vec<Action> {
        return string.chars().map(Action::Print).collect();
    }

    #[test_case(b"abc", printed("abc"); "plain characters")]
    #[test_case(b"", vec![]; "nothing at all")]
    #[test_case(b"\x07", vec![Action::Control(ControlCharacter::Bell)]; "a control")]
    #[test_case(b"a\rb\n", vec![Action::Print('a'), Action::Control(ControlCharacter::CarriageReturn), Action::Print('b'), Action::Control(ControlCharacter::LineFeed)]; "characters and controls together")]
    #[test_case(b"\x1b[H", vec![Action::Function(ControlFunction::CursorPosition { row: 1, column: 1 })]; "a control function")]
    #[test_case(b"\x1b[31m", vec![Action::Function(ControlFunction::SelectGraphicRendition(vec![GraphicRendition::Foreground(Color::Red)]))]; "a rendition")]
    #[test_case("é".as_bytes(), printed("é"); "a character written in two bytes")]
    #[test_case("🦀".as_bytes(), printed("🦀"); "a character written in four bytes")]
    #[test_case(b"a\x1b[Hb", vec![Action::Print('a'), Action::Function(ControlFunction::CursorPosition { row: 1, column: 1 }), Action::Print('b')]; "a sequence in among characters")]
    fn test_feed(bytes: &[u8], expected_actions: Vec<Action>) {
        assert_eq!(feed(bytes), expected_actions);
    }

    #[test_case(&[b"\x1b", b"[H"]; "a sequence split before its parameters")]
    #[test_case(&[b"\x1b[", b"H"]; "a sequence split before its final byte")]
    #[test_case(&[b"\x1b[1", b";1H"]; "a sequence split partway through its parameters")]
    fn test_a_sequence_split_across_what_arrives_is_read_once_all_of_it_has(lots: &[&[u8]]) {
        let mut parser = Parser::default();

        let mut actions: Vec<Action> = Vec::new();
        for lot in lots {
            actions.extend(parser.feed(lot));
        }

        assert_eq!(
            actions,
            vec![Action::Function(ControlFunction::CursorPosition {
                row: 1,
                column: 1
            })]
        );
        assert_eq!(parser.is_pending(), false);
    }

    #[test]
    fn test_a_character_split_across_what_arrives_is_read_once_all_of_it_has() {
        let bytes: &[u8] = "🦀".as_bytes();
        let mut parser = Parser::default();

        // Every byte but the last leaves it waiting for the rest.
        for byte in &bytes[..bytes.len() - 1] {
            assert_eq!(parser.feed(&[*byte]), vec![]);
        }
        let actions: Vec<Action> = parser.feed(&bytes[bytes.len() - 1..]);

        assert_eq!(actions, printed("🦀"));
        assert_eq!(parser.is_pending(), false);
    }

    #[test]
    fn test_bytes_which_do_not_spell_a_character_are_read_one_at_a_time() {
        // The first byte says three more follow it and they do not, so it is not a character.
        let actions: Vec<Action> = feed(b"\xf0abc");

        assert_eq!(
            actions,
            vec![
                Action::Print(char::REPLACEMENT_CHARACTER),
                Action::Print('a'),
                Action::Print('b'),
                Action::Print('c'),
            ]
        );
    }

    #[test]
    fn test_a_byte_which_starts_no_character_at_all_is_read_on_its_own() {
        assert_eq!(
            feed(b"\x80a"),
            vec![
                Action::Print(char::REPLACEMENT_CHARACTER),
                Action::Print('a')
            ]
        );
    }

    #[test]
    fn test_a_string_which_is_never_ended_is_given_up_on() {
        let mut parser = Parser::default();

        // An operating system command only ends when its terminator arrives, so one which never
        // does would be held on to forever.
        let mut bytes: Vec<u8> = vec![ESCAPE, b']'];
        bytes.resize(Parser::MOST_PENDING + 1, b'a');
        let actions: Vec<Action> = parser.feed(&bytes);

        assert_eq!(
            actions.first(),
            Some(&Action::Control(ControlCharacter::Escape))
        );
        assert_eq!(parser.is_pending(), false);
    }

    #[test]
    fn test_what_is_waited_on_is_kept_and_nothing_else_is() {
        let mut parser = Parser::default();

        parser.feed(b"ab\x1b[");

        assert_eq!(parser.is_pending(), true);
        assert_eq!(parser.bytes, b"\x1b[");
    }
}

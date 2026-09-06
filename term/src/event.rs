use ansi::{
    AnsiEscapeSequence, AnsiEscapeSequenceParseError, BracketedPaste, ControlSequence,
    ParsedAnsiEscapeSequence, ESCAPE,
};
use size::Size;

use std::fmt::{Display, Error as FmtError, Formatter};
use std::str;

use bitflags::bitflags;

#[derive(Debug, Clone)]
pub enum TermEvent {
    KeyEvent(KeyEvent),
    /// Text which was pasted into the terminal.
    Paste(String),
    Resize(Size),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: KeyMods,
}

impl Display for KeyEvent {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        // The shift key is not shown because the key itself is already shifted.
        if self.mods.contains(KeyMods::CONTROL) {
            write!(formatter, "<Ctrl>-")?;
        }
        if self.mods.contains(KeyMods::ALT) {
            write!(formatter, "<Alt>-")?;
        }
        if self.mods.contains(KeyMods::SUPER) {
            write!(formatter, "<Super>-")?;
        }
        write!(formatter, "{}", self.key)
    }
}

/// A terminal event along with the number of bytes which it was parsed from.
#[derive(Debug, Clone)]
pub struct ParsedTermEvent {
    pub event: TermEvent,
    pub len: usize,
}

impl TryFrom<&[u8]> for ParsedTermEvent {
    type Error = TermEventParseError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let byte: u8 = match bytes.first() {
            Some(byte) => *byte,
            None => {
                return Err(TermEventParseError::Need(1));
            }
        };

        match byte {
            ESCAPE => Self::parse_escape(bytes),
            0x80..=0xff => Self::parse_utf8(bytes),
            byte => Ok(Self {
                event: TermEvent::KeyEvent(KeyEvent::from(byte)),
                len: 1,
            }),
        }
    }
}

impl ParsedTermEvent {
    /// Parse an escape sequence as the key which was pressed to send it.
    fn parse_escape(bytes: &[u8]) -> Result<Self, TermEventParseError> {
        // Pasted text is not a sequence but two of them with the text in between, and its length is
        // the one which is not known ahead of time.
        if bytes.starts_with(BracketedPaste::START) {
            return Self::parse_paste(bytes);
        }
        if BracketedPaste::START.starts_with(bytes) {
            return Err(TermEventParseError::Need(BracketedPaste::START.len()));
        }

        match ParsedAnsiEscapeSequence::try_from(bytes) {
            Ok(ParsedAnsiEscapeSequence { sequence, len }) => match Self::key_event(&sequence) {
                Some(key_event) => Ok(Self {
                    event: TermEvent::KeyEvent(key_event),
                    len,
                }),
                // The sequence is one the terminal sends as a reply rather than for a key.
                None => Err(TermEventParseError::Unrecognized(len)),
            },
            Err(AnsiEscapeSequenceParseError::Need(needed)) => {
                Err(TermEventParseError::Need(needed))
            }
            // How long a string is cannot be known in advance, so the most that can be done is to
            // ask for one more byte than has turned up so far.
            Err(AnsiEscapeSequenceParseError::NeedTerminator) => {
                Err(TermEventParseError::Need(bytes.len() + 1))
            }
            // A key held with <Alt> sends an escape followed by whatever that key sends on its own,
            // which is not a sequence when the key is one like an arrow or backspace.
            Err(
                AnsiEscapeSequenceParseError::Unrecognized(_)
                | AnsiEscapeSequenceParseError::NotAnEscapeSequence,
            ) => Self::parse_alt(bytes),
        }
    }

    /// Parse an escape followed by the bytes which a key held with <Alt> sends.
    fn parse_alt(bytes: &[u8]) -> Result<Self, TermEventParseError> {
        let parsed: Self = match Self::try_from(&bytes[1..]) {
            Ok(parsed) => parsed,
            Err(TermEventParseError::Need(needed)) => {
                return Err(TermEventParseError::Need(needed + 1));
            }
            Err(TermEventParseError::Unrecognized(len)) => {
                return Err(TermEventParseError::Unrecognized(len + 1));
            }
            Err(error) => {
                return Err(error);
            }
        };

        let event: TermEvent = match parsed.event {
            TermEvent::KeyEvent(KeyEvent { key, mods }) => TermEvent::KeyEvent(KeyEvent {
                key,
                mods: mods | KeyMods::ALT,
            }),
            event => event,
        };

        Ok(Self {
            event,
            len: parsed.len + 1,
        })
    }

    /// Return the key which an escape sequence was sent for, or `None` if it is one which is not
    /// sent for a key at all.
    fn key_event(sequence: &AnsiEscapeSequence) -> Option<KeyEvent> {
        match sequence {
            AnsiEscapeSequence::ControlSequence(control) => {
                Self::control_sequence_key_event(control)
            }
            // These are what the cursor and keypad keys send when the terminal is in application
            // mode, and what the first four function keys send either way.
            AnsiEscapeSequence::SingleShiftThree(byte) => {
                let key: Key = match byte {
                    b'A' => Key::Up,
                    b'B' => Key::Down,
                    b'C' => Key::Right,
                    b'D' => Key::Left,
                    b'F' => Key::End,
                    b'H' => Key::Home,
                    b'M' => Key::CarriageReturn,
                    b'P'..=b'S' => Key::Function(byte - b'P' + 1),
                    _ => {
                        return None;
                    }
                };

                Some(KeyEvent {
                    key,
                    mods: KeyMods::NONE,
                })
            }
            // A key held with <Alt> sends an escape followed by whatever that key sends on its own,
            // which looks like one of the short sequences when that key is an ordinary character.
            AnsiEscapeSequence::Escape {
                intermediates,
                final_byte,
            } if intermediates.is_empty() => {
                let KeyEvent { key, mods } = KeyEvent::from(*final_byte);
                Some(KeyEvent {
                    key,
                    mods: mods | KeyMods::ALT,
                })
            }
            _ => None,
        }
    }

    /// Return the key which a control sequence was sent for, or `None` if it is one which is not
    /// sent for a key at all.
    fn control_sequence_key_event(control: &ControlSequence) -> Option<KeyEvent> {
        // No key is sent as a sequence which is private to a terminal or which has intermediates.
        if control.private.is_some() || !control.intermediates.is_empty() {
            return None;
        }

        // The modifiers which a key was held with are the second parameter.
        let mods: KeyMods = KeyMods::from_control_sequence_parameter(control.parameter(1));

        let key: Key = match control.final_byte {
            b'A' => Key::Up,
            b'B' => Key::Down,
            b'C' => Key::Right,
            b'D' => Key::Left,
            b'F' => Key::End,
            b'H' => Key::Home,
            b'Z' => {
                return Some(KeyEvent {
                    key: Key::BackTab,
                    mods: KeyMods::SHIFT,
                });
            }
            // NOTE: The first four function keys are sent as single shift three sequences unless
            // they are held with a modifier, in which case the first parameter is a one which is
            // only there to leave room for the modifier as the second. Requiring that one is what
            // keeps a report of where the cursor is, which is the row and the column then an `R`,
            // from being read as the third function key. The two are the same sequence for a cursor
            // on the first row, but nothing here ever asks the terminal where the cursor is.
            byte @ b'P'..=b'S' if control.parameter(0) == Some(1) => Key::Function(byte - b'P' + 1),
            b'~' => match control.parameter(0) {
                Some(1 | 7) => Key::Home,
                Some(2) => Key::Insert,
                Some(3) => Key::Delete,
                Some(4 | 8) => Key::End,
                Some(5) => Key::PageUp,
                Some(6) => Key::PageDown,
                Some(number @ 11..=15) => Key::Function((number - 10) as u8),
                Some(number @ 17..=21) => Key::Function((number - 11) as u8),
                Some(number @ 23..=24) => Key::Function((number - 12) as u8),
                _ => {
                    return None;
                }
            },
            _ => {
                return None;
            }
        };

        Some(KeyEvent { key, mods })
    }

    /// Parse text which was pasted into the terminal while it was in bracketed paste mode.
    fn parse_paste(bytes: &[u8]) -> Result<Self, TermEventParseError> {
        let text_start: usize = BracketedPaste::START.len();

        let text_end: usize = match bytes[text_start..]
            .windows(BracketedPaste::END.len())
            .position(|window| window == BracketedPaste::END)
        {
            Some(position) => text_start + position,
            None => {
                return Err(TermEventParseError::NeedRestOfPaste);
            }
        };

        // NOTE: The terminal is not obliged to send text which is valid UTF-8, so the bytes which
        // are not are replaced rather than the whole paste being thrown away.
        let text: String = String::from_utf8_lossy(&bytes[text_start..text_end]).into_owned();

        Ok(Self {
            event: TermEvent::Paste(text),
            len: text_end + BracketedPaste::END.len(),
        })
    }

    /// Parse a character which is encoded as more than one byte.
    fn parse_utf8(bytes: &[u8]) -> Result<Self, TermEventParseError> {
        let len: usize = match bytes[0] {
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            // A continuation byte with nothing to continue, or a byte which UTF-8 never uses.
            _ => {
                return Err(TermEventParseError::Unrecognized(1));
            }
        };

        if bytes.len() < len {
            return Err(TermEventParseError::Need(len));
        }

        let character: char = match str::from_utf8(&bytes[..len]) {
            Ok(string) => match string.chars().next() {
                Some(character) => character,
                None => {
                    return Err(TermEventParseError::Unrecognized(len));
                }
            },
            Err(_) => {
                return Err(TermEventParseError::Unrecognized(1));
            }
        };

        Ok(Self {
            event: TermEvent::KeyEvent(KeyEvent {
                key: Key::Char(character),
                mods: KeyMods::NONE,
            }),
            len,
        })
    }
}

#[derive(Debug)]
pub enum TermEventParseError {
    /// The given number of bytes are needed before an event can be parsed. Waiting for them can
    /// only be done for so long, because the bytes which have been read so far may be all that the
    /// terminal is going to send. (A press of the escape key is indistinguishable from the start of
    /// an escape sequence until the rest of the sequence does or does not turn up.)
    Need(usize),
    /// The rest of some pasted text is needed. Unlike [`Need`](Self::Need), the terminal has
    /// committed to sending it, so it is worth waiting for however long it takes to arrive.
    NeedRestOfPaste,
    /// The given number of bytes are not the start of any event which is recognized and have to be
    /// skipped before parsing is tried again.
    Unrecognized(usize),
}

impl From<u8> for KeyEvent {
    fn from(byte: u8) -> Self {
        match byte {
            0 => Self {
                key: Key::Null,
                mods: KeyMods::NONE,
            },
            1 => Self {
                key: Key::Char('a'),
                mods: KeyMods::CONTROL,
            },
            2 => Self {
                key: Key::Char('b'),
                mods: KeyMods::CONTROL,
            },
            3 => Self {
                key: Key::Char('c'),
                mods: KeyMods::CONTROL,
            },
            4 => Self {
                key: Key::Char('d'),
                mods: KeyMods::CONTROL,
            },
            5 => Self {
                key: Key::Char('e'),
                mods: KeyMods::CONTROL,
            },
            6 => Self {
                key: Key::Char('f'),
                mods: KeyMods::CONTROL,
            },
            7 => Self {
                key: Key::Char('g'),
                mods: KeyMods::CONTROL,
            },
            8 => Self {
                key: Key::Char('h'),
                mods: KeyMods::CONTROL,
            },
            9 => Self {
                key: Key::HorizontalTab,
                mods: KeyMods::NONE,
            },
            10 => Self {
                key: Key::Char('j'),
                mods: KeyMods::CONTROL,
            },
            11 => Self {
                key: Key::Char('k'),
                mods: KeyMods::CONTROL,
            },
            12 => Self {
                key: Key::Char('l'),
                mods: KeyMods::CONTROL,
            },
            13 => Self {
                key: Key::CarriageReturn,
                mods: KeyMods::NONE,
            },
            14 => Self {
                key: Key::Char('n'),
                mods: KeyMods::CONTROL,
            },
            15 => Self {
                key: Key::Char('o'),
                mods: KeyMods::CONTROL,
            },
            16 => Self {
                key: Key::Char('p'),
                mods: KeyMods::CONTROL,
            },
            17 => Self {
                key: Key::Char('q'),
                mods: KeyMods::CONTROL,
            },
            18 => Self {
                key: Key::Char('r'),
                mods: KeyMods::CONTROL,
            },
            19 => Self {
                key: Key::Char('s'),
                mods: KeyMods::CONTROL,
            },
            20 => Self {
                key: Key::Char('t'),
                mods: KeyMods::CONTROL,
            },
            21 => Self {
                key: Key::Char('u'),
                mods: KeyMods::CONTROL,
            },
            22 => Self {
                key: Key::Char('v'),
                mods: KeyMods::CONTROL,
            },
            23 => Self {
                key: Key::Char('w'),
                mods: KeyMods::CONTROL,
            },
            24 => Self {
                key: Key::Char('x'),
                mods: KeyMods::CONTROL,
            },
            25 => Self {
                key: Key::Char('y'),
                mods: KeyMods::CONTROL,
            },
            26 => Self {
                key: Key::Char('z'),
                mods: KeyMods::CONTROL,
            },
            27 => Self {
                key: Key::Escape,
                mods: KeyMods::NONE,
            },
            28 => Self {
                key: Key::FileSep,
                mods: KeyMods::NONE,
            },
            29 => Self {
                key: Key::GroupSep,
                mods: KeyMods::NONE,
            },
            30 => Self {
                key: Key::RecordSep,
                mods: KeyMods::NONE,
            },
            31 => Self {
                key: Key::UnitSep,
                mods: KeyMods::NONE,
            },
            32 => Self {
                key: Key::Char(' '),
                mods: KeyMods::NONE,
            },
            33 => Self {
                key: Key::Char('!'),
                mods: KeyMods::SHIFT,
            },
            34 => Self {
                key: Key::Char('"'),
                mods: KeyMods::SHIFT,
            },
            35 => Self {
                key: Key::Char('#'),
                mods: KeyMods::SHIFT,
            },
            36 => Self {
                key: Key::Char('$'),
                mods: KeyMods::SHIFT,
            },
            37 => Self {
                key: Key::Char('%'),
                mods: KeyMods::SHIFT,
            },
            38 => Self {
                key: Key::Char('&'),
                mods: KeyMods::SHIFT,
            },
            39 => Self {
                key: Key::Char('\''),
                mods: KeyMods::NONE,
            },
            40 => Self {
                key: Key::Char('('),
                mods: KeyMods::SHIFT,
            },
            41 => Self {
                key: Key::Char(')'),
                mods: KeyMods::SHIFT,
            },
            42 => Self {
                key: Key::Char('*'),
                mods: KeyMods::SHIFT,
            },
            43 => Self {
                key: Key::Char('+'),
                mods: KeyMods::SHIFT,
            },
            44 => Self {
                key: Key::Char(','),
                mods: KeyMods::NONE,
            },
            45 => Self {
                key: Key::Char('-'),
                mods: KeyMods::NONE,
            },
            46 => Self {
                key: Key::Char('.'),
                mods: KeyMods::NONE,
            },
            47 => Self {
                key: Key::Char('/'),
                mods: KeyMods::NONE,
            },
            48 => Self {
                key: Key::Char('0'),
                mods: KeyMods::NONE,
            },
            49 => Self {
                key: Key::Char('1'),
                mods: KeyMods::NONE,
            },
            50 => Self {
                key: Key::Char('2'),
                mods: KeyMods::NONE,
            },
            51 => Self {
                key: Key::Char('3'),
                mods: KeyMods::NONE,
            },
            52 => Self {
                key: Key::Char('4'),
                mods: KeyMods::NONE,
            },
            53 => Self {
                key: Key::Char('5'),
                mods: KeyMods::NONE,
            },
            54 => Self {
                key: Key::Char('6'),
                mods: KeyMods::NONE,
            },
            55 => Self {
                key: Key::Char('7'),
                mods: KeyMods::NONE,
            },
            56 => Self {
                key: Key::Char('8'),
                mods: KeyMods::NONE,
            },
            57 => Self {
                key: Key::Char('9'),
                mods: KeyMods::NONE,
            },
            58 => Self {
                key: Key::Char(':'),
                mods: KeyMods::SHIFT,
            },
            59 => Self {
                key: Key::Char(';'),
                mods: KeyMods::NONE,
            },
            60 => Self {
                key: Key::Char('<'),
                mods: KeyMods::SHIFT,
            },
            61 => Self {
                key: Key::Char('='),
                mods: KeyMods::NONE,
            },
            62 => Self {
                key: Key::Char('>'),
                mods: KeyMods::SHIFT,
            },
            63 => Self {
                key: Key::Char('?'),
                mods: KeyMods::SHIFT,
            },
            64 => Self {
                key: Key::Char('@'),
                mods: KeyMods::SHIFT,
            },
            65 => Self {
                key: Key::Char('A'),
                mods: KeyMods::SHIFT,
            },
            66 => Self {
                key: Key::Char('B'),
                mods: KeyMods::SHIFT,
            },
            67 => Self {
                key: Key::Char('C'),
                mods: KeyMods::SHIFT,
            },
            68 => Self {
                key: Key::Char('D'),
                mods: KeyMods::SHIFT,
            },
            69 => Self {
                key: Key::Char('E'),
                mods: KeyMods::SHIFT,
            },
            70 => Self {
                key: Key::Char('F'),
                mods: KeyMods::SHIFT,
            },
            71 => Self {
                key: Key::Char('G'),
                mods: KeyMods::SHIFT,
            },
            72 => Self {
                key: Key::Char('H'),
                mods: KeyMods::SHIFT,
            },
            73 => Self {
                key: Key::Char('I'),
                mods: KeyMods::SHIFT,
            },
            74 => Self {
                key: Key::Char('J'),
                mods: KeyMods::SHIFT,
            },
            75 => Self {
                key: Key::Char('K'),
                mods: KeyMods::SHIFT,
            },
            76 => Self {
                key: Key::Char('L'),
                mods: KeyMods::SHIFT,
            },
            77 => Self {
                key: Key::Char('M'),
                mods: KeyMods::SHIFT,
            },
            78 => Self {
                key: Key::Char('N'),
                mods: KeyMods::SHIFT,
            },
            79 => Self {
                key: Key::Char('O'),
                mods: KeyMods::SHIFT,
            },
            80 => Self {
                key: Key::Char('P'),
                mods: KeyMods::SHIFT,
            },
            81 => Self {
                key: Key::Char('Q'),
                mods: KeyMods::SHIFT,
            },
            82 => Self {
                key: Key::Char('R'),
                mods: KeyMods::SHIFT,
            },
            83 => Self {
                key: Key::Char('S'),
                mods: KeyMods::SHIFT,
            },
            84 => Self {
                key: Key::Char('T'),
                mods: KeyMods::SHIFT,
            },
            85 => Self {
                key: Key::Char('U'),
                mods: KeyMods::SHIFT,
            },
            86 => Self {
                key: Key::Char('V'),
                mods: KeyMods::SHIFT,
            },
            87 => Self {
                key: Key::Char('W'),
                mods: KeyMods::SHIFT,
            },
            88 => Self {
                key: Key::Char('X'),
                mods: KeyMods::SHIFT,
            },
            89 => Self {
                key: Key::Char('Y'),
                mods: KeyMods::SHIFT,
            },
            90 => Self {
                key: Key::Char('Z'),
                mods: KeyMods::SHIFT,
            },
            91 => Self {
                key: Key::Char('['),
                mods: KeyMods::NONE,
            },
            92 => Self {
                key: Key::Char('\\'),
                mods: KeyMods::NONE,
            },
            93 => Self {
                key: Key::Char(']'),
                mods: KeyMods::NONE,
            },
            94 => Self {
                key: Key::Char('^'),
                mods: KeyMods::SHIFT,
            },
            95 => Self {
                key: Key::Char('_'),
                mods: KeyMods::SHIFT,
            },
            96 => Self {
                key: Key::Char('`'),
                mods: KeyMods::NONE,
            },
            97 => Self {
                key: Key::Char('a'),
                mods: KeyMods::NONE,
            },
            98 => Self {
                key: Key::Char('b'),
                mods: KeyMods::NONE,
            },
            99 => Self {
                key: Key::Char('c'),
                mods: KeyMods::NONE,
            },
            100 => Self {
                key: Key::Char('d'),
                mods: KeyMods::NONE,
            },
            101 => Self {
                key: Key::Char('e'),
                mods: KeyMods::NONE,
            },
            102 => Self {
                key: Key::Char('f'),
                mods: KeyMods::NONE,
            },
            103 => Self {
                key: Key::Char('g'),
                mods: KeyMods::NONE,
            },
            104 => Self {
                key: Key::Char('h'),
                mods: KeyMods::NONE,
            },
            105 => Self {
                key: Key::Char('i'),
                mods: KeyMods::NONE,
            },
            106 => Self {
                key: Key::Char('j'),
                mods: KeyMods::NONE,
            },
            107 => Self {
                key: Key::Char('k'),
                mods: KeyMods::NONE,
            },
            108 => Self {
                key: Key::Char('l'),
                mods: KeyMods::NONE,
            },
            109 => Self {
                key: Key::Char('m'),
                mods: KeyMods::NONE,
            },
            110 => Self {
                key: Key::Char('n'),
                mods: KeyMods::NONE,
            },
            111 => Self {
                key: Key::Char('o'),
                mods: KeyMods::NONE,
            },
            112 => Self {
                key: Key::Char('p'),
                mods: KeyMods::NONE,
            },
            113 => Self {
                key: Key::Char('q'),
                mods: KeyMods::NONE,
            },
            114 => Self {
                key: Key::Char('r'),
                mods: KeyMods::NONE,
            },
            115 => Self {
                key: Key::Char('s'),
                mods: KeyMods::NONE,
            },
            116 => Self {
                key: Key::Char('t'),
                mods: KeyMods::NONE,
            },
            117 => Self {
                key: Key::Char('u'),
                mods: KeyMods::NONE,
            },
            118 => Self {
                key: Key::Char('v'),
                mods: KeyMods::NONE,
            },
            119 => Self {
                key: Key::Char('w'),
                mods: KeyMods::NONE,
            },
            120 => Self {
                key: Key::Char('x'),
                mods: KeyMods::NONE,
            },
            121 => Self {
                key: Key::Char('y'),
                mods: KeyMods::NONE,
            },
            122 => Self {
                key: Key::Char('z'),
                mods: KeyMods::NONE,
            },
            123 => Self {
                key: Key::Char('{'),
                mods: KeyMods::SHIFT,
            },
            124 => Self {
                key: Key::Char('|'),
                mods: KeyMods::SHIFT,
            },
            125 => Self {
                key: Key::Char('}'),
                mods: KeyMods::SHIFT,
            },
            126 => Self {
                key: Key::Char('~'),
                mods: KeyMods::SHIFT,
            },
            127 => Self {
                key: Key::Backspace,
                mods: KeyMods::NONE,
            },
            byte => Self {
                key: Key::Unknown(byte),
                mods: KeyMods::NONE,
            },
        }
    }
}

impl From<&KeyEvent> for Vec<u8> {
    fn from(key_event: &KeyEvent) -> Self {
        let KeyEvent { key, mods } = key_event;

        match key {
            Key::Null => vec![0],
            Key::StartOfHeading => vec![1],
            Key::StartOfText => vec![2],
            Key::EndOfText => vec![3],
            Key::EndOfTransmission => vec![4],
            Key::Enquiry => vec![5],
            Key::Ack => vec![6],
            Key::Bell => vec![7],
            Key::HorizontalTab => vec![9],
            Key::LineFeed => vec![10],
            Key::VertialTab => vec![11],
            Key::FormFeed => vec![12],
            Key::CarriageReturn => vec![13],
            Key::ShiftOut => vec![14],
            Key::ShiftIn => vec![15],
            Key::DataLinkEscape => vec![16],
            Key::DeviceControl1 => vec![17],
            Key::DeviceControl2 => vec![18],
            Key::DeviceControl3 => vec![19],
            Key::DeviceControl4 => vec![20],
            Key::Nack => vec![21],
            Key::SynchronousIdle => vec![22],
            Key::EndOfTransmissionBlock => vec![23],
            Key::Cancel => vec![24],
            Key::EndOfMedium => vec![25],
            Key::Substitute => vec![26],
            Key::Escape => vec![ESCAPE],
            Key::FileSep => vec![28],
            Key::GroupSep => vec![29],
            Key::RecordSep => vec![30],
            Key::UnitSep => vec![31],
            Key::Backspace => vec![127],
            Key::Unknown(byte) => vec![*byte],

            Key::Up => KeyEvent::control_sequence(mods, None, b'A'),
            Key::Down => KeyEvent::control_sequence(mods, None, b'B'),
            Key::Right => KeyEvent::control_sequence(mods, None, b'C'),
            Key::Left => KeyEvent::control_sequence(mods, None, b'D'),
            Key::End => KeyEvent::control_sequence(mods, None, b'F'),
            Key::Home => KeyEvent::control_sequence(mods, None, b'H'),
            Key::Insert => KeyEvent::control_sequence(mods, Some(2), b'~'),
            Key::Delete => KeyEvent::control_sequence(mods, Some(3), b'~'),
            Key::PageUp => KeyEvent::control_sequence(mods, Some(5), b'~'),
            Key::PageDown => KeyEvent::control_sequence(mods, Some(6), b'~'),
            Key::BackTab => vec![ESCAPE, b'[', b'Z'],

            // NOTE: The first four function keys are sent as single shift three sequences unless
            // they are held with a modifier, which those sequences have no room for.
            Key::Function(number @ 1..=4) if *mods == KeyMods::NONE => {
                vec![ESCAPE, b'O', b'P' + (number - 1)]
            }
            Key::Function(number @ 1..=4) => {
                KeyEvent::control_sequence(mods, Some(1), b'P' + (number - 1))
            }
            Key::Function(5) => KeyEvent::control_sequence(mods, Some(15), b'~'),
            Key::Function(number @ 6..=10) => {
                KeyEvent::control_sequence(mods, Some(u16::from(*number) + 11), b'~')
            }
            Key::Function(number @ 11..=12) => {
                KeyEvent::control_sequence(mods, Some(u16::from(*number) + 12), b'~')
            }
            // NOTE: There is nothing to send for a function key which no terminal has.
            Key::Function(_) => Vec::new(),

            Key::Char(character) => {
                let mut bytes: Self = Self::new();

                // <Alt> is sent as an escape followed by whatever the key which was held with it
                // sends.
                if mods.contains(KeyMods::ALT) {
                    bytes.push(ESCAPE);
                }

                if mods.contains(KeyMods::CONTROL) && character.is_ascii_alphabetic() {
                    bytes.push(character.to_ascii_lowercase() as u8 - b'a' + 1);
                } else {
                    let mut buffer: [u8; 4] = [0; 4];
                    bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                }

                bytes
            }
        }
    }
}

impl KeyEvent {
    /// Return the bytes of the control sequence with the given parameter and final byte, along
    /// with the parameter which the modifiers need.
    fn control_sequence(mods: &KeyMods, parameter: Option<u16>, final_byte: u8) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![ESCAPE, b'['];

        if *mods == KeyMods::NONE {
            if let Some(parameter) = parameter {
                bytes.extend_from_slice(parameter.to_string().as_bytes());
            }
        } else {
            // The modifiers are the second parameter, so the first one has to be written out even
            // when it is the default, which is one.
            let parameter: u16 = parameter.unwrap_or(1);
            let mods: u16 = mods.control_sequence_parameter();
            bytes.extend_from_slice(format!("{};{}", parameter, mods).as_bytes());
        }

        bytes.push(final_byte);
        bytes
    }
}

impl Display for Key {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::Char(character) => write!(formatter, "{}", character),
            Self::Null => write!(formatter, "<Null>"),
            Self::Backspace => write!(formatter, "<Backspace>"),
            Self::HorizontalTab => write!(formatter, "<Tab>"),
            Self::CarriageReturn => write!(formatter, "<Enter>"),
            Self::Escape => write!(formatter, "<Escape>"),
            Self::Delete => write!(formatter, "<Delete>"),
            Self::Insert => write!(formatter, "<Insert>"),
            Self::Up => write!(formatter, "<Up>"),
            Self::Down => write!(formatter, "<Down>"),
            Self::Left => write!(formatter, "<Left>"),
            Self::Right => write!(formatter, "<Right>"),
            Self::Home => write!(formatter, "<Home>"),
            Self::End => write!(formatter, "<End>"),
            Self::PageUp => write!(formatter, "<PageUp>"),
            Self::PageDown => write!(formatter, "<PageDown>"),
            Self::BackTab => write!(formatter, "<Shift>-<Tab>"),
            Self::Function(number) => write!(formatter, "<F{}>", number),
            // The rest of the keys are control codes which the terminal is not expected to send
            // since it sends them as characters held with the control key instead.
            key => write!(formatter, "<{:?}>", key),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Null,
    /// Start of text (same as <Ctrl>-a)
    StartOfHeading,
    /// Start of text (same as <Ctrl>-b)
    StartOfText,
    /// End of text (same as <Ctrl>-c)
    EndOfText,
    /// End of transmission (same as <Ctrl>-d)
    EndOfTransmission,
    /// Enquiry (same as <Ctrl>-e)
    Enquiry,
    /// Acknowledgement (same as <Ctrl>-f)
    Ack,
    /// Bell (same as <Ctrl>-g)
    Bell,
    /// Horizontal tab (same as <Ctrl>-i)
    HorizontalTab,
    /// Line Feed (same as <Ctrl>-j)
    LineFeed,
    /// Vertical Tab (same as <Ctrl>-k)
    VertialTab,
    /// Form feed (same as <Ctrl>-l)
    FormFeed,
    /// Carriage Return (Enter) (same as <Ctrl>-m)
    CarriageReturn,
    /// Shift out (same as <Ctrl>-n)
    ShiftOut,
    /// Shift in (same as <Ctrl>-o)
    ShiftIn,
    /// Data link escape (same as <Ctrl>-p)
    DataLinkEscape,
    /// Device control 1 (same as <Ctrl>-q)
    DeviceControl1,
    /// Device control 2 (same as <Ctrl>-r)
    DeviceControl2,
    /// Device control 3 (same as <Ctrl>-s)
    DeviceControl3,
    /// Device control 4 (same as <Ctrl>-t)
    DeviceControl4,
    /// Negative acknowledgement (same as <Ctrl>-u)
    Nack,
    /// Synchronous idle (same as <Ctrl>-v)
    SynchronousIdle,
    /// End of transmission block (same as <Ctrl>-w)
    EndOfTransmissionBlock,
    /// Cancel (same as <Ctrl>-x)
    Cancel,
    /// End of medium (same as <Ctrl>-y)
    EndOfMedium,
    /// Substitute (same as <Ctrl>-z)
    Substitute,
    Escape,
    FileSep,
    GroupSep,
    RecordSep,
    UnitSep,
    Char(char),
    /// The key which deletes the character before the cursor. Note that this is what the terminal
    /// sends for a press of the backspace key, which is a delete character and not the backspace
    /// control character (that one is the same as <Ctrl>-h).
    Backspace,
    /// The key which deletes the character after the cursor.
    Delete,
    Insert,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    /// Tab held with shift.
    BackTab,
    /// A function key, numbered from one.
    Function(u8),
    Unknown(u8),
}

bitflags! {
    /// Key modifiers.
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub struct KeyMods: u8 {
        const NONE = 0b0000_0000;
        const SHIFT = 0b0000_0001;
        const CONTROL = 0b0000_0010;
        const ALT = 0b0000_0100;
        const SUPER = 0b0000_1000;
    }
}

impl KeyMods {
    /// Return the modifiers which the given control sequence parameter encodes. The parameter is
    /// one more than a mask of the modifiers so that it is never zero.
    fn from_control_sequence_parameter(parameter: Option<u16>) -> Self {
        let parameter: u16 = match parameter {
            Some(parameter) => parameter,
            None => {
                return Self::NONE;
            }
        };

        let mut mods: Self = Self::NONE;
        let mask: u16 = parameter.saturating_sub(1);

        if mask & 0b0001 != 0 {
            mods |= Self::SHIFT;
        }
        if mask & 0b0010 != 0 {
            mods |= Self::ALT;
        }
        if mask & 0b0100 != 0 {
            mods |= Self::CONTROL;
        }
        if mask & 0b1000 != 0 {
            mods |= Self::SUPER;
        }

        mods
    }

    /// Return the control sequence parameter which encodes the modifiers.
    fn control_sequence_parameter(&self) -> u16 {
        let mut mask: u16 = 0;

        if self.contains(Self::SHIFT) {
            mask |= 0b0001;
        }
        if self.contains(Self::ALT) {
            mask |= 0b0010;
        }
        if self.contains(Self::CONTROL) {
            mask |= 0b0100;
        }
        if self.contains(Self::SUPER) {
            mask |= 0b1000;
        }

        mask + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    /// Return the key event which the bytes are parsed as, requiring that all of them are used.
    fn parse(bytes: &[u8]) -> KeyEvent {
        let parsed = ParsedTermEvent::try_from(bytes).unwrap();
        assert_eq!(parsed.len, bytes.len());
        match parsed.event {
            TermEvent::KeyEvent(key_event) => key_event,
            event => panic!("Expected a key event but got {:?}.", event),
        }
    }

    #[test_case(b"\x1b[A", Key::Up, KeyMods::NONE; "up")]
    #[test_case(b"\x1b[B", Key::Down, KeyMods::NONE; "down")]
    #[test_case(b"\x1b[C", Key::Right, KeyMods::NONE; "right")]
    #[test_case(b"\x1b[D", Key::Left, KeyMods::NONE; "left")]
    #[test_case(b"\x1b[H", Key::Home, KeyMods::NONE; "home")]
    #[test_case(b"\x1b[F", Key::End, KeyMods::NONE; "end")]
    #[test_case(b"\x1b[2~", Key::Insert, KeyMods::NONE; "insert")]
    #[test_case(b"\x1b[3~", Key::Delete, KeyMods::NONE; "delete")]
    #[test_case(b"\x1b[5~", Key::PageUp, KeyMods::NONE; "page up")]
    #[test_case(b"\x1b[6~", Key::PageDown, KeyMods::NONE; "page down")]
    #[test_case(b"\x1b[Z", Key::BackTab, KeyMods::SHIFT; "back tab")]
    #[test_case(b"\x1bOP", Key::Function(1), KeyMods::NONE; "first function key")]
    #[test_case(b"\x1b[15~", Key::Function(5), KeyMods::NONE; "fifth function key")]
    #[test_case(b"\x1b[17~", Key::Function(6), KeyMods::NONE; "sixth function key")]
    #[test_case(b"\x1b[21~", Key::Function(10), KeyMods::NONE; "tenth function key")]
    #[test_case(b"\x1b[23~", Key::Function(11), KeyMods::NONE; "eleventh function key")]
    #[test_case(b"\x1b[24~", Key::Function(12), KeyMods::NONE; "twelfth function key")]
    #[test_case(b"\x1b[1;5A", Key::Up, KeyMods::CONTROL; "up with control")]
    #[test_case(b"\x1b[1;2D", Key::Left, KeyMods::SHIFT; "left with shift")]
    #[test_case(b"\x1b[1;3C", Key::Right, KeyMods::ALT; "right with alt")]
    #[test_case(b"\x1b[3;5~", Key::Delete, KeyMods::CONTROL; "delete with control")]
    #[test_case(b"\x1b[1;6B", Key::Down, KeyMods::SHIFT.union(KeyMods::CONTROL); "down with shift and control")]
    #[test_case(b"\x1b[1;5P", Key::Function(1), KeyMods::CONTROL; "first function key with control")]
    #[test_case(b"\x7f", Key::Backspace, KeyMods::NONE; "backspace")]
    #[test_case(b"\x1bj", Key::Char('j'), KeyMods::ALT; "a character with alt")]
    #[test_case("é".as_bytes(), Key::Char('é'), KeyMods::NONE; "a two byte character")]
    #[test_case("→".as_bytes(), Key::Char('→'), KeyMods::NONE; "a three byte character")]
    #[test_case("🦀".as_bytes(), Key::Char('🦀'), KeyMods::NONE; "a four byte character")]
    fn test_a_key_round_trips(bytes: &[u8], key: Key, mods: KeyMods) {
        let key_event: KeyEvent = parse(bytes);
        assert_eq!(key_event.key, key);
        assert_eq!(key_event.mods, mods);

        assert_eq!(Vec::<u8>::from(&KeyEvent { key, mods }), bytes);
    }

    // A key can have more than one sequence, only one of which is written back out for it.
    #[test_case(b"\x1bOA", Key::Up; "up in application mode")]
    #[test_case(b"\x1bOD", Key::Left; "left in application mode")]
    #[test_case(b"\x1bOH", Key::Home; "home in application mode")]
    #[test_case(b"\x1bOM", Key::CarriageReturn; "enter on the keypad")]
    #[test_case(b"\x1b[1~", Key::Home; "home as a numbered sequence")]
    #[test_case(b"\x1b[7~", Key::Home; "home as the other numbered sequence")]
    #[test_case(b"\x1b[4~", Key::End; "end as a numbered sequence")]
    #[test_case(b"\x1b[8~", Key::End; "end as the other numbered sequence")]
    #[test_case(b"\x1b[11~", Key::Function(1); "first function key as a numbered sequence")]
    fn test_a_key_is_parsed(bytes: &[u8], key: Key) {
        assert_eq!(parse(bytes).key, key);
    }

    #[test_case(b"\x1b["; "the start of a control sequence")]
    #[test_case(b"\x1b[1;5"; "a control sequence without its final byte")]
    #[test_case(b"\x1bO"; "the start of a single shift three sequence")]
    #[test_case(&"é".as_bytes()[..1]; "the first byte of a character")]
    fn test_more_bytes_are_needed(bytes: &[u8]) {
        assert!(matches!(
            ParsedTermEvent::try_from(bytes),
            Err(TermEventParseError::Need(_))
        ));
    }

    #[test]
    fn test_an_escape_is_only_a_key_once_it_is_known_that_nothing_follows_it() {
        assert!(matches!(
            ParsedTermEvent::try_from(&[ESCAPE][..]),
            Err(TermEventParseError::Need(_))
        ));

        let key_event: KeyEvent = KeyEvent::from(ESCAPE);
        assert_eq!(key_event.key, Key::Escape);
        assert_eq!(Vec::<u8>::from(&key_event), vec![ESCAPE]);
    }

    #[test]
    fn test_an_event_is_parsed_from_the_bytes_it_needs_and_no_more() {
        let parsed = ParsedTermEvent::try_from(&b"\x1b[Aj"[..]).unwrap();

        assert_eq!(parsed.len, 3);
    }

    #[test]
    fn test_pasted_text_is_parsed() {
        let bytes = b"\x1b[200~one\ntwo\x1b[201~j";

        let parsed = ParsedTermEvent::try_from(&bytes[..]).unwrap();

        assert_eq!(parsed.len, bytes.len() - 1);
        match parsed.event {
            TermEvent::Paste(text) => assert_eq!(text, "one\ntwo"),
            event => panic!("Expected pasted text but got {:?}.", event),
        }
    }

    #[test]
    fn test_the_rest_of_a_paste_is_waited_for() {
        assert!(matches!(
            ParsedTermEvent::try_from(&b"\x1b[200~one"[..]),
            Err(TermEventParseError::NeedRestOfPaste)
        ));
    }

    #[test]
    fn test_a_partial_paste_start_is_not_mistaken_for_another_sequence() {
        assert!(matches!(
            ParsedTermEvent::try_from(&b"\x1b[20"[..]),
            Err(TermEventParseError::Need(_))
        ));
    }

    #[test_case(b"\x1b[6n"; "a request the terminal never sends")]
    #[test_case(b"\x1b[?1049h"; "a sequence with a private parameter byte")]
    #[test_case(b"\x1b[10;20R"; "a report of where the cursor is")]
    #[test_case(b"\x1b]0;a title\x07"; "an operating system command")]
    #[test_case(b"\x1bP1$r0m\x1b\\"; "a device control string")]
    #[test_case(b"\x1b(B"; "a sequence which picks a character set")]
    #[test_case(b"\x80"; "a continuation byte with nothing to continue")]
    fn test_bytes_which_are_not_an_event_are_skipped(bytes: &[u8]) {
        assert_eq!(
            match ParsedTermEvent::try_from(bytes) {
                Err(TermEventParseError::Unrecognized(len)) => len,
                result => panic!("Expected the bytes to be skipped but got {:?}.", result),
            },
            bytes.len()
        );
    }

    // A key held with <Alt> sends an escape and then whatever that key sends on its own, which is
    // not a sequence of its own when the key is one like an arrow or backspace.
    #[test_case(b"\x1b\x1b[A", Key::Up; "up with alt")]
    #[test_case(b"\x1b\x1b[3~", Key::Delete; "delete with alt")]
    #[test_case(b"\x1b\x7f", Key::Backspace; "backspace with alt")]
    fn test_a_key_with_alt_which_is_not_a_sequence_is_parsed(bytes: &[u8], key: Key) {
        let key_event: KeyEvent = parse(bytes);

        assert_eq!(key_event.key, key);
        assert!(key_event.mods.contains(KeyMods::ALT));
    }
}

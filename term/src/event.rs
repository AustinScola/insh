use size::Size;

use std::fmt::{Display, Error as FmtError, Formatter};

use bitflags::bitflags;

#[derive(Debug, Clone)]
pub enum TermEvent {
    KeyEvent(KeyEvent),
    Resize(Size),
}

#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: KeyMods,
}

impl TryFrom<&[u8]> for TermEvent {
    type Error = TermEventParseError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.is_empty() {
            return Err(TermEventParseError::Need(1));
        }

        match bytes[0] {
            0 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Null,
                mods: KeyMods::NONE,
            })),
            1 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('a'),
                mods: KeyMods::CONTROL,
            })),
            2 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('b'),
                mods: KeyMods::CONTROL,
            })),
            3 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('c'),
                mods: KeyMods::CONTROL,
            })),
            4 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('d'),
                mods: KeyMods::CONTROL,
            })),
            5 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('e'),
                mods: KeyMods::CONTROL,
            })),
            6 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('f'),
                mods: KeyMods::CONTROL,
            })),
            7 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('g'),
                mods: KeyMods::CONTROL,
            })),
            8 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('h'),
                mods: KeyMods::CONTROL,
            })),
            9 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::HorizontalTab,
                mods: KeyMods::NONE,
            })),
            10 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('j'),
                mods: KeyMods::CONTROL,
            })),
            11 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('k'),
                mods: KeyMods::CONTROL,
            })),
            12 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('l'),
                mods: KeyMods::CONTROL,
            })),
            13 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::CarriageReturn,
                mods: KeyMods::NONE,
            })),
            14 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('n'),
                mods: KeyMods::CONTROL,
            })),
            15 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('o'),
                mods: KeyMods::CONTROL,
            })),
            16 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('p'),
                mods: KeyMods::CONTROL,
            })),
            17 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('q'),
                mods: KeyMods::CONTROL,
            })),
            18 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('r'),
                mods: KeyMods::CONTROL,
            })),
            19 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('s'),
                mods: KeyMods::CONTROL,
            })),
            20 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('t'),
                mods: KeyMods::CONTROL,
            })),
            21 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('u'),
                mods: KeyMods::CONTROL,
            })),
            22 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('v'),
                mods: KeyMods::CONTROL,
            })),
            23 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('w'),
                mods: KeyMods::CONTROL,
            })),
            24 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('x'),
                mods: KeyMods::CONTROL,
            })),
            25 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('y'),
                mods: KeyMods::CONTROL,
            })),
            26 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('z'),
                mods: KeyMods::CONTROL,
            })),
            27 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Escape,
                mods: KeyMods::NONE,
            })),
            28 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::FileSep,
                mods: KeyMods::NONE,
            })),
            29 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::GroupSep,
                mods: KeyMods::NONE,
            })),
            30 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::RecordSep,
                mods: KeyMods::NONE,
            })),
            31 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::UnitSep,
                mods: KeyMods::NONE,
            })),
            32 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char(' '),
                mods: KeyMods::NONE,
            })),
            33 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('!'),
                mods: KeyMods::SHIFT,
            })),
            34 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('"'),
                mods: KeyMods::SHIFT,
            })),
            35 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('#'),
                mods: KeyMods::SHIFT,
            })),
            36 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('$'),
                mods: KeyMods::SHIFT,
            })),
            37 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('%'),
                mods: KeyMods::SHIFT,
            })),
            38 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('&'),
                mods: KeyMods::SHIFT,
            })),
            39 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('\''),
                mods: KeyMods::NONE,
            })),
            40 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('('),
                mods: KeyMods::SHIFT,
            })),
            41 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char(')'),
                mods: KeyMods::SHIFT,
            })),
            42 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('*'),
                mods: KeyMods::SHIFT,
            })),
            43 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('+'),
                mods: KeyMods::SHIFT,
            })),
            44 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char(','),
                mods: KeyMods::NONE,
            })),
            45 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('-'),
                mods: KeyMods::NONE,
            })),
            46 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('.'),
                mods: KeyMods::NONE,
            })),
            47 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('/'),
                mods: KeyMods::NONE,
            })),
            48 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('0'),
                mods: KeyMods::NONE,
            })),
            49 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('1'),
                mods: KeyMods::NONE,
            })),
            50 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('2'),
                mods: KeyMods::NONE,
            })),
            51 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('3'),
                mods: KeyMods::NONE,
            })),
            52 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('4'),
                mods: KeyMods::NONE,
            })),
            53 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('5'),
                mods: KeyMods::NONE,
            })),
            54 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('6'),
                mods: KeyMods::NONE,
            })),
            55 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('7'),
                mods: KeyMods::NONE,
            })),
            56 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('8'),
                mods: KeyMods::NONE,
            })),
            57 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('9'),
                mods: KeyMods::NONE,
            })),
            58 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char(':'),
                mods: KeyMods::SHIFT,
            })),
            59 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char(';'),
                mods: KeyMods::NONE,
            })),
            60 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('<'),
                mods: KeyMods::SHIFT,
            })),
            61 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('='),
                mods: KeyMods::NONE,
            })),
            62 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('>'),
                mods: KeyMods::SHIFT,
            })),
            63 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('?'),
                mods: KeyMods::SHIFT,
            })),
            64 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('@'),
                mods: KeyMods::SHIFT,
            })),
            65 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('A'),
                mods: KeyMods::SHIFT,
            })),
            66 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('B'),
                mods: KeyMods::SHIFT,
            })),
            67 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('C'),
                mods: KeyMods::SHIFT,
            })),
            68 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('D'),
                mods: KeyMods::SHIFT,
            })),
            69 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('E'),
                mods: KeyMods::SHIFT,
            })),
            70 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('F'),
                mods: KeyMods::SHIFT,
            })),
            71 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('G'),
                mods: KeyMods::SHIFT,
            })),
            72 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('H'),
                mods: KeyMods::SHIFT,
            })),
            73 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('I'),
                mods: KeyMods::SHIFT,
            })),
            74 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('J'),
                mods: KeyMods::SHIFT,
            })),
            75 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('K'),
                mods: KeyMods::SHIFT,
            })),
            76 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('L'),
                mods: KeyMods::SHIFT,
            })),
            77 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('M'),
                mods: KeyMods::SHIFT,
            })),
            78 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('N'),
                mods: KeyMods::SHIFT,
            })),
            79 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('O'),
                mods: KeyMods::SHIFT,
            })),
            80 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('P'),
                mods: KeyMods::SHIFT,
            })),
            81 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('Q'),
                mods: KeyMods::SHIFT,
            })),
            82 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('R'),
                mods: KeyMods::SHIFT,
            })),
            83 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('S'),
                mods: KeyMods::SHIFT,
            })),
            84 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('T'),
                mods: KeyMods::SHIFT,
            })),
            85 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('U'),
                mods: KeyMods::SHIFT,
            })),
            86 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('V'),
                mods: KeyMods::SHIFT,
            })),
            87 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('W'),
                mods: KeyMods::SHIFT,
            })),
            88 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('X'),
                mods: KeyMods::SHIFT,
            })),
            89 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('Y'),
                mods: KeyMods::SHIFT,
            })),
            90 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('Z'),
                mods: KeyMods::SHIFT,
            })),
            91 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('['),
                mods: KeyMods::NONE,
            })),
            92 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('\\'),
                mods: KeyMods::NONE,
            })),
            93 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char(']'),
                mods: KeyMods::NONE,
            })),
            94 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('^'),
                mods: KeyMods::SHIFT,
            })),
            95 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('_'),
                mods: KeyMods::SHIFT,
            })),
            96 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('`'),
                mods: KeyMods::NONE,
            })),
            97 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('a'),
                mods: KeyMods::NONE,
            })),
            98 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('b'),
                mods: KeyMods::NONE,
            })),
            99 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('c'),
                mods: KeyMods::NONE,
            })),
            100 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('d'),
                mods: KeyMods::NONE,
            })),
            101 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('e'),
                mods: KeyMods::NONE,
            })),
            102 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('f'),
                mods: KeyMods::NONE,
            })),
            103 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('g'),
                mods: KeyMods::NONE,
            })),
            104 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('h'),
                mods: KeyMods::NONE,
            })),
            105 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('i'),
                mods: KeyMods::NONE,
            })),
            106 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('j'),
                mods: KeyMods::NONE,
            })),
            107 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('k'),
                mods: KeyMods::NONE,
            })),
            108 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('l'),
                mods: KeyMods::NONE,
            })),
            109 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('m'),
                mods: KeyMods::NONE,
            })),
            110 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('n'),
                mods: KeyMods::NONE,
            })),
            111 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('o'),
                mods: KeyMods::NONE,
            })),
            112 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('p'),
                mods: KeyMods::NONE,
            })),
            113 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('q'),
                mods: KeyMods::NONE,
            })),
            114 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('r'),
                mods: KeyMods::NONE,
            })),
            115 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('s'),
                mods: KeyMods::NONE,
            })),
            116 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('t'),
                mods: KeyMods::NONE,
            })),
            117 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('u'),
                mods: KeyMods::NONE,
            })),
            118 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('v'),
                mods: KeyMods::NONE,
            })),
            119 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('w'),
                mods: KeyMods::NONE,
            })),
            120 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('x'),
                mods: KeyMods::NONE,
            })),
            121 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('y'),
                mods: KeyMods::NONE,
            })),
            122 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('z'),
                mods: KeyMods::NONE,
            })),
            123 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('{'),
                mods: KeyMods::SHIFT,
            })),
            124 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('|'),
                mods: KeyMods::SHIFT,
            })),
            125 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('}'),
                mods: KeyMods::SHIFT,
            })),
            126 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('~'),
                mods: KeyMods::SHIFT,
            })),
            127 => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Delete,
                mods: KeyMods::NONE,
            })),
            value => Ok(TermEvent::KeyEvent(KeyEvent {
                key: Key::Unknown(value),
                mods: KeyMods::NONE,
            })),
        }
    }
}

#[derive(Debug)]
pub enum TermEventParseError {
    Need(usize),
}

impl TryInto<Vec<u8>> for &KeyEvent {
    type Error = KeyEventToBytesError;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        match self {
            KeyEvent { key: Key::Null, .. } => Ok(vec![0]),
            KeyEvent {
                key: Key::StartOfHeading,
                ..
            } => Ok(vec![1]),
            KeyEvent {
                key: Key::Char('a'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![1]),
            KeyEvent {
                key: Key::StartOfText,
                ..
            } => Ok(vec![2]),
            KeyEvent {
                key: Key::Char('b'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![2]),
            KeyEvent {
                key: Key::EndOfText,
                ..
            } => Ok(vec![3]),
            KeyEvent {
                key: Key::Char('c'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![3]),
            KeyEvent {
                key: Key::EndOfTransmission,
                ..
            } => Ok(vec![4]),
            KeyEvent {
                key: Key::Char('d'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![4]),
            KeyEvent {
                key: Key::Enquiry, ..
            } => Ok(vec![5]),
            KeyEvent {
                key: Key::Char('e'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![5]),
            KeyEvent { key: Key::Ack, .. } => Ok(vec![6]),
            KeyEvent {
                key: Key::Char('f'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![6]),
            KeyEvent { key: Key::Bell, .. } => Ok(vec![7]),
            KeyEvent {
                key: Key::Char('g'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![7]),
            KeyEvent {
                key: Key::Backspace,
                ..
            } => Ok(vec![8]),
            KeyEvent {
                key: Key::Char('h'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![8]),
            KeyEvent {
                key: Key::HorizontalTab,
                ..
            } => Ok(vec![9]),
            KeyEvent {
                key: Key::LineFeed, ..
            } => Ok(vec![10]),
            KeyEvent {
                key: Key::Char('j'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![10]),
            KeyEvent {
                key: Key::VertialTab,
                ..
            } => Ok(vec![11]),
            KeyEvent {
                key: Key::Char('k'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![11]),
            KeyEvent {
                key: Key::FormFeed, ..
            } => Ok(vec![12]),
            KeyEvent {
                key: Key::Char('l'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![12]),
            KeyEvent {
                key: Key::CarriageReturn,
                ..
            } => Ok(vec![13]),
            KeyEvent {
                key: Key::ShiftOut, ..
            } => Ok(vec![14]),
            KeyEvent {
                key: Key::Char('n'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![14]),
            KeyEvent {
                key: Key::ShiftIn, ..
            } => Ok(vec![15]),
            KeyEvent {
                key: Key::Char('o'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![15]),
            KeyEvent {
                key: Key::DataLinkEscape,
                ..
            } => Ok(vec![16]),
            KeyEvent {
                key: Key::Char('p'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![16]),
            KeyEvent {
                key: Key::DeviceControl1,
                ..
            } => Ok(vec![17]),
            KeyEvent {
                key: Key::Char('q'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![17]),
            KeyEvent {
                key: Key::DeviceControl2,
                ..
            } => Ok(vec![18]),
            KeyEvent {
                key: Key::Char('r'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![18]),
            KeyEvent {
                key: Key::DeviceControl3,
                ..
            } => Ok(vec![19]),
            KeyEvent {
                key: Key::Char('s'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![19]),
            KeyEvent {
                key: Key::DeviceControl4,
                ..
            } => Ok(vec![20]),
            KeyEvent {
                key: Key::Char('t'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![20]),
            KeyEvent { key: Key::Nack, .. } => Ok(vec![21]),
            KeyEvent {
                key: Key::Char('u'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![21]),
            KeyEvent {
                key: Key::SynchronousIdle,
                ..
            } => Ok(vec![22]),
            KeyEvent {
                key: Key::Char('v'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![22]),
            KeyEvent {
                key: Key::EndOfTransmissionBlock,
                ..
            } => Ok(vec![23]),
            KeyEvent {
                key: Key::Char('w'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![23]),
            KeyEvent {
                key: Key::Cancel, ..
            } => Ok(vec![24]),
            KeyEvent {
                key: Key::Char('x'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![24]),
            KeyEvent {
                key: Key::EndOfMedium,
                ..
            } => Ok(vec![25]),
            KeyEvent {
                key: Key::Char('y'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![25]),
            KeyEvent {
                key: Key::Substitute,
                ..
            } => Ok(vec![26]),
            KeyEvent {
                key: Key::Char('z'),
                mods: KeyMods::CONTROL,
            } => Ok(vec![26]),
            KeyEvent {
                key: Key::Escape, ..
            } => Ok(vec![27]),
            KeyEvent {
                key: Key::FileSep, ..
            } => Ok(vec![28]),
            KeyEvent {
                key: Key::GroupSep, ..
            } => Ok(vec![29]),
            KeyEvent {
                key: Key::RecordSep,
                ..
            } => Ok(vec![30]),
            KeyEvent {
                key: Key::UnitSep, ..
            } => Ok(vec![31]),
            KeyEvent {
                key: Key::Char(' '),
                mods: KeyMods::NONE,
            } => Ok(vec![32]),
            KeyEvent {
                key: Key::Char('!'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![33]),
            KeyEvent {
                key: Key::Char('"'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![34]),
            KeyEvent {
                key: Key::Char('#'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![35]),
            KeyEvent {
                key: Key::Char('$'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![36]),
            KeyEvent {
                key: Key::Char('%'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![37]),
            KeyEvent {
                key: Key::Char('&'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![38]),
            KeyEvent {
                key: Key::Char('\''),
                mods: KeyMods::NONE,
            } => Ok(vec![39]),
            KeyEvent {
                key: Key::Char('('),
                mods: KeyMods::SHIFT,
            } => Ok(vec![40]),
            KeyEvent {
                key: Key::Char(')'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![41]),
            KeyEvent {
                key: Key::Char('*'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![42]),
            KeyEvent {
                key: Key::Char('+'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![43]),
            KeyEvent {
                key: Key::Char(','),
                mods: KeyMods::NONE,
            } => Ok(vec![44]),
            KeyEvent {
                key: Key::Char('-'),
                mods: KeyMods::NONE,
            } => Ok(vec![45]),
            KeyEvent {
                key: Key::Char('.'),
                mods: KeyMods::NONE,
            } => Ok(vec![46]),
            KeyEvent {
                key: Key::Char('/'),
                mods: KeyMods::NONE,
            } => Ok(vec![47]),
            KeyEvent {
                key: Key::Char('0'),
                mods: KeyMods::NONE,
            } => Ok(vec![48]),
            KeyEvent {
                key: Key::Char('1'),
                mods: KeyMods::NONE,
            } => Ok(vec![49]),
            KeyEvent {
                key: Key::Char('2'),
                mods: KeyMods::NONE,
            } => Ok(vec![50]),
            KeyEvent {
                key: Key::Char('3'),
                mods: KeyMods::NONE,
            } => Ok(vec![51]),
            KeyEvent {
                key: Key::Char('4'),
                mods: KeyMods::NONE,
            } => Ok(vec![52]),
            KeyEvent {
                key: Key::Char('5'),
                mods: KeyMods::NONE,
            } => Ok(vec![53]),
            KeyEvent {
                key: Key::Char('6'),
                mods: KeyMods::NONE,
            } => Ok(vec![54]),
            KeyEvent {
                key: Key::Char('7'),
                mods: KeyMods::NONE,
            } => Ok(vec![55]),
            KeyEvent {
                key: Key::Char('8'),
                mods: KeyMods::NONE,
            } => Ok(vec![56]),
            KeyEvent {
                key: Key::Char('9'),
                mods: KeyMods::NONE,
            } => Ok(vec![57]),
            KeyEvent {
                key: Key::Char(':'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![58]),
            KeyEvent {
                key: Key::Char(';'),
                mods: KeyMods::NONE,
            } => Ok(vec![59]),
            KeyEvent {
                key: Key::Char('<'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![60]),
            KeyEvent {
                key: Key::Char('='),
                mods: KeyMods::NONE,
            } => Ok(vec![61]),
            KeyEvent {
                key: Key::Char('>'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![62]),
            KeyEvent {
                key: Key::Char('?'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![63]),
            KeyEvent {
                key: Key::Char('@'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![64]),
            KeyEvent {
                key: Key::Char('A'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![65]),
            KeyEvent {
                key: Key::Char('B'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![66]),
            KeyEvent {
                key: Key::Char('C'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![67]),
            KeyEvent {
                key: Key::Char('D'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![68]),
            KeyEvent {
                key: Key::Char('E'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![69]),
            KeyEvent {
                key: Key::Char('F'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![70]),
            KeyEvent {
                key: Key::Char('G'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![71]),
            KeyEvent {
                key: Key::Char('H'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![72]),
            KeyEvent {
                key: Key::Char('I'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![73]),
            KeyEvent {
                key: Key::Char('J'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![74]),
            KeyEvent {
                key: Key::Char('K'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![75]),
            KeyEvent {
                key: Key::Char('L'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![76]),
            KeyEvent {
                key: Key::Char('M'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![77]),
            KeyEvent {
                key: Key::Char('N'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![78]),
            KeyEvent {
                key: Key::Char('O'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![79]),
            KeyEvent {
                key: Key::Char('P'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![80]),
            KeyEvent {
                key: Key::Char('Q'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![81]),
            KeyEvent {
                key: Key::Char('R'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![82]),
            KeyEvent {
                key: Key::Char('S'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![83]),
            KeyEvent {
                key: Key::Char('T'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![84]),
            KeyEvent {
                key: Key::Char('U'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![85]),
            KeyEvent {
                key: Key::Char('V'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![86]),
            KeyEvent {
                key: Key::Char('W'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![87]),
            KeyEvent {
                key: Key::Char('X'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![88]),
            KeyEvent {
                key: Key::Char('Y'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![89]),
            KeyEvent {
                key: Key::Char('Z'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![90]),
            KeyEvent {
                key: Key::Char('['),
                mods: KeyMods::NONE,
            } => Ok(vec![91]),
            KeyEvent {
                key: Key::Char('\\'),
                mods: KeyMods::NONE,
            } => Ok(vec![92]),
            KeyEvent {
                key: Key::Char(']'),
                mods: KeyMods::NONE,
            } => Ok(vec![93]),
            KeyEvent {
                key: Key::Char('^'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![94]),
            KeyEvent {
                key: Key::Char('_'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![95]),
            KeyEvent {
                key: Key::Char('`'),
                mods: KeyMods::NONE,
            } => Ok(vec![96]),
            KeyEvent {
                key: Key::Char('a'),
                mods: KeyMods::NONE,
            } => Ok(vec![97]),
            KeyEvent {
                key: Key::Char('b'),
                mods: KeyMods::NONE,
            } => Ok(vec![98]),
            KeyEvent {
                key: Key::Char('c'),
                mods: KeyMods::NONE,
            } => Ok(vec![99]),
            KeyEvent {
                key: Key::Char('d'),
                mods: KeyMods::NONE,
            } => Ok(vec![100]),
            KeyEvent {
                key: Key::Char('e'),
                mods: KeyMods::NONE,
            } => Ok(vec![101]),
            KeyEvent {
                key: Key::Char('f'),
                mods: KeyMods::NONE,
            } => Ok(vec![102]),
            KeyEvent {
                key: Key::Char('g'),
                mods: KeyMods::NONE,
            } => Ok(vec![103]),
            KeyEvent {
                key: Key::Char('h'),
                mods: KeyMods::NONE,
            } => Ok(vec![104]),
            KeyEvent {
                key: Key::Char('i'),
                mods: KeyMods::NONE,
            } => Ok(vec![105]),
            KeyEvent {
                key: Key::Char('j'),
                mods: KeyMods::NONE,
            } => Ok(vec![106]),
            KeyEvent {
                key: Key::Char('k'),
                mods: KeyMods::NONE,
            } => Ok(vec![107]),
            KeyEvent {
                key: Key::Char('l'),
                mods: KeyMods::NONE,
            } => Ok(vec![108]),
            KeyEvent {
                key: Key::Char('m'),
                mods: KeyMods::NONE,
            } => Ok(vec![109]),
            KeyEvent {
                key: Key::Char('n'),
                mods: KeyMods::NONE,
            } => Ok(vec![110]),
            KeyEvent {
                key: Key::Char('o'),
                mods: KeyMods::NONE,
            } => Ok(vec![111]),
            KeyEvent {
                key: Key::Char('p'),
                mods: KeyMods::NONE,
            } => Ok(vec![112]),
            KeyEvent {
                key: Key::Char('q'),
                mods: KeyMods::NONE,
            } => Ok(vec![113]),
            KeyEvent {
                key: Key::Char('r'),
                mods: KeyMods::NONE,
            } => Ok(vec![114]),
            KeyEvent {
                key: Key::Char('s'),
                mods: KeyMods::NONE,
            } => Ok(vec![115]),
            KeyEvent {
                key: Key::Char('t'),
                mods: KeyMods::NONE,
            } => Ok(vec![116]),
            KeyEvent {
                key: Key::Char('u'),
                mods: KeyMods::NONE,
            } => Ok(vec![117]),
            KeyEvent {
                key: Key::Char('v'),
                mods: KeyMods::NONE,
            } => Ok(vec![118]),
            KeyEvent {
                key: Key::Char('w'),
                mods: KeyMods::NONE,
            } => Ok(vec![119]),
            KeyEvent {
                key: Key::Char('x'),
                mods: KeyMods::NONE,
            } => Ok(vec![120]),
            KeyEvent {
                key: Key::Char('y'),
                mods: KeyMods::NONE,
            } => Ok(vec![121]),
            KeyEvent {
                key: Key::Char('z'),
                mods: KeyMods::NONE,
            } => Ok(vec![122]),
            KeyEvent {
                key: Key::Char('{'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![123]),
            KeyEvent {
                key: Key::Char('|'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![124]),
            KeyEvent {
                key: Key::Char('}'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![125]),
            KeyEvent {
                key: Key::Char('~'),
                mods: KeyMods::SHIFT,
            } => Ok(vec![126]),
            KeyEvent {
                key: Key::Delete, ..
            } => Ok(vec![127]),
            KeyEvent {
                key: Key::Unknown(value),
                ..
            } => Ok(vec![*value]),
            KeyEvent {
                key: Key::Char(character),
                ..
            } => Err(KeyEventToBytesError::UnhandledKeyChar(*character)),
        }
    }
}

pub enum KeyEventToBytesError {
    UnhandledKeyChar(char),
}

impl Display for KeyEventToBytesError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::UnhandledKeyChar(character) => {
                write!(formatter, "Unhandled key char: {}", character)
            }
        }
    }
}

#[derive(Debug, Clone)]
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
    /// Backspace (same as <Ctrl>-h)
    Backspace,
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
    Delete,
    Unknown(u8),
}

bitflags! {
    /// Key modifiers.
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub struct KeyMods: u8 {
        const NONE = 0b0000_0000;
        const SHIFT = 0b0000_0001;
        const CONTROL = 0b0000_0010;
    }
}

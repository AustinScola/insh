/*!
What an escape sequence tells the terminal to do.

This is the layer above the structure of a sequence: it takes one apart into the command it stands
for, so that `CSI 3 A` becomes moving the cursor up three rows. A sequence which is not one of the
ones here reads as `None` rather than being thrown away, so that anything passing sequences on has
the sequence itself to fall back on.
*/

use std::fmt::{Display, Error as FmtError, Formatter};
use std::str;

use super::graphic_rendition::GraphicRendition;
use super::operating_system_command::OperatingSystemCommand;
use super::sequence::{AnsiEscapeSequence, ControlSequence, ESCAPE};

/// What an escape sequence tells the terminal to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlFunction {
    /// Move the cursor up the given number of rows, staying in the same column (`CSI n A`).
    CursorUp(u16),
    /// Move the cursor down the given number of rows, staying in the same column (`CSI n B`).
    CursorDown(u16),
    /// Move the cursor right the given number of columns (`CSI n C`).
    CursorForward(u16),
    /// Move the cursor left the given number of columns (`CSI n D`).
    CursorBack(u16),
    /// Move the cursor to the start of the row the given number of rows down (`CSI n E`).
    CursorNextLine(u16),
    /// Move the cursor to the start of the row the given number of rows up (`CSI n F`).
    CursorPreviousLine(u16),
    /// Move the cursor to the given column of the row it is on, counting from one (CHA, `CSI n G`).
    CursorColumn(u16),
    /// Move the cursor to the given row of the column it is in, counting from one (VPA, `CSI n d`).
    CursorRow(u16),
    /// Move the cursor to the given column, counting from one (HPA, ``CSI n ` ``). This is the same
    /// as [`CursorColumn`](Self::CursorColumn) on a terminal which writes a row at a time, which is
    /// every one of them, but the standard keeps the two apart.
    CharacterPositionAbsolute(u16),
    /// Move the cursor the given number of columns forward (HPR, `CSI n a`).
    CharacterPositionForward(u16),
    /// Move the cursor the given number of rows forward (VPR, `CSI n e`).
    LinePositionForward(u16),
    /// Move the cursor to the given row and column, both counting from one (`CSI n ; m H`, and
    /// `CSI n ; m f`, which does the same thing).
    CursorPosition { row: u16, column: u16 },
    /// Move the cursor forward the given number of tab stops (`CSI n I`).
    CursorForwardTabulation(u16),
    /// Move the cursor back the given number of tab stops (`CSI n Z`).
    CursorBackwardTabulation(u16),
    /// Remember where the cursor is, along with how text is being styled and which character sets
    /// are picked (`ESC 7`).
    SaveCursor,
    /// Put all of that back to what was remembered (`ESC 8`).
    RestoreCursor,
    /// Remember where the cursor is and nothing else (`CSI s`).
    SaveCursorPosition,
    /// Put the cursor back where it was remembered (`CSI u`).
    RestoreCursorPosition,
    /// Move the cursor down a row, scrolling if it is at the bottom of the scrolling region
    /// (`ESC D`).
    Index,
    /// Move the cursor up a row, scrolling if it is at the top of the scrolling region (`ESC M`).
    ReverseIndex,
    /// Move the cursor to the start of the next row, scrolling if it has to (`ESC E`).
    NextLine,
    /// Report where the cursor is (`CSI 6 n`).
    RequestCursorPosition,
    /// Where the cursor is, which is the reply to being asked (`CSI n ; m R`).
    CursorPositionReport { row: u16, column: u16 },

    /// Erase part of the screen (`CSI n J`).
    EraseInDisplay(EraseInDisplay),
    /// Erase part of the row the cursor is on (`CSI n K`).
    EraseInLine(EraseInLine),
    /// Replace the given number of characters from the cursor with blanks, without moving anything
    /// along (`CSI n X`).
    EraseCharacters(u16),
    /// Make room for the given number of characters at the cursor, pushing the rest of the row
    /// right (`CSI n @`).
    InsertCharacters(u16),
    /// Take the given number of characters out at the cursor, pulling the rest of the row left
    /// (`CSI n P`).
    DeleteCharacters(u16),
    /// Make room for the given number of rows at the cursor, pushing the rest down (`CSI n L`).
    InsertLines(u16),
    /// Take the given number of rows out at the cursor, pulling the rest up (`CSI n M`).
    DeleteLines(u16),
    /// Write the character before this the given number of times over again (`CSI n b`).
    Repeat(u16),

    /// Scroll the text up the given number of rows, bringing in blank ones (`CSI n S`).
    ScrollUp(u16),
    /// Scroll the text down the given number of rows, bringing in blank ones (`CSI n T`).
    ScrollDown(u16),
    /// Scroll the text left the given number of columns (SL, `CSI n SP @`).
    ScrollLeft(u16),
    /// Scroll the text right the given number of columns (SR, `CSI n SP A`).
    ScrollRight(u16),
    /// Set the rows between which the text scrolls, counting from one, or put it back to the whole
    /// screen when there is no bottom (DECSTBM, `CSI n ; m r`).
    ///
    /// NOTE: This one is not in ECMA-48. It is a DEC control function which every terminal has.
    SetScrollingRegion { top: u16, bottom: Option<u16> },
    /// Move on the given number of pages (NP, `CSI n U`).
    NextPage(u16),
    /// Move back the given number of pages (PP, `CSI n V`).
    PrecedingPage(u16),

    /// Put a tab stop where the cursor is (HTS, `ESC H`).
    SetTabStop,
    /// Take away tab stops (TBC, `CSI n g`).
    ClearTabStops(ClearTabStops),
    /// Set or clear tab stops (CTC, `CSI n W`).
    CursorTabulationControl(CursorTabulationControl),

    /// Move data to or from an auxiliary device, which is a printer on the terminals which have one
    /// (MC, `CSI n i`).
    MediaCopy(MediaCopy),

    /// Style the text which follows (`CSI n ; ... m`).
    SelectGraphicRendition(Vec<GraphicRendition>),

    /// Turn the given modes on or off (`CSI n h` and `CSI n l`, and `CSI ? n h` and `CSI ? n l`
    /// for the ones which are private to a terminal).
    SetMode {
        modes: Vec<Mode>,
        /// Whether the modes are being turned on rather than off.
        set: bool,
    },

    /// Ask what sort of terminal it is (`CSI n c`).
    RequestDeviceAttributes(u16),
    /// Ask for a report on the terminal, where `5` asks whether it is working and `6` asks where
    /// the cursor is (`CSI n n`).
    RequestDeviceStatus(u16),

    /// Do what an operating system command asks (`OSC ... ST`).
    OperatingSystemCommand(OperatingSystemCommand),

    /// Put the terminal back to how it starts up (`ESC c`).
    Reset,
    /// Fill the screen with `E`s, which is a way of checking the screen is lined up (`ESC # 8`).
    ScreenAlignmentTest,
    /// Pick the character set to use for one of the four slots (`ESC ( B` and the like).
    DesignateCharacterSet {
        /// Which of the four slots it is for, counting from zero.
        slot: u8,
        /// The byte which says which character set it is, such as `B` for ASCII.
        character_set: u8,
    },
    /// Have the keypad send the sequences a program reads rather than numbers (`ESC =` to turn it
    /// on and `ESC >` to turn it off).
    ApplicationKeypad(bool),
}

impl AnsiEscapeSequence {
    /// Return what the sequence tells the terminal to do, or `None` if it is not one which is
    /// recognized.
    pub fn control_function(&self) -> Option<ControlFunction> {
        match self {
            Self::ControlSequence(control) => ControlFunction::of_control_sequence(control),
            Self::OperatingSystemCommand(payload) => Some(ControlFunction::OperatingSystemCommand(
                OperatingSystemCommand::from(&payload[..]),
            )),
            Self::Escape {
                intermediates,
                final_byte,
            } => ControlFunction::of_escape(intermediates, *final_byte),
            _ => None,
        }
    }
}

impl Display for ControlFunction {
    /// Write the sequence for the control function, so that it can be put in amongst text which is
    /// being formatted.
    ///
    /// The only one which cannot be written this way is an operating system command which was not
    /// recognized and whose payload is not valid UTF-8, because a string cannot hold those bytes.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        let bytes: Vec<u8> = Vec::from(self);

        match str::from_utf8(&bytes) {
            Ok(string) => formatter.write_str(string),
            Err(_) => Err(FmtError),
        }
    }
}

impl From<&ControlFunction> for Vec<u8> {
    fn from(function: &ControlFunction) -> Self {
        // The parameters which most of these take, and what they mean when they are left out.
        const COUNT: u16 = 1;
        const ERASE: u16 = 0;

        match function {
            ControlFunction::CursorUp(rows) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*rows, COUNT)],
                b'A',
            ),
            ControlFunction::CursorDown(rows) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*rows, COUNT)],
                b'B',
            ),
            ControlFunction::CursorForward(columns) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*columns, COUNT)],
                b'C',
            ),
            ControlFunction::CursorBack(columns) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*columns, COUNT)],
                b'D',
            ),
            ControlFunction::CursorNextLine(rows) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*rows, COUNT)],
                b'E',
            ),
            ControlFunction::CursorPreviousLine(rows) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*rows, COUNT)],
                b'F',
            ),
            ControlFunction::CursorColumn(column) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*column, COUNT)],
                b'G',
            ),
            ControlFunction::CursorRow(row) => {
                ControlFunction::control_sequence(None, &[ControlFunction::omit(*row, COUNT)], b'd')
            }
            ControlFunction::CharacterPositionAbsolute(column) => {
                ControlFunction::control_sequence(
                    None,
                    &[ControlFunction::omit(*column, COUNT)],
                    b'`',
                )
            }
            ControlFunction::CharacterPositionForward(columns) => {
                ControlFunction::control_sequence(
                    None,
                    &[ControlFunction::omit(*columns, COUNT)],
                    b'a',
                )
            }
            ControlFunction::LinePositionForward(rows) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*rows, COUNT)],
                b'e',
            ),
            // NOTE: This is written the `H` way rather than the `f` way, which is the same thing in
            // one of the two spellings the sequence has.
            ControlFunction::CursorPosition { row, column } => ControlFunction::control_sequence(
                None,
                &[
                    ControlFunction::omit(*row, COUNT),
                    ControlFunction::omit(*column, COUNT),
                ],
                b'H',
            ),
            ControlFunction::CursorForwardTabulation(stops) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*stops, COUNT)],
                b'I',
            ),
            ControlFunction::CursorBackwardTabulation(stops) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*stops, COUNT)],
                b'Z',
            ),
            ControlFunction::SaveCursor => vec![ESCAPE, b'7'],
            ControlFunction::RestoreCursor => vec![ESCAPE, b'8'],
            ControlFunction::SaveCursorPosition => {
                ControlFunction::control_sequence(None, &[], b's')
            }
            ControlFunction::RestoreCursorPosition => {
                ControlFunction::control_sequence(None, &[], b'u')
            }
            ControlFunction::Index => vec![ESCAPE, b'D'],
            ControlFunction::ReverseIndex => vec![ESCAPE, b'M'],
            ControlFunction::NextLine => vec![ESCAPE, b'E'],
            ControlFunction::RequestCursorPosition => {
                ControlFunction::control_sequence(None, &[Some(6)], b'n')
            }
            ControlFunction::CursorPositionReport { row, column } => {
                ControlFunction::control_sequence(
                    None,
                    &[
                        ControlFunction::omit(*row, COUNT),
                        ControlFunction::omit(*column, COUNT),
                    ],
                    b'R',
                )
            }

            ControlFunction::EraseInDisplay(erase) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(u16::from(*erase), ERASE)],
                b'J',
            ),
            ControlFunction::EraseInLine(erase) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(u16::from(*erase), ERASE)],
                b'K',
            ),
            ControlFunction::EraseCharacters(characters) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*characters, COUNT)],
                b'X',
            ),
            ControlFunction::InsertCharacters(characters) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*characters, COUNT)],
                b'@',
            ),
            ControlFunction::DeleteCharacters(characters) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*characters, COUNT)],
                b'P',
            ),
            ControlFunction::InsertLines(rows) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*rows, COUNT)],
                b'L',
            ),
            ControlFunction::DeleteLines(rows) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*rows, COUNT)],
                b'M',
            ),
            ControlFunction::Repeat(times) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*times, COUNT)],
                b'b',
            ),

            ControlFunction::ScrollUp(rows) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*rows, COUNT)],
                b'S',
            ),
            ControlFunction::ScrollDown(rows) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*rows, COUNT)],
                b'T',
            ),
            ControlFunction::ScrollLeft(columns) => {
                ControlFunction::control_sequence_with_intermediate(
                    ControlFunction::omit(*columns, COUNT),
                    b' ',
                    b'@',
                )
            }
            ControlFunction::ScrollRight(columns) => {
                ControlFunction::control_sequence_with_intermediate(
                    ControlFunction::omit(*columns, COUNT),
                    b' ',
                    b'A',
                )
            }
            ControlFunction::NextPage(pages) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*pages, COUNT)],
                b'U',
            ),
            ControlFunction::PrecedingPage(pages) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(*pages, COUNT)],
                b'V',
            ),
            ControlFunction::SetScrollingRegion { top, bottom } => {
                ControlFunction::control_sequence(
                    None,
                    &[ControlFunction::omit(*top, COUNT), *bottom],
                    b'r',
                )
            }

            ControlFunction::SetTabStop => vec![ESCAPE, b'H'],
            ControlFunction::ClearTabStops(clear) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(u16::from(*clear), ERASE)],
                b'g',
            ),
            ControlFunction::CursorTabulationControl(control) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(u16::from(*control), ERASE)],
                b'W',
            ),
            ControlFunction::MediaCopy(copy) => ControlFunction::control_sequence(
                None,
                &[ControlFunction::omit(u16::from(*copy), ERASE)],
                b'i',
            ),

            ControlFunction::SelectGraphicRendition(renditions) => {
                let mut bytes: Self = vec![ESCAPE, b'['];
                bytes.extend_from_slice(&GraphicRendition::parameters(renditions));
                bytes.push(b'm');
                bytes
            }

            // NOTE: A sequence is either all private modes or all standard ones, so which it is
            // comes from the first of them.
            ControlFunction::SetMode { modes, set } => {
                let private: Option<u8> = match modes.first() {
                    Some(mode) if mode.is_private() => Some(b'?'),
                    _ => None,
                };
                let parameters: Vec<Option<u16>> =
                    modes.iter().map(|mode| Some(mode.number())).collect();

                ControlFunction::control_sequence(
                    private,
                    &parameters,
                    if *set { b'h' } else { b'l' },
                )
            }

            ControlFunction::RequestDeviceAttributes(attributes) => {
                ControlFunction::control_sequence(
                    None,
                    &[ControlFunction::omit(*attributes, 0)],
                    b'c',
                )
            }
            ControlFunction::RequestDeviceStatus(status) => {
                ControlFunction::control_sequence(None, &[Some(*status)], b'n')
            }

            ControlFunction::OperatingSystemCommand(command) => Self::from(
                &AnsiEscapeSequence::OperatingSystemCommand(Self::from(command)),
            ),

            ControlFunction::Reset => vec![ESCAPE, b'c'],
            ControlFunction::ScreenAlignmentTest => vec![ESCAPE, b'#', b'8'],
            ControlFunction::DesignateCharacterSet {
                slot,
                character_set,
            } => vec![
                ESCAPE,
                ControlFunction::CHARACTER_SET_SLOTS[usize::from(*slot)],
                *character_set,
            ],
            ControlFunction::ApplicationKeypad(on) => {
                vec![ESCAPE, if *on { b'=' } else { b'>' }]
            }
        }
    }
}

impl ControlFunction {
    /// The four slots which a character set can be picked for, in the order they are numbered.
    const CHARACTER_SET_SLOTS: &'static [u8] = b"()*+";

    /// Return the value as a parameter, or nothing at all when it is the given default, which is
    /// the shortest way of saying the same thing.
    fn omit(value: u16, default: u16) -> Option<u16> {
        match value == default {
            true => None,
            false => Some(value),
        }
    }

    /// Return the bytes of the control sequence with the given private marker, parameters and
    /// final byte.
    ///
    /// A parameter which is not there is left out, and the ones at the end which are not there do
    /// not even need their separators.
    fn control_sequence(
        private: Option<u8>,
        parameters: &[Option<u16>],
        final_byte: u8,
    ) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![ESCAPE, b'['];

        if let Some(private) = private {
            bytes.push(private);
        }

        let mut len: usize = parameters.len();
        while len > 0 && parameters[len - 1].is_none() {
            len -= 1;
        }

        for (position, parameter) in parameters[..len].iter().enumerate() {
            if position > 0 {
                bytes.push(b';');
            }
            if let Some(parameter) = parameter {
                bytes.extend_from_slice(parameter.to_string().as_bytes());
            }
        }

        bytes.push(final_byte);
        bytes
    }

    /// Return the bytes of the control sequence with the given parameter, intermediate and final
    /// byte.
    fn control_sequence_with_intermediate(
        parameter: Option<u16>,
        intermediate: u8,
        final_byte: u8,
    ) -> Vec<u8> {
        // NOTE: The intermediate goes between the parameters and the final byte, which is where
        // writing the sequence as if the intermediate were the final byte leaves it.
        let mut bytes: Vec<u8> = Self::control_sequence(None, &[parameter], intermediate);
        bytes.push(final_byte);
        bytes
    }

    /// Return what a control sequence tells the terminal to do.
    fn of_control_sequence(control: &ControlSequence) -> Option<Self> {
        // A sequence which is private to a terminal is only one of these when it is setting a mode.
        if control.private.is_some() {
            return Self::of_mode(control, true);
        }

        match control.intermediates.as_slice() {
            [] => Self::of_control_sequence_without_intermediates(control),
            // Scrolling sideways is the only thing here written with an intermediate.
            [b' '] => match control.final_byte {
                b'@' => Some(Self::ScrollLeft(control.count(0, 1))),
                b'A' => Some(Self::ScrollRight(control.count(0, 1))),
                _ => None,
            },
            // The rest of the sequences with intermediates set the terminal up rather than write on
            // it, and which of them a terminal has varies far too much to read them here.
            _ => None,
        }
    }

    /// Return what a sequence which sets modes says to do, or `None` if it names none.
    fn of_mode(control: &ControlSequence, private: bool) -> Option<Self> {
        // NOTE: Setting and resetting modes are the two control sequences whose parameter has no
        // default, so a sequence which names no mode is not saying to do anything.
        if control.parameters.is_empty() {
            return None;
        }

        match control.final_byte {
            b'h' | b'l' => Some(Self::SetMode {
                modes: Mode::all(control, private),
                set: control.final_byte == b'h',
            }),
            _ => None,
        }
    }

    /// Return what a control sequence with no intermediates tells the terminal to do.
    fn of_control_sequence_without_intermediates(control: &ControlSequence) -> Option<Self> {
        let function: Self = match control.final_byte {
            b'A' => Self::CursorUp(control.count(0, 1)),
            b'B' => Self::CursorDown(control.count(0, 1)),
            b'C' => Self::CursorForward(control.count(0, 1)),
            b'D' => Self::CursorBack(control.count(0, 1)),
            b'E' => Self::CursorNextLine(control.count(0, 1)),
            b'F' => Self::CursorPreviousLine(control.count(0, 1)),
            b'G' => Self::CursorColumn(control.count(0, 1)),
            b'H' | b'f' => Self::CursorPosition {
                row: control.count(0, 1),
                column: control.count(1, 1),
            },
            b'I' => Self::CursorForwardTabulation(control.count(0, 1)),
            b'J' => Self::EraseInDisplay(EraseInDisplay::from(control.parameter_or(0, 0))),
            b'K' => Self::EraseInLine(EraseInLine::from(control.parameter_or(0, 0))),
            b'L' => Self::InsertLines(control.count(0, 1)),
            b'M' => Self::DeleteLines(control.count(0, 1)),
            b'P' => Self::DeleteCharacters(control.count(0, 1)),
            // NOTE: This is the reply to being asked where the cursor is, which is the only one of
            // these a terminal sends back rather than being told.
            b'R' => Self::CursorPositionReport {
                row: control.count(0, 1),
                column: control.count(1, 1),
            },
            b'S' => Self::ScrollUp(control.count(0, 1)),
            b'T' => Self::ScrollDown(control.count(0, 1)),
            b'X' => Self::EraseCharacters(control.count(0, 1)),
            b'U' => Self::NextPage(control.count(0, 1)),
            b'V' => Self::PrecedingPage(control.count(0, 1)),
            b'W' => Self::CursorTabulationControl(CursorTabulationControl::from(
                control.parameter_or(0, 0),
            )),
            b'Z' => Self::CursorBackwardTabulation(control.count(0, 1)),
            b'@' => Self::InsertCharacters(control.count(0, 1)),
            b'`' => Self::CharacterPositionAbsolute(control.count(0, 1)),
            b'a' => Self::CharacterPositionForward(control.count(0, 1)),
            b'b' => Self::Repeat(control.count(0, 1)),
            b'c' => Self::RequestDeviceAttributes(control.parameter_or(0, 0)),
            b'd' => Self::CursorRow(control.count(0, 1)),
            b'e' => Self::LinePositionForward(control.count(0, 1)),
            b'g' => Self::ClearTabStops(ClearTabStops::from(control.parameter_or(0, 0))),
            b'h' | b'l' => {
                return Self::of_mode(control, false);
            }
            b'i' => Self::MediaCopy(MediaCopy::from(control.parameter_or(0, 0))),
            b'm' => Self::SelectGraphicRendition(GraphicRendition::all(&control.parameters)),
            b'n' => match control.parameter_or(0, 0) {
                6 => Self::RequestCursorPosition,
                status => Self::RequestDeviceStatus(status),
            },
            b'r' => Self::SetScrollingRegion {
                top: control.count(0, 1),
                bottom: control.parameter(1),
            },
            b's' => Self::SaveCursorPosition,
            b'u' => Self::RestoreCursorPosition,
            _ => {
                return None;
            }
        };

        Some(function)
    }

    /// Return what one of the short escape sequences tells the terminal to do.
    fn of_escape(intermediates: &[u8], final_byte: u8) -> Option<Self> {
        match intermediates {
            [] => match final_byte {
                b'7' => Some(Self::SaveCursor),
                b'8' => Some(Self::RestoreCursor),
                b'=' => Some(Self::ApplicationKeypad(true)),
                b'>' => Some(Self::ApplicationKeypad(false)),
                b'D' => Some(Self::Index),
                b'E' => Some(Self::NextLine),
                b'H' => Some(Self::SetTabStop),
                b'M' => Some(Self::ReverseIndex),
                b'c' => Some(Self::Reset),
                _ => None,
            },
            [b'#'] if final_byte == b'8' => Some(Self::ScreenAlignmentTest),
            [intermediate] => Self::CHARACTER_SET_SLOTS
                .iter()
                .position(|slot| slot == intermediate)
                .map(|slot| Self::DesignateCharacterSet {
                    slot: slot as u8,
                    character_set: final_byte,
                }),
            _ => None,
        }
    }
}

/// How much of the screen to erase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseInDisplay {
    /// From the cursor to the end of the screen (`0`).
    ToEnd,
    /// From the start of the screen to the cursor (`1`).
    ToStart,
    /// The whole screen (`2`).
    All,
    /// The whole screen and everything which has scrolled off the top of it (`3`).
    ///
    /// NOTE: ECMA-48 does not have this one. It comes from xterm and is widely supported.
    AllAndScrollback,
    /// A number which is none of the above.
    Unknown(u16),
}

impl From<EraseInDisplay> for u16 {
    fn from(erase: EraseInDisplay) -> Self {
        match erase {
            EraseInDisplay::ToEnd => 0,
            EraseInDisplay::ToStart => 1,
            EraseInDisplay::All => 2,
            EraseInDisplay::AllAndScrollback => 3,
            EraseInDisplay::Unknown(parameter) => parameter,
        }
    }
}

impl From<u16> for EraseInDisplay {
    fn from(parameter: u16) -> Self {
        match parameter {
            0 => Self::ToEnd,
            1 => Self::ToStart,
            2 => Self::All,
            3 => Self::AllAndScrollback,
            parameter => Self::Unknown(parameter),
        }
    }
}

/// How much of a row to erase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseInLine {
    /// From the cursor to the end of the row (`0`).
    ToEnd,
    /// From the start of the row to the cursor (`1`).
    ToStart,
    /// The whole row (`2`).
    All,
    /// A number which is none of the above.
    Unknown(u16),
}

impl From<EraseInLine> for u16 {
    fn from(erase: EraseInLine) -> Self {
        match erase {
            EraseInLine::ToEnd => 0,
            EraseInLine::ToStart => 1,
            EraseInLine::All => 2,
            EraseInLine::Unknown(parameter) => parameter,
        }
    }
}

impl From<u16> for EraseInLine {
    fn from(parameter: u16) -> Self {
        match parameter {
            0 => Self::ToEnd,
            1 => Self::ToStart,
            2 => Self::All,
            parameter => Self::Unknown(parameter),
        }
    }
}

/// Which tab stops to set or clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorTabulationControl {
    /// Set a tab stop in the column the cursor is in (`0`).
    SetColumn,
    /// Set a tab stop on the row the cursor is on (`1`).
    SetRow,
    /// Clear the tab stop in the column the cursor is in (`2`).
    ClearColumn,
    /// Clear the tab stop on the row the cursor is on (`3`).
    ClearRow,
    /// Clear every tab stop on the row the cursor is on (`4`).
    ClearRowColumns,
    /// Clear every tab stop in a column (`5`).
    ClearAllColumns,
    /// Clear every tab stop on a row (`6`).
    ClearAllRows,
    /// A number which is none of the above.
    Unknown(u16),
}

impl From<CursorTabulationControl> for u16 {
    fn from(control: CursorTabulationControl) -> Self {
        match control {
            CursorTabulationControl::SetColumn => 0,
            CursorTabulationControl::SetRow => 1,
            CursorTabulationControl::ClearColumn => 2,
            CursorTabulationControl::ClearRow => 3,
            CursorTabulationControl::ClearRowColumns => 4,
            CursorTabulationControl::ClearAllColumns => 5,
            CursorTabulationControl::ClearAllRows => 6,
            CursorTabulationControl::Unknown(parameter) => parameter,
        }
    }
}

impl From<u16> for CursorTabulationControl {
    fn from(parameter: u16) -> Self {
        match parameter {
            0 => Self::SetColumn,
            1 => Self::SetRow,
            2 => Self::ClearColumn,
            3 => Self::ClearRow,
            4 => Self::ClearRowColumns,
            5 => Self::ClearAllColumns,
            6 => Self::ClearAllRows,
            parameter => Self::Unknown(parameter),
        }
    }
}

/// What to do with an auxiliary device, which is a printer on the terminals which have one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCopy {
    /// Start sending to the primary device (`0`).
    ToPrimary,
    /// Start receiving from the primary device (`1`).
    FromPrimary,
    /// Start sending to the secondary device (`2`).
    ToSecondary,
    /// Start receiving from the secondary device (`3`).
    FromSecondary,
    /// Stop passing on what is received to the primary device (`4`).
    StopRelayToPrimary,
    /// Start passing on what is received to the primary device (`5`).
    StartRelayToPrimary,
    /// Stop passing on what is received to the secondary device (`6`).
    StopRelayToSecondary,
    /// Start passing on what is received to the secondary device (`7`).
    StartRelayToSecondary,
    /// A number which is none of the above.
    Unknown(u16),
}

impl From<MediaCopy> for u16 {
    fn from(copy: MediaCopy) -> Self {
        match copy {
            MediaCopy::ToPrimary => 0,
            MediaCopy::FromPrimary => 1,
            MediaCopy::ToSecondary => 2,
            MediaCopy::FromSecondary => 3,
            MediaCopy::StopRelayToPrimary => 4,
            MediaCopy::StartRelayToPrimary => 5,
            MediaCopy::StopRelayToSecondary => 6,
            MediaCopy::StartRelayToSecondary => 7,
            MediaCopy::Unknown(parameter) => parameter,
        }
    }
}

impl From<u16> for MediaCopy {
    fn from(parameter: u16) -> Self {
        match parameter {
            0 => Self::ToPrimary,
            1 => Self::FromPrimary,
            2 => Self::ToSecondary,
            3 => Self::FromSecondary,
            4 => Self::StopRelayToPrimary,
            5 => Self::StartRelayToPrimary,
            6 => Self::StopRelayToSecondary,
            7 => Self::StartRelayToSecondary,
            parameter => Self::Unknown(parameter),
        }
    }
}

/// Which tab stops to take away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearTabStops {
    /// The one where the cursor is (`0`).
    AtCursor,
    /// Every one of them (`3`).
    All,
    /// A number which is none of the above.
    Unknown(u16),
}

impl From<ClearTabStops> for u16 {
    fn from(clear: ClearTabStops) -> Self {
        match clear {
            ClearTabStops::AtCursor => 0,
            ClearTabStops::All => 3,
            ClearTabStops::Unknown(parameter) => parameter,
        }
    }
}

impl From<u16> for ClearTabStops {
    fn from(parameter: u16) -> Self {
        match parameter {
            0 => Self::AtCursor,
            3 => Self::All,
            parameter => Self::Unknown(parameter),
        }
    }
}

/// Something about the terminal which can be turned on and off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Writing a character makes room for it rather than writing over what is there (`4`).
    Insert,
    /// The terminal writes what is typed itself rather than leaving it to the program (`12`).
    SendReceive,
    /// A line feed also returns to the start of the row (`20`).
    LineFeedNewLine,
    /// The cursor keys send sequences beginning with an `ESC O` rather than an `ESC [` (`? 1`).
    ApplicationCursorKeys,
    /// The screen is a hundred and thirty two columns wide rather than eighty (`? 3`).
    WideColumns,
    /// The whole screen is drawn the other way round (`? 5`).
    ReverseVideo,
    /// Where the cursor is counted from the top of the scrolling region rather than the screen
    /// (`? 6`).
    Origin,
    /// Writing past the last column carries on at the start of the next row (`? 7`).
    AutoWrap,
    /// Holding a key down repeats it (`? 8`).
    AutoRepeat,
    /// The cursor is shown (`? 25`).
    CursorVisible,
    /// The cursor blinks (`? 12`).
    CursorBlink,
    /// The terminal reports a press of a mouse button (`? 1000`).
    ReportMouseClicks,
    /// The terminal reports the mouse moving while a button is held (`? 1002`).
    ReportMouseDrags,
    /// The terminal reports the mouse moving at all (`? 1003`).
    ReportMouseMotion,
    /// The terminal reports the window being focused and unfocused (`? 1004`).
    ReportFocus,
    /// Mouse reports are written the way UTF-8 writes a character (`? 1005`).
    Utf8MouseReports,
    /// Mouse reports are written as a control sequence, which is the only way of writing them
    /// which works past the two hundred and twenty third column (`? 1006`).
    ControlSequenceMouseReports,
    /// Mouse reports are written with the numbers as they are (`? 1015`).
    UrxvtMouseReports,
    /// The screen is the second one, which is not scrolled back through (`? 47`).
    AlternateScreen,
    /// The screen is the second one, which is cleared on the way back to the first (`? 1047`).
    AlternateScreenAndClear,
    /// Where the cursor is is remembered (`? 1048`).
    SaveCursor,
    /// The screen is the second one, remembering where the cursor is on the way in and out, which
    /// is what a program taking the terminal over uses (`? 1049`).
    AlternateScreenAndSaveCursor,
    /// Pasted text is wrapped so that it can be told apart from text which is typed (`? 2004`).
    BracketedPaste,
    /// A mode which is not one of the ones above.
    Unknown {
        /// Which mode it is.
        number: u16,
        /// Whether it is one which is private to a terminal rather than a standard one.
        private: bool,
    },
}

impl Mode {
    /// Return the number which stands for the mode.
    pub fn number(&self) -> u16 {
        match self {
            Self::Insert => 4,
            Self::SendReceive => 12,
            Self::LineFeedNewLine => 20,
            Self::ApplicationCursorKeys => 1,
            Self::WideColumns => 3,
            Self::ReverseVideo => 5,
            Self::Origin => 6,
            Self::AutoWrap => 7,
            Self::AutoRepeat => 8,
            Self::CursorBlink => 12,
            Self::CursorVisible => 25,
            Self::AlternateScreen => 47,
            Self::ReportMouseClicks => 1000,
            Self::ReportMouseDrags => 1002,
            Self::ReportMouseMotion => 1003,
            Self::ReportFocus => 1004,
            Self::Utf8MouseReports => 1005,
            Self::ControlSequenceMouseReports => 1006,
            Self::UrxvtMouseReports => 1015,
            Self::AlternateScreenAndClear => 1047,
            Self::SaveCursor => 1048,
            Self::AlternateScreenAndSaveCursor => 1049,
            Self::BracketedPaste => 2004,
            Self::Unknown { number, .. } => *number,
        }
    }

    /// Return whether the mode is one which is private to a terminal rather than a standard one.
    pub fn is_private(&self) -> bool {
        match self {
            Self::Insert | Self::SendReceive | Self::LineFeedNewLine => false,
            Self::Unknown { private, .. } => *private,
            _ => true,
        }
    }

    /// Return the modes which the parameters of a sequence setting them name.
    fn all(control: &ControlSequence, private: bool) -> Vec<Self> {
        control
            .parameters
            .iter()
            .map(|parameter| Self::of(parameter.value.unwrap_or(0), private))
            .collect()
    }

    /// Return the mode which the given number stands for.
    fn of(number: u16, private: bool) -> Self {
        if !private {
            return match number {
                4 => Self::Insert,
                12 => Self::SendReceive,
                20 => Self::LineFeedNewLine,
                number => Self::Unknown {
                    number,
                    private: false,
                },
            };
        }

        match number {
            1 => Self::ApplicationCursorKeys,
            3 => Self::WideColumns,
            5 => Self::ReverseVideo,
            6 => Self::Origin,
            7 => Self::AutoWrap,
            8 => Self::AutoRepeat,
            12 => Self::CursorBlink,
            25 => Self::CursorVisible,
            47 => Self::AlternateScreen,
            1047 => Self::AlternateScreenAndClear,
            1000 => Self::ReportMouseClicks,
            1002 => Self::ReportMouseDrags,
            1003 => Self::ReportMouseMotion,
            1004 => Self::ReportFocus,
            1005 => Self::Utf8MouseReports,
            1006 => Self::ControlSequenceMouseReports,
            1015 => Self::UrxvtMouseReports,
            1048 => Self::SaveCursor,
            1049 => Self::AlternateScreenAndSaveCursor,
            2004 => Self::BracketedPaste,
            number => Self::Unknown {
                number,
                private: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::graphic_rendition::Color;
    use crate::sequence::ParsedAnsiEscapeSequence;

    use test_case::test_case;

    /// Return what the sequence the bytes are tells the terminal to do.
    fn parse(bytes: &[u8]) -> Option<ControlFunction> {
        ParsedAnsiEscapeSequence::try_from(bytes)
            .unwrap()
            .sequence
            .control_function()
    }

    #[test_case(b"\x1b[A", ControlFunction::CursorUp(1); "moving the cursor up by the default")]
    #[test_case(b"\x1b[3A", ControlFunction::CursorUp(3); "moving the cursor up")]
    #[test_case(b"\x1b[2B", ControlFunction::CursorDown(2); "moving the cursor down")]
    #[test_case(b"\x1b[4C", ControlFunction::CursorForward(4); "moving the cursor forward")]
    #[test_case(b"\x1b[5D", ControlFunction::CursorBack(5); "moving the cursor back")]
    #[test_case(b"\x1b[2E", ControlFunction::CursorNextLine(2); "moving the cursor to the next line")]
    #[test_case(b"\x1b[2F", ControlFunction::CursorPreviousLine(2); "moving the cursor to the previous line")]
    #[test_case(b"\x1b[7G", ControlFunction::CursorColumn(7); "moving the cursor to a column")]
    #[test_case(b"\x1b[7d", ControlFunction::CursorRow(7); "moving the cursor to a row")]
    #[test_case(b"\x1b[2I", ControlFunction::CursorForwardTabulation(2); "moving the cursor forward by tab stops")]
    #[test_case(b"\x1b[2Z", ControlFunction::CursorBackwardTabulation(2); "moving the cursor back by tab stops")]
    #[test_case(b"\x1b[s", ControlFunction::SaveCursorPosition; "saving where the cursor is")]
    #[test_case(b"\x1b[u", ControlFunction::RestoreCursorPosition; "restoring where the cursor is")]
    #[test_case(b"\x1b7", ControlFunction::SaveCursor; "saving the cursor")]
    #[test_case(b"\x1b8", ControlFunction::RestoreCursor; "restoring the cursor")]
    #[test_case(b"\x1bD", ControlFunction::Index; "indexing")]
    #[test_case(b"\x1bM", ControlFunction::ReverseIndex; "reverse indexing")]
    #[test_case(b"\x1bE", ControlFunction::NextLine; "moving to the next line")]
    #[test_case(b"\x1b[6n", ControlFunction::RequestCursorPosition; "asking where the cursor is")]
    #[test_case(b"\x1b[5n", ControlFunction::RequestDeviceStatus(5); "asking whether the terminal is working")]
    #[test_case(b"\x1b[3X", ControlFunction::EraseCharacters(3); "erasing characters")]
    #[test_case(b"\x1b[3@", ControlFunction::InsertCharacters(3); "inserting characters")]
    #[test_case(b"\x1b[3P", ControlFunction::DeleteCharacters(3); "deleting characters")]
    #[test_case(b"\x1b[3L", ControlFunction::InsertLines(3); "inserting lines")]
    #[test_case(b"\x1b[3M", ControlFunction::DeleteLines(3); "deleting lines")]
    #[test_case(b"\x1b[3b", ControlFunction::Repeat(3); "repeating a character")]
    #[test_case(b"\x1b[2S", ControlFunction::ScrollUp(2); "scrolling up")]
    #[test_case(b"\x1b[2T", ControlFunction::ScrollDown(2); "scrolling down")]
    #[test_case(b"\x1bH", ControlFunction::SetTabStop; "setting a tab stop")]
    #[test_case(b"\x1b[3g", ControlFunction::ClearTabStops(ClearTabStops::All); "clearing every tab stop")]
    #[test_case(b"\x1b[g", ControlFunction::ClearTabStops(ClearTabStops::AtCursor); "clearing the tab stop at the cursor")]
    #[test_case(b"\x1bc", ControlFunction::Reset; "resetting the terminal")]
    #[test_case(b"\x1b#8", ControlFunction::ScreenAlignmentTest; "testing the screen alignment")]
    #[test_case(b"\x1b=", ControlFunction::ApplicationKeypad(true); "turning the application keypad on")]
    #[test_case(b"\x1b>", ControlFunction::ApplicationKeypad(false); "turning the application keypad off")]
    fn test_a_sequence_is_read(bytes: &[u8], function: ControlFunction) {
        assert_eq!(parse(bytes), Some(function.clone()));
        assert_eq!(Vec::<u8>::from(&function), bytes);
    }

    /// Check that the bytes are written back out as the given ones, and that those say the same
    /// thing when they are read again.
    fn assert_shortest(bytes: &[u8], shortest: &[u8]) {
        let function: ControlFunction = parse(bytes).unwrap();

        assert_eq!(Vec::<u8>::from(&function), shortest);
        assert_eq!(parse(shortest), Some(function));
    }

    #[test_case(b"\x1b[1A", b"\x1b[A"; "a count which is the default")]
    #[test_case(b"\x1b[1 @", b"\x1b[ @"; "a scroll left which is the default")]
    #[test_case(b"\x1b[0W", b"\x1b[W"; "a cursor tabulation control which is the default")]
    #[test_case(b"\x1b[0i", b"\x1b[i"; "a media copy which is the default")]
    #[test_case(b"\x1b[1;1H", b"\x1b[H"; "both of the numbers a cursor is moved to")]
    #[test_case(b"\x1b[10;1H", b"\x1b[10H"; "the column a cursor is moved to")]
    #[test_case(b"\x1b[1;20H", b"\x1b[;20H"; "the row a cursor is moved to")]
    #[test_case(b"\x1b[10;20f", b"\x1b[10;20H"; "the other spelling of moving the cursor")]
    #[test_case(b"\x1b[0J", b"\x1b[J"; "an erase which is the default")]
    #[test_case(b"\x1b[0K", b"\x1b[K"; "an erase in a line which is the default")]
    #[test_case(b"\x1b[0g", b"\x1b[g"; "a tab clear which is the default")]
    #[test_case(b"\x1b[1;r", b"\x1b[r"; "a scrolling region which is the whole screen")]
    #[test_case(b"\x1b[1;24r", b"\x1b[;24r"; "the top of a scrolling region")]
    #[test_case(b"\x1b[0c", b"\x1b[c"; "asking what sort of terminal it is")]
    #[test_case(b"\x1b[0A", b"\x1b[A"; "a count of zero, which means one")]
    #[test_case(b"\x1b[0m", b"\x1b[m"; "a reset of how text is styled")]
    #[test_case(b"\x1b[38;5;1m", b"\x1b[31m"; "a basic colour written as a palette entry")]
    #[test_case(b"\x1b[38;5;9m", b"\x1b[91m"; "a bright colour written as a palette entry")]
    #[test_case(b"\x1b[38:5:196m", b"\x1b[38;5;196m"; "a palette entry written as sub-parameters")]
    #[test_case(b"\x1b[38:2::255:0:0m", b"\x1b[38;2;255;0;0m"; "components written as sub-parameters")]
    #[test_case(b"\x1b[4:2m", b"\x1b[21m"; "an underline of two lines")]
    fn test_a_sequence_is_written_back_out_as_short_as_it_goes(bytes: &[u8], shortest: &[u8]) {
        assert_shortest(bytes, shortest);
    }

    // Every sequence which is understood has to survive being read and written back out, because
    // anything relaying them writes out what it read.
    #[test_case(b"\x1b[3A"; "moving the cursor up")]
    #[test_case(b"\x1b[10;20H"; "moving the cursor")]
    #[test_case(b"\x1b[2J"; "erasing the screen")]
    #[test_case(b"\x1b[3J"; "erasing the screen and the scrollback")]
    #[test_case(b"\x1b[2K"; "erasing a line")]
    #[test_case(b"\x1b[5;20r"; "setting the scrolling region")]
    #[test_case(b"\x1b[3g"; "clearing every tab stop")]
    #[test_case(b"\x1b[5`"; "the character position absolutely")]
    #[test_case(b"\x1b[5a"; "the character position forward")]
    #[test_case(b"\x1b[5e"; "the line position forward")]
    #[test_case(b"\x1b[2U"; "the next page")]
    #[test_case(b"\x1b[2V"; "the preceding page")]
    #[test_case(b"\x1b[2W"; "setting a tab stop with cursor tabulation control")]
    #[test_case(b"\x1b[5i"; "starting a relay to a printer")]
    #[test_case(b"\x1b[3 @"; "scrolling left")]
    #[test_case(b"\x1b[3 A"; "scrolling right")]
    #[test_case(b"\x1b[?1049h"; "turning the alternate screen on")]
    #[test_case(b"\x1b[?1049l"; "turning the alternate screen off")]
    #[test_case(b"\x1b[?47h"; "turning the older alternate screen on")]
    #[test_case(b"\x1b[?1047h"; "turning the middle alternate screen on")]
    #[test_case(b"\x1b[?25l"; "hiding the cursor")]
    #[test_case(b"\x1b[?2004h"; "turning bracketed paste on")]
    #[test_case(b"\x1b[?1000;1002;1006h"; "turning several mouse modes on at once")]
    #[test_case(b"\x1b[4h"; "turning a standard mode on")]
    #[test_case(b"\x1b[?9999h"; "turning a mode which is not known on")]
    #[test_case(b"\x1b[1;4;7;31;44m"; "styling the text several ways")]
    #[test_case(b"\x1b[38;2;255;128;0m"; "a colour given as components")]
    #[test_case(b"\x1b[58;5;196m"; "the colour of an underline")]
    #[test_case(b"\x1b[58;5;1m"; "a basic colour for an underline, which has no short spelling")]
    #[test_case(b"\x1b[4:3m"; "a curly underline")]
    #[test_case(b"\x1b[4:0m"; "an underline turned off by its sub-parameter")]
    #[test_case(b"\x1b[24m"; "an underline turned off by its own parameter")]
    #[test_case(b"\x1b[200m"; "a rendition which is not known")]
    #[test_case(b"\x1b[6n"; "asking where the cursor is")]
    #[test_case(b"\x1b[10;20R"; "reporting where the cursor is")]
    #[test_case(b"\x1b[s"; "saving where the cursor is")]
    #[test_case(b"\x1b7"; "saving the cursor")]
    #[test_case(b"\x1bc"; "resetting the terminal")]
    #[test_case(b"\x1b#8"; "testing the screen alignment")]
    #[test_case(b"\x1b(B"; "picking a character set")]
    #[test_case(b"\x1b)0"; "picking a character set for another slot")]
    #[test_case(b"\x1b="; "turning the application keypad on")]
    #[test_case(b"\x1b]0;a title\x07"; "setting the window title")]
    #[test_case(b"\x1b]52;c;aGVsbG8=\x07"; "writing the clipboard")]
    #[test_case(b"\x1b]8;id=1;https://example.com\x07"; "a hyperlink")]
    #[test_case(b"\x1b]777;notify;hello\x07"; "an operating system command which is not known")]
    fn test_a_sequence_survives_being_read_and_written(bytes: &[u8]) {
        assert_shortest(bytes, bytes);
    }

    #[test_case(b"\x1b[H", 1, 1; "with both left out")]
    #[test_case(b"\x1b[10;20H", 10, 20; "with both given")]
    #[test_case(b"\x1b[10;20f", 10, 20; "written the other way")]
    #[test_case(b"\x1b[;20H", 1, 20; "with the row left out")]
    fn test_moving_the_cursor_is_read(bytes: &[u8], row: u16, column: u16) {
        assert_eq!(
            parse(bytes),
            Some(ControlFunction::CursorPosition { row, column })
        );
    }

    #[test_case(b"\x1b[J", EraseInDisplay::ToEnd; "to the end by default")]
    #[test_case(b"\x1b[0J", EraseInDisplay::ToEnd; "to the end")]
    #[test_case(b"\x1b[1J", EraseInDisplay::ToStart; "to the start")]
    #[test_case(b"\x1b[2J", EraseInDisplay::All; "all of it")]
    #[test_case(b"\x1b[3J", EraseInDisplay::AllAndScrollback; "all of it and the scrollback")]
    fn test_erasing_the_display_is_read(bytes: &[u8], erase: EraseInDisplay) {
        assert_eq!(parse(bytes), Some(ControlFunction::EraseInDisplay(erase)));
    }

    #[test_case(b"\x1b[K", EraseInLine::ToEnd; "to the end by default")]
    #[test_case(b"\x1b[1K", EraseInLine::ToStart; "to the start")]
    #[test_case(b"\x1b[2K", EraseInLine::All; "all of it")]
    fn test_erasing_a_line_is_read(bytes: &[u8], erase: EraseInLine) {
        assert_eq!(parse(bytes), Some(ControlFunction::EraseInLine(erase)));
    }

    #[test_case(b"\x1b[2;10r", 2, Some(10); "with both rows")]
    #[test_case(b"\x1b[r", 1, None; "with neither, which is the whole screen")]
    fn test_setting_the_scrolling_region_is_read(bytes: &[u8], top: u16, bottom: Option<u16>) {
        assert_eq!(
            parse(bytes),
            Some(ControlFunction::SetScrollingRegion { top, bottom })
        );
    }

    #[test_case(b"\x1b[?1049h", Mode::AlternateScreenAndSaveCursor, true; "turning the alternate screen on")]
    #[test_case(b"\x1b[?1049l", Mode::AlternateScreenAndSaveCursor, false; "turning the alternate screen off")]
    #[test_case(b"\x1b[?25h", Mode::CursorVisible, true; "showing the cursor")]
    #[test_case(b"\x1b[?25l", Mode::CursorVisible, false; "hiding the cursor")]
    #[test_case(b"\x1b[?2004h", Mode::BracketedPaste, true; "turning bracketed paste on")]
    #[test_case(b"\x1b[?1h", Mode::ApplicationCursorKeys, true; "turning application cursor keys on")]
    #[test_case(b"\x1b[?7l", Mode::AutoWrap, false; "turning auto wrap off")]
    #[test_case(b"\x1b[4h", Mode::Insert, true; "turning insert on, which is not private")]
    fn test_setting_a_mode_is_read(bytes: &[u8], mode: Mode, set: bool) {
        assert_eq!(
            parse(bytes),
            Some(ControlFunction::SetMode {
                modes: vec![mode],
                set
            })
        );
    }

    #[test]
    fn test_setting_more_than_one_mode_at_once_is_read() {
        assert_eq!(
            parse(b"\x1b[?1000;1002;1006h"),
            Some(ControlFunction::SetMode {
                modes: vec![
                    Mode::ReportMouseClicks,
                    Mode::ReportMouseDrags,
                    Mode::ControlSequenceMouseReports,
                ],
                set: true,
            })
        );
    }

    #[test]
    fn test_a_mode_which_is_not_known_is_kept() {
        assert_eq!(
            parse(b"\x1b[?9999h"),
            Some(ControlFunction::SetMode {
                modes: vec![Mode::Unknown {
                    number: 9999,
                    private: true
                }],
                set: true,
            })
        );
    }

    #[test]
    fn test_styling_the_text_is_read() {
        assert_eq!(
            parse(b"\x1b[1;31m"),
            Some(ControlFunction::SelectGraphicRendition(vec![
                GraphicRendition::Bold,
                GraphicRendition::Foreground(Color::Red),
            ]))
        );
    }

    #[test]
    fn test_an_operating_system_command_is_read() {
        assert_eq!(
            parse(b"\x1b]0;a title\x07"),
            Some(ControlFunction::OperatingSystemCommand(
                OperatingSystemCommand::SetIconNameAndWindowTitle("a title".to_string())
            ))
        );
    }

    #[test]
    fn test_picking_a_character_set_is_read() {
        assert_eq!(
            parse(b"\x1b(B"),
            Some(ControlFunction::DesignateCharacterSet {
                slot: 0,
                character_set: b'B',
            })
        );
        assert_eq!(
            parse(b"\x1b)0"),
            Some(ControlFunction::DesignateCharacterSet {
                slot: 1,
                character_set: b'0',
            })
        );
    }

    #[test]
    fn test_where_the_cursor_is_reported_to_be_is_read() {
        assert_eq!(
            parse(b"\x1b[10;20R"),
            Some(ControlFunction::CursorPositionReport {
                row: 10,
                column: 20
            })
        );
    }

    #[test_case(b"\x1b[4 q"; "a sequence with an intermediate which is not scrolling sideways")]
    #[test_case(b"\x1b[h"; "setting a mode with no mode named, which has no default")]
    #[test_case(b"\x1b[?l"; "resetting a private mode with none named")]
    #[test_case(b"\x1b[?6c"; "a reply saying what sort of terminal it is")]
    #[test_case(b"\x1bP1$r0m\x1b\\"; "a device control string")]
    #[test_case(b"\x1b_a payload\x1b\\"; "an application program command")]
    #[test_case(b"\x1bOP"; "a single shift three sequence")]
    fn test_a_sequence_which_is_not_recognized_is_left_alone(bytes: &[u8]) {
        assert_eq!(parse(bytes), None);
    }
}

/*!
The ways of styling text which a select graphic rendition sequence turns on and off.
*/

use super::sequence::Parameter;

/// A way of styling the text which is written after it.
///
/// These are the parameters of ECMA-48 clause 8.3.117 unless the one in question says otherwise.
/// Proportional spacing comes from ITU T.416 rather than ECMA-48, and the colour of an underline
/// and superscript and subscript are not in any standard at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphicRendition {
    /// Turn every one of these off (`0`).
    Reset,
    /// Bold or bright (`1`).
    Bold,
    /// Faint or dim (`2`).
    Faint,
    /// Italic (`3`).
    Italic,
    /// Underlined (`4`, or `21` for a doubly underlined one).
    ///
    /// NOTE: ECMA-48 says `21` is a double underline, and that is what it is taken as here, but
    /// some terminals take it as turning bold off instead.
    Underline(Underline),
    /// Blinking less than a hundred and fifty times a minute (`5`).
    SlowBlink,
    /// Blinking more than a hundred and fifty times a minute (`6`).
    RapidBlink,
    /// The foreground and background swapped (`7`).
    Reverse,
    /// Not shown at all (`8`).
    Conceal,
    /// Crossed out (`9`).
    Strike,
    /// One of the fonts, where the zeroth is the default one (`10` to `19`).
    Font(u8),
    /// Fraktur, which is hardly ever supported (`20`).
    Fraktur,
    /// Neither bold nor faint (`22`).
    NormalIntensity,
    /// Neither italic nor fraktur (`23`).
    NotItalic,
    /// Not underlined (`24`).
    NotUnderlined,
    /// Not blinking (`25`).
    NotBlinking,
    /// Proportionally spaced (`26`). This is from ITU T.416, not ECMA-48.
    Proportional,
    /// Not reversed (`27`).
    NotReversed,
    /// Shown after having been concealed (`28`).
    Reveal,
    /// Not crossed out (`29`).
    NotStruck,
    /// The colour to write the text in (`30` to `37`, `38`, `39` and `90` to `97`).
    Foreground(Color),
    /// The colour to write the text on (`40` to `47`, `48`, `49` and `100` to `107`).
    Background(Color),
    /// The colour to underline the text in (`58` and `59`).
    ///
    /// NOTE: This is in no standard. It comes from kitty and is in VTE, mintty and iTerm2.
    UnderlineColor(Color),
    /// Not proportionally spaced (`50`). This is from ITU T.416, not ECMA-48.
    NotProportional,
    /// Framed (`51`).
    Framed,
    /// Encircled (`52`).
    Encircled,
    /// Overlined (`53`).
    Overlined,
    /// Neither framed nor encircled (`54`).
    NotFramedOrEncircled,
    /// Not overlined (`55`).
    NotOverlined,
    /// One of the ideogram attributes, which are hardly ever supported (`60` to `65`).
    Ideogram(u8),
    /// Superscript (`73`). This is in no standard and comes from mintty.
    Superscript,
    /// Subscript (`74`). This is in no standard and comes from mintty.
    Subscript,
    /// Neither superscript nor subscript (`75`). This is in no standard and comes from mintty.
    NeitherSuperscriptNorSubscript,
    /// A parameter which is not one of the ones above.
    Unknown(u16),
}

impl GraphicRendition {
    /// Return the parameters of the select graphic rendition sequence which says to style the text
    /// with the given renditions, written as short as they go.
    pub fn parameters(renditions: &[Self]) -> Vec<u8> {
        // A sequence with no parameters at all is a reset, which is the shortest way of saying so.
        if renditions == [Self::Reset] {
            return Vec::new();
        }

        let mut parameters: Vec<String> = Vec::new();
        for rendition in renditions {
            rendition.write(&mut parameters);
        }

        parameters.join(";").into_bytes()
    }

    /// Write the rendition onto the given parameters.
    fn write(&self, parameters: &mut Vec<String>) {
        let parameter: u16 = match self {
            Self::Reset => 0,
            Self::Bold => 1,
            Self::Faint => 2,
            Self::Italic => 3,
            Self::Underline(underline) => {
                parameters.push(underline.parameter());
                return;
            }
            Self::SlowBlink => 5,
            Self::RapidBlink => 6,
            Self::Reverse => 7,
            Self::Conceal => 8,
            Self::Strike => 9,
            Self::Font(font) => 10 + u16::from(*font),
            Self::Fraktur => 20,
            Self::NormalIntensity => 22,
            Self::NotItalic => 23,
            Self::NotUnderlined => 24,
            Self::NotBlinking => 25,
            Self::Proportional => 26,
            Self::NotReversed => 27,
            Self::Reveal => 28,
            Self::NotStruck => 29,
            Self::Foreground(color) => {
                color.write(Some(30), 38, 39, parameters);
                return;
            }
            Self::Background(color) => {
                color.write(Some(40), 48, 49, parameters);
                return;
            }
            // NOTE: There is no short way of writing the colour of an underline, so the basic
            // colours have to go through the palette, whose first sixteen entries they are.
            Self::UnderlineColor(color) => {
                color.write(None, 58, 59, parameters);
                return;
            }
            Self::NotProportional => 50,
            Self::Framed => 51,
            Self::Encircled => 52,
            Self::Overlined => 53,
            Self::NotFramedOrEncircled => 54,
            Self::NotOverlined => 55,
            Self::Ideogram(ideogram) => 60 + u16::from(*ideogram),
            Self::Superscript => 73,
            Self::Subscript => 74,
            Self::NeitherSuperscriptNorSubscript => 75,
            Self::Unknown(parameter) => *parameter,
        };

        parameters.push(parameter.to_string());
    }

    /// Return what the parameters of a select graphic rendition sequence say to style the text
    /// with.
    ///
    /// A sequence with no parameters at all is a reset, which is what `CSI m` on its own means.
    pub fn all(parameters: &[Parameter]) -> Vec<Self> {
        if parameters.is_empty() {
            return vec![Self::Reset];
        }

        let mut renditions: Vec<Self> = Vec::new();
        let mut position: usize = 0;

        while position < parameters.len() {
            let parameter: &Parameter = &parameters[position];
            // A parameter which is left out is a zero here rather than being an error.
            let value: u16 = parameter.value.unwrap_or(0);

            // The colours are the only parameters which are more than one number, either as
            // sub-parameters of this one or as the parameters which follow it.
            let rendition: Self = match value {
                38 | 48 | 58 => {
                    let (color, used) = match Color::read(parameters, position) {
                        Some((color, used)) => (color, used),
                        None => {
                            renditions.push(Self::Unknown(value));
                            position += 1;
                            continue;
                        }
                    };
                    position += used;

                    match value {
                        38 => Self::Foreground(color),
                        48 => Self::Background(color),
                        _ => Self::UnderlineColor(color),
                    }
                }
                0 => Self::Reset,
                1 => Self::Bold,
                2 => Self::Faint,
                3 => Self::Italic,
                4 => Self::Underline(Underline::from(parameter.subs.first().copied().flatten())),
                5 => Self::SlowBlink,
                6 => Self::RapidBlink,
                7 => Self::Reverse,
                8 => Self::Conceal,
                9 => Self::Strike,
                10..=19 => Self::Font((value - 10) as u8),
                20 => Self::Fraktur,
                21 => Self::Underline(Underline::Double),
                22 => Self::NormalIntensity,
                23 => Self::NotItalic,
                24 => Self::NotUnderlined,
                25 => Self::NotBlinking,
                26 => Self::Proportional,
                27 => Self::NotReversed,
                28 => Self::Reveal,
                29 => Self::NotStruck,
                30..=37 => Self::Foreground(Color::from_parameter(value - 30)),
                39 => Self::Foreground(Color::Default),
                40..=47 => Self::Background(Color::from_parameter(value - 40)),
                49 => Self::Background(Color::Default),
                50 => Self::NotProportional,
                51 => Self::Framed,
                52 => Self::Encircled,
                53 => Self::Overlined,
                54 => Self::NotFramedOrEncircled,
                55 => Self::NotOverlined,
                59 => Self::UnderlineColor(Color::Default),
                60..=65 => Self::Ideogram((value - 60) as u8),
                73 => Self::Superscript,
                74 => Self::Subscript,
                75 => Self::NeitherSuperscriptNorSubscript,
                90..=97 => Self::Foreground(Color::from_parameter(value - 90).bright()),
                100..=107 => Self::Background(Color::from_parameter(value - 100).bright()),
                value => Self::Unknown(value),
            };

            renditions.push(rendition);
            position += 1;
        }

        renditions
    }
}

/// The way in which text is underlined.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Underline {
    /// Underlined with one line, which is what `4` on its own means.
    #[default]
    Single,
    /// Underlined with two lines (`4:2`, or `21`).
    Double,
    /// Underlined with a wavy line (`4:3`).
    Curly,
    /// Underlined with a dotted line (`4:4`).
    Dotted,
    /// Underlined with a dashed line (`4:5`).
    Dashed,
    /// Not underlined, which is what `4:0` means.
    None,
}

impl From<Option<u16>> for Underline {
    fn from(sub: Option<u16>) -> Self {
        match sub {
            Some(0) => Self::None,
            Some(2) => Self::Double,
            Some(3) => Self::Curly,
            Some(4) => Self::Dotted,
            Some(5) => Self::Dashed,
            _ => Self::Single,
        }
    }
}

impl Underline {
    /// Return the parameter which says to underline text this way, written as short as it goes.
    fn parameter(self) -> String {
        match self {
            Self::Single => "4".to_string(),
            // NOTE: Two lines has a parameter of its own, which is a byte shorter than the
            // sub-parameter it can also be written as.
            Self::Double => "21".to_string(),
            Self::Curly => "4:3".to_string(),
            Self::Dotted => "4:4".to_string(),
            Self::Dashed => "4:5".to_string(),
            // NOTE: This is not written as the parameter for not being underlined at all, even
            // though that is shorter, because the two do not read back the same way.
            Self::None => "4:0".to_string(),
        }
    }
}

/// A colour to write text in or on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    /// One of the colours of the palette the terminal is set up with.
    ///
    /// Picking one of these, and giving a colour as its components, are both from ITU T.416 rather
    /// than ECMA-48, which leaves `38` and `48` reserved.
    ///
    /// The first sixteen entries of the palette are the colours above, so an index into them reads
    /// as one of those rather than as this. That keeps there being one way of saying each colour,
    /// and the way it reads back out is the shorter one.
    Indexed(u8),
    /// A colour given as the amount of each of its components.
    Rgb {
        red: u8,
        green: u8,
        blue: u8,
    },
    /// Whichever colour the terminal uses when none has been picked.
    Default,
}

impl Color {
    /// Return the colour which one of the eight parameters `30` to `37` or `40` to `47` stands
    /// for, counting from the first of them.
    fn from_parameter(offset: u16) -> Self {
        match offset {
            0 => Self::Black,
            1 => Self::Red,
            2 => Self::Green,
            3 => Self::Yellow,
            4 => Self::Blue,
            5 => Self::Magenta,
            6 => Self::Cyan,
            _ => Self::White,
        }
    }

    /// Return the colour which the given entry of the palette is. The first sixteen entries are the
    /// basic colours and the bright ones, so those read as the colours they are.
    fn from_index(index: u8) -> Self {
        match index {
            0..=7 => Self::from_parameter(u16::from(index)),
            8..=15 => Self::from_parameter(u16::from(index) - 8).bright(),
            index => Self::Indexed(index),
        }
    }

    /// Return which of the eight basic colours this is and whether it is the bright one, or `None`
    /// if it is not one of them.
    fn offset(self) -> Option<(u16, bool)> {
        let offset: u16 = match self {
            Self::Black | Self::BrightBlack => 0,
            Self::Red | Self::BrightRed => 1,
            Self::Green | Self::BrightGreen => 2,
            Self::Yellow | Self::BrightYellow => 3,
            Self::Blue | Self::BrightBlue => 4,
            Self::Magenta | Self::BrightMagenta => 5,
            Self::Cyan | Self::BrightCyan => 6,
            Self::White | Self::BrightWhite => 7,
            _ => {
                return None;
            }
        };

        let bright: bool = matches!(
            self,
            Self::BrightBlack
                | Self::BrightRed
                | Self::BrightGreen
                | Self::BrightYellow
                | Self::BrightBlue
                | Self::BrightMagenta
                | Self::BrightCyan
                | Self::BrightWhite
        );

        Some((offset, bright))
    }

    /// Write the colour onto the given parameters.
    ///
    /// Which parameters a colour is written as depends on what it is being set for, so the one
    /// which the first of the eight basic colours is for that (when they can be written that
    /// short), the one which introduces the rest, and the one which means the default are given.
    fn write(&self, basic: Option<u16>, extended: u16, default: u16, parameters: &mut Vec<String>) {
        // The bright colours are sixty on from the basic ones, and eight on in the palette.
        const BRIGHT_PARAMETER_OFFSET: u16 = 60;
        const BRIGHT_INDEX_OFFSET: u16 = 8;

        let index: u16 = match (self.offset(), basic) {
            (Some((offset, bright)), Some(basic)) => {
                let bright: u16 = if bright { BRIGHT_PARAMETER_OFFSET } else { 0 };
                parameters.push((basic + offset + bright).to_string());
                return;
            }
            (Some((offset, bright)), None) => offset + if bright { BRIGHT_INDEX_OFFSET } else { 0 },
            (None, _) => match self {
                Self::Default => {
                    parameters.push(default.to_string());
                    return;
                }
                Self::Rgb { red, green, blue } => {
                    parameters.push(extended.to_string());
                    parameters.push("2".to_string());
                    parameters.push(red.to_string());
                    parameters.push(green.to_string());
                    parameters.push(blue.to_string());
                    return;
                }
                Self::Indexed(index) => u16::from(*index),
                // NOTE: Every colour which is not a palette entry is written out above.
                _ => 0,
            },
        };

        parameters.push(extended.to_string());
        parameters.push("5".to_string());
        parameters.push(index.to_string());
    }

    /// Return the bright version of the colour, or the colour itself if it does not have one.
    fn bright(self) -> Self {
        match self {
            Self::Black => Self::BrightBlack,
            Self::Red => Self::BrightRed,
            Self::Green => Self::BrightGreen,
            Self::Yellow => Self::BrightYellow,
            Self::Blue => Self::BrightBlue,
            Self::Magenta => Self::BrightMagenta,
            Self::Cyan => Self::BrightCyan,
            Self::White => Self::BrightWhite,
            color => color,
        }
    }

    /// Return the colour which the `38`, `48` or `58` parameter at the given position introduces,
    /// along with the number of parameters after that one which it takes up.
    ///
    /// The colour is written either as the sub-parameters of that parameter or as the parameters
    /// which follow it, and both are in use, so both are read.
    fn read(parameters: &[Parameter], position: usize) -> Option<(Self, usize)> {
        let subs: &[Option<u16>] = &parameters[position].subs;

        if !subs.is_empty() {
            let color: Self = match subs.first().copied().flatten() {
                Some(5) => Self::from_index(Self::component(subs.get(1).copied().flatten())),
                // NOTE: The standard puts the colour space between the `2` and the components, but
                // it is left out often enough that both lengths have to be read.
                Some(2) => {
                    let start: usize = if subs.len() > 4 { 2 } else { 1 };
                    Self::Rgb {
                        red: Self::component(subs.get(start).copied().flatten()),
                        green: Self::component(subs.get(start + 1).copied().flatten()),
                        blue: Self::component(subs.get(start + 2).copied().flatten()),
                    }
                }
                _ => {
                    return None;
                }
            };

            return Some((color, 0));
        }

        match parameters.get(position + 1).and_then(|next| next.value) {
            Some(5) => {
                let index: Self =
                    Self::from_index(Self::component(parameters.get(position + 2)?.value));
                Some((index, 2))
            }
            Some(2) => {
                let color = Self::Rgb {
                    red: Self::component(parameters.get(position + 2)?.value),
                    green: Self::component(parameters.get(position + 3)?.value),
                    blue: Self::component(parameters.get(position + 4)?.value),
                };
                Some((color, 4))
            }
            _ => None,
        }
    }

    /// Return a component of a colour, which is a byte however large the parameter it was written
    /// as happens to be.
    fn component(value: Option<u16>) -> u8 {
        value.unwrap_or(0).min(u16::from(u8::MAX)) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence::{AnsiEscapeSequence, ControlSequence, ParsedAnsiEscapeSequence};

    /// Return the renditions which the bytes of a select graphic rendition sequence say to use.
    fn parse(bytes: &[u8]) -> Vec<GraphicRendition> {
        let parsed = ParsedAnsiEscapeSequence::try_from(bytes).unwrap();
        let control: ControlSequence = match parsed.sequence {
            AnsiEscapeSequence::ControlSequence(control) => control,
            sequence => panic!("Expected a control sequence but got {:?}.", sequence),
        };

        GraphicRendition::all(&control.parameters)
    }

    #[test]
    fn test_a_sequence_with_no_parameters_is_a_reset() {
        assert_eq!(parse(b"\x1b[m"), vec![GraphicRendition::Reset]);
    }

    #[test]
    fn test_the_renditions_of_a_sequence_are_read_in_order() {
        assert_eq!(
            parse(b"\x1b[0;1;4;31m"),
            vec![
                GraphicRendition::Reset,
                GraphicRendition::Bold,
                GraphicRendition::Underline(Underline::Single),
                GraphicRendition::Foreground(Color::Red),
            ]
        );
    }

    #[test]
    fn test_the_bright_colours_are_read() {
        assert_eq!(
            parse(b"\x1b[92;104m"),
            vec![
                GraphicRendition::Foreground(Color::BrightGreen),
                GraphicRendition::Background(Color::BrightBlue),
            ]
        );
    }

    #[test]
    fn test_the_default_colours_are_read() {
        assert_eq!(
            parse(b"\x1b[39;49;59m"),
            vec![
                GraphicRendition::Foreground(Color::Default),
                GraphicRendition::Background(Color::Default),
                GraphicRendition::UnderlineColor(Color::Default),
            ]
        );
    }

    #[test]
    fn test_an_indexed_colour_written_as_parameters_is_read() {
        assert_eq!(
            parse(b"\x1b[38;5;196m"),
            vec![GraphicRendition::Foreground(Color::Indexed(196))]
        );
    }

    #[test]
    fn test_an_indexed_colour_written_as_sub_parameters_is_read() {
        assert_eq!(
            parse(b"\x1b[38:5:196m"),
            vec![GraphicRendition::Foreground(Color::Indexed(196))]
        );
    }

    #[test]
    fn test_a_colour_written_as_components_in_parameters_is_read() {
        assert_eq!(
            parse(b"\x1b[48;2;255;128;0m"),
            vec![GraphicRendition::Background(Color::Rgb {
                red: 255,
                green: 128,
                blue: 0
            })]
        );
    }

    #[test]
    fn test_a_colour_written_as_components_in_sub_parameters_is_read() {
        let rgb = Color::Rgb {
            red: 255,
            green: 128,
            blue: 0,
        };

        // With the colour space which the standard asks for, and without it.
        assert_eq!(
            parse(b"\x1b[38:2::255:128:0m"),
            vec![GraphicRendition::Foreground(rgb)]
        );
        assert_eq!(
            parse(b"\x1b[38:2:255:128:0m"),
            vec![GraphicRendition::Foreground(rgb)]
        );
    }

    #[test]
    fn test_the_renditions_after_a_colour_are_still_read() {
        assert_eq!(
            parse(b"\x1b[1;38;2;1;2;3;4m"),
            vec![
                GraphicRendition::Bold,
                GraphicRendition::Foreground(Color::Rgb {
                    red: 1,
                    green: 2,
                    blue: 3
                }),
                GraphicRendition::Underline(Underline::Single),
            ]
        );
    }

    #[test]
    fn test_the_style_of_an_underline_is_read() {
        assert_eq!(
            parse(b"\x1b[4:3m"),
            vec![GraphicRendition::Underline(Underline::Curly)]
        );
        assert_eq!(
            parse(b"\x1b[4:0m"),
            vec![GraphicRendition::Underline(Underline::None)]
        );
        assert_eq!(
            parse(b"\x1b[21m"),
            vec![GraphicRendition::Underline(Underline::Double)]
        );
    }

    #[test]
    fn test_a_rendition_which_is_not_known_is_kept() {
        assert_eq!(parse(b"\x1b[1;200m")[1], GraphicRendition::Unknown(200));
    }
}

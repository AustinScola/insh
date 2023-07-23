/*!
A parser for text with ANSI escape codes.
*/
use nom::branch::alt;
use nom::bytes::streaming::{tag, take};
use nom::combinator::value;
use nom::IResult as ParseResult;

use nom::combinator::map;

#[derive(Debug, PartialEq, Eq)]
pub enum ANSIEscapedText {
    ANSIEscapeCode(ANSIEscapeCode),
    Character(u8),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ANSIEscapeCode {
    EnableAlternativeScreen,
    DisableAlternativeScreen,
}

pub fn parser(input: &[u8]) -> ParseResult<&[u8], ANSIEscapedText> {
    alt((
        map(ansi_escape_code, |ansi_escape_code: ANSIEscapeCode| {
            ANSIEscapedText::ANSIEscapeCode(ansi_escape_code)
        }),
        map(take(1usize), |bytes: &[u8]| {
            ANSIEscapedText::Character(bytes[0])
        }),
    ))(input)
}

fn ansi_escape_code(input: &[u8]) -> ParseResult<&[u8], ANSIEscapeCode> {
    let (input, _) = control_sequence_introducer(input)?;

    // TODO: Eventually use `alt` to parse other escape codes too.
    alternative_screen(input)
}

fn alternative_screen(input: &[u8]) -> ParseResult<&[u8], ANSIEscapeCode> {
    let (input, _) = tag(&[0x3F, 0x31, 0x30, 0x34, 0x39])(input)?; // ? 1049

    alt((
        value(ANSIEscapeCode::EnableAlternativeScreen, tag(&[0x68])), // h
        value(ANSIEscapeCode::DisableAlternativeScreen, tag(&[0x6C])), // l
    ))(input)
}

fn control_sequence_introducer(input: &[u8]) -> ParseResult<&[u8], &[u8]> {
    tag(&[0x1B, 0x5B])(input) // `<Esc> [`
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case(&[0x61], Ok((&[], ANSIEscapedText::Character(0x61))); "the single letter a")]
    #[test_case(&[0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x39, 0x68], Ok((&[], ANSIEscapedText::ANSIEscapeCode(ANSIEscapeCode::EnableAlternativeScreen))); "enable alternative screen")]
    #[test_case(&[0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x39, 0x6C], Ok((&[], ANSIEscapedText::ANSIEscapeCode(ANSIEscapeCode::DisableAlternativeScreen))); "disable alternative screen")]
    fn test_ansi_escaped_text(input: &[u8], expected: ParseResult<&[u8], ANSIEscapedText>) {
        let result = parser(input);

        assert_eq!(result, expected);
    }

    #[test_case(&[0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x39, 0x68], Ok((&[], ANSIEscapeCode::EnableAlternativeScreen)); "enable alternative screen")]
    #[test_case(&[0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x39, 0x6C], Ok((&[], ANSIEscapeCode::DisableAlternativeScreen)); "disable alternative screen")]
    fn test_ansi_escape_code(input: &[u8], expected: ParseResult<&[u8], ANSIEscapeCode>) {
        let result = ansi_escape_code(input);

        assert_eq!(result, expected);
    }

    #[test_case(&[0x1B, 0x5B], Ok((&[], &[0x1B, 0x5B])); "the control sequence introducer")]
    fn test_control_sequence_introducer(input: &[u8], expected: ParseResult<&[u8], &[u8]>) {
        let result = control_sequence_introducer(input);

        assert_eq!(result, expected);
    }
}

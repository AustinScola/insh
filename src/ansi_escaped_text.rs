/// A parser for text with ANSI escape codes.
use combine;
use combine::{choice, Parser, ParseError, token};
use combine::parser::byte::bytes;
use combine::stream::Stream;

#[derive(Debug, PartialEq, Eq)]
pub enum ANSIEscapedText {
    ANSIEscapeCode(ANSIEscapeCode),
    Character(u8),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ANSIEscapeCode {
    EnableAlternativeScreen,
    DisableAlternativeScreen,
}

pub fn parser<'a, Input>() -> impl Parser<Input, Output = ANSIEscapedText> + 'a
    where Input: Stream<Token = u8, Range = &'a [u8]> + 'a,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    choice!(
        ansi_escape_code().map(|ansi_escape_code: ANSIEscapeCode| ANSIEscapedText::ANSIEscapeCode(ansi_escape_code)),
        combine::any().map(|byte: u8| ANSIEscapedText::Character(byte))
    )
}

fn ansi_escape_code<'a, Input>() -> impl Parser<Input, Output = ANSIEscapeCode> + 'a
    where Input: Stream<Token = u8, Range = &'a [u8]> + 'a,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        control_sequence_introducer(),
        choice!(
            (
                bytes(&[0x3F, 0x31, 0x30, 0x34, 0x39]), // ? 1049
                choice!(
                    token(0x68) // h
                        .map(|_| ANSIEscapeCode::EnableAlternativeScreen),
                    token(0x6C) // l
                        .map(|_| ANSIEscapeCode::DisableAlternativeScreen)
                )
            ).map(|(_, code)| code)
        )
    ).map(|(_csi, code)| code)
}

fn control_sequence_introducer<'a, Input>() -> impl Parser<Input, Output = &'a [u8]> + 'a
    where Input: Stream<Token = u8, Range = &'a [u8]> + 'a,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    bytes(&[0x1B, 0x5B])  // `<Esc> [`
}

#[cfg(test)]
mod tests {
    use super::*;

    use combine::StreamOnce;

    use test_case::test_case;

    #[test_case(&[0x61], Ok((ANSIEscapedText::Character(0x61), &[])); "the single letter a")]
    #[test_case(&[0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x39, 0x68], Ok((ANSIEscapedText::ANSIEscapeCode(ANSIEscapeCode::EnableAlternativeScreen), &[])); "enable alternative screen")]
    #[test_case(&[0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x39, 0x6C], Ok((ANSIEscapedText::ANSIEscapeCode(ANSIEscapeCode::DisableAlternativeScreen), &[])); "disable alternative screen")]
    fn test_ansi_escaped_text(input: &[u8], expected: Result<(ANSIEscapedText, &[u8]), <&[u8] as StreamOnce>::Error>) {
        let result = parser().parse(input);

        assert_eq!(result, expected);
    }

    #[test_case(&[0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x39, 0x68], Ok((ANSIEscapeCode::EnableAlternativeScreen, &[])); "enable alternative screen")]
    #[test_case(&[0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x39, 0x6C], Ok((ANSIEscapeCode::DisableAlternativeScreen, &[])); "disable alternative screen")]
    fn test_ansi_escape_code(input: &[u8], expected: Result<(ANSIEscapeCode, &[u8]), <&[u8] as StreamOnce>::Error>) {
        let result = ansi_escape_code().parse(input);

        assert_eq!(result, expected);
    }

    #[test_case(&[0x1B, 0x5B], Ok((&[0x1B, 0x5B], &[])); "the control sequence introducer")]
    fn test_control_sequence_introducer(input: &[u8], expected: Result<(&[u8], &[u8]), <&[u8] as StreamOnce>::Error>) {
        let result = control_sequence_introducer().parse(input);

        assert_eq!(result, expected);
    }
}

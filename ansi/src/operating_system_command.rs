/*!
What the operating system commands, which are the sequences carrying a string rather than numbers,
tell the terminal to do.
*/

use std::str;

/// What an operating system command asks the terminal for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatingSystemCommand {
    /// Set both the name shown when the window is minimized and its title (`0`).
    SetIconNameAndWindowTitle(String),
    /// Set the name shown when the window is minimized (`1`).
    SetIconName(String),
    /// Set the title of the window (`2`).
    SetWindowTitle(String),
    /// Set one of the colors of the palette, or ask what it is when the value is a `?` (`4`).
    SetPaletteColor {
        /// Which of the two hundred and fifty six colors of the palette it is.
        index: u8,
        /// The color, written the way X11 writes one, such as `rgb:ff/00/00`.
        color: String,
    },
    /// Turn the text which follows into a link, or end one when the link is empty (`8`).
    Hyperlink {
        /// The parameters of the link, such as an `id` to join broken up runs of it back together.
        parameters: String,
        /// What the link points at.
        uri: String,
    },
    /// Set the color text is written in by default (`10`).
    SetForegroundColor(String),
    /// Set the color text is written on by default (`11`).
    SetBackgroundColor(String),
    /// Set the color of the cursor (`12`).
    SetCursorColor(String),
    /// Set the directory the shell is in, so that a new window can be opened in the same one (`7`).
    SetWorkingDirectory(String),
    /// Read or write one of the selections, which is how a program running over ssh reaches the
    /// clipboard of the machine the terminal is on (`52`).
    Clipboard {
        /// Which selections it is for, as a letter each: `c` for the clipboard, `p` for the
        /// primary selection, and `s` for whichever of them is configured.
        selections: String,
        /// The contents encoded as base64, or a `?` to ask what they are.
        data: String,
    },
    /// Put one of the colors of the palette back to what it was (`104`).
    ResetPaletteColor(Option<u8>),
    /// Put the color text is written in by default back to what it was (`110`).
    ResetForegroundColor,
    /// Put the color text is written on by default back to what it was (`111`).
    ResetBackgroundColor,
    /// Put the color of the cursor back to what it was (`112`).
    ResetCursorColor,
    /// A command which is not one of the ones above.
    Unknown {
        /// The number at the start of the command, if it has one.
        number: Option<u16>,
        /// Everything after the number.
        payload: Vec<u8>,
    },
}

impl From<&[u8]> for OperatingSystemCommand {
    fn from(payload: &[u8]) -> Self {
        // The command starts with a number saying which one it is, then the rest of it after a
        // semicolon. Some of them have no rest at all.
        let (number, rest): (&[u8], &[u8]) = match payload.iter().position(|byte| *byte == b';') {
            Some(position) => (&payload[..position], &payload[position + 1..]),
            None => (payload, &[]),
        };

        let number: Option<u16> = str::from_utf8(number)
            .ok()
            .and_then(|number| number.parse().ok());

        // NOTE: What a command carries is a title or a path or something else which came from the
        // user, so the bytes which are not valid UTF-8 are replaced rather than the whole command
        // being thrown away.
        let rest: String = String::from_utf8_lossy(rest).into_owned();

        match number {
            Some(0) => Self::SetIconNameAndWindowTitle(rest),
            Some(1) => Self::SetIconName(rest),
            Self::WINDOW_TITLE => Self::SetWindowTitle(rest),
            Some(4) => match Self::split(&rest) {
                Some((index, color)) => match index.parse() {
                    Ok(index) => Self::SetPaletteColor { index, color },
                    Err(_) => Self::unknown(number, payload),
                },
                None => Self::unknown(number, payload),
            },
            Some(7) => Self::SetWorkingDirectory(rest),
            Some(8) => match Self::split(&rest) {
                Some((parameters, uri)) => Self::Hyperlink {
                    parameters: parameters.to_string(),
                    uri,
                },
                None => Self::unknown(number, payload),
            },
            Some(10) => Self::SetForegroundColor(rest),
            Some(11) => Self::SetBackgroundColor(rest),
            Some(12) => Self::SetCursorColor(rest),
            Some(52) => match Self::split(&rest) {
                Some((selections, data)) => Self::Clipboard {
                    selections: selections.to_string(),
                    data,
                },
                None => Self::unknown(number, payload),
            },
            Some(104) => Self::ResetPaletteColor(rest.parse().ok()),
            Some(110) => Self::ResetForegroundColor,
            Some(111) => Self::ResetBackgroundColor,
            Some(112) => Self::ResetCursorColor,
            number => Self::unknown(number, payload),
        }
    }
}

impl From<&OperatingSystemCommand> for Vec<u8> {
    fn from(command: &OperatingSystemCommand) -> Self {
        match command {
            OperatingSystemCommand::SetIconNameAndWindowTitle(title) => {
                format!("0;{}", title).into_bytes()
            }
            OperatingSystemCommand::SetIconName(name) => format!("1;{}", name).into_bytes(),
            OperatingSystemCommand::SetWindowTitle(title) => format!("2;{}", title).into_bytes(),
            OperatingSystemCommand::SetPaletteColor { index, color } => {
                format!("4;{};{}", index, color).into_bytes()
            }
            OperatingSystemCommand::SetWorkingDirectory(directory) => {
                format!("7;{}", directory).into_bytes()
            }
            OperatingSystemCommand::Hyperlink { parameters, uri } => {
                format!("8;{};{}", parameters, uri).into_bytes()
            }
            OperatingSystemCommand::SetForegroundColor(color) => {
                format!("10;{}", color).into_bytes()
            }
            OperatingSystemCommand::SetBackgroundColor(color) => {
                format!("11;{}", color).into_bytes()
            }
            OperatingSystemCommand::SetCursorColor(color) => format!("12;{}", color).into_bytes(),
            OperatingSystemCommand::Clipboard { selections, data } => {
                format!("52;{};{}", selections, data).into_bytes()
            }
            OperatingSystemCommand::ResetPaletteColor(Some(index)) => {
                format!("104;{}", index).into_bytes()
            }
            OperatingSystemCommand::ResetPaletteColor(None) => b"104".to_vec(),
            OperatingSystemCommand::ResetForegroundColor => b"110".to_vec(),
            OperatingSystemCommand::ResetBackgroundColor => b"111".to_vec(),
            OperatingSystemCommand::ResetCursorColor => b"112".to_vec(),
            // NOTE: The whole payload is kept for a command which is not known, number and all, so
            // that it goes back out exactly as it came in.
            OperatingSystemCommand::Unknown { payload, .. } => payload.clone(),
        }
    }
}

impl OperatingSystemCommand {
    /// The number of the command which sets the title of the window.
    const WINDOW_TITLE: Option<u16> = Some(2);

    /// Return the command as one which is not known.
    fn unknown(number: Option<u16>, payload: &[u8]) -> Self {
        Self::Unknown {
            number,
            payload: payload.to_vec(),
        }
    }

    /// Split what follows the number of a command at the first semicolon, which is where the
    /// commands taking two things apart from the number put the one between them.
    fn split(rest: &str) -> Option<(&str, String)> {
        rest.split_once(';')
            .map(|(before, after)| (before, after.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case(b"0;a title", OperatingSystemCommand::SetIconNameAndWindowTitle("a title".to_string()); "setting the icon name and window title")]
    #[test_case(b"1;a name", OperatingSystemCommand::SetIconName("a name".to_string()); "setting the icon name")]
    #[test_case(b"2;a title", OperatingSystemCommand::SetWindowTitle("a title".to_string()); "setting the window title")]
    #[test_case(b"7;file:///home", OperatingSystemCommand::SetWorkingDirectory("file:///home".to_string()); "setting the working directory")]
    #[test_case(b"10;rgb:ff/ff/ff", OperatingSystemCommand::SetForegroundColor("rgb:ff/ff/ff".to_string()); "setting the foreground color")]
    #[test_case(b"11;rgb:00/00/00", OperatingSystemCommand::SetBackgroundColor("rgb:00/00/00".to_string()); "setting the background color")]
    #[test_case(b"12;red", OperatingSystemCommand::SetCursorColor("red".to_string()); "setting the cursor color")]
    #[test_case(b"110", OperatingSystemCommand::ResetForegroundColor; "resetting the foreground color")]
    #[test_case(b"111", OperatingSystemCommand::ResetBackgroundColor; "resetting the background color")]
    #[test_case(b"112", OperatingSystemCommand::ResetCursorColor; "resetting the cursor color")]
    #[test_case(b"104;7", OperatingSystemCommand::ResetPaletteColor(Some(7)); "resetting one palette color")]
    #[test_case(b"104", OperatingSystemCommand::ResetPaletteColor(None); "resetting every palette color")]
    fn test_a_command_is_read(payload: &[u8], command: OperatingSystemCommand) {
        assert_eq!(OperatingSystemCommand::from(payload), command);
        assert_eq!(Vec::<u8>::from(&command), payload);
    }

    #[test]
    fn test_setting_a_palette_color_is_read() {
        assert_eq!(
            OperatingSystemCommand::from(&b"4;196;rgb:ff/00/00"[..]),
            OperatingSystemCommand::SetPaletteColor {
                index: 196,
                color: "rgb:ff/00/00".to_string(),
            }
        );
    }

    #[test]
    fn test_a_hyperlink_is_read() {
        assert_eq!(
            OperatingSystemCommand::from(&b"8;id=1;https://example.com"[..]),
            OperatingSystemCommand::Hyperlink {
                parameters: "id=1".to_string(),
                uri: "https://example.com".to_string(),
            }
        );
    }

    #[test]
    fn test_the_end_of_a_hyperlink_is_read() {
        assert_eq!(
            OperatingSystemCommand::from(&b"8;;"[..]),
            OperatingSystemCommand::Hyperlink {
                parameters: String::new(),
                uri: String::new(),
            }
        );
    }

    #[test]
    fn test_writing_the_clipboard_is_read() {
        assert_eq!(
            OperatingSystemCommand::from(&b"52;c;aGVsbG8="[..]),
            OperatingSystemCommand::Clipboard {
                selections: "c".to_string(),
                data: "aGVsbG8=".to_string(),
            }
        );
    }

    #[test]
    fn test_a_command_which_is_not_known_is_kept() {
        assert_eq!(
            OperatingSystemCommand::from(&b"777;notify;hello"[..]),
            OperatingSystemCommand::Unknown {
                number: Some(777),
                payload: b"777;notify;hello".to_vec(),
            }
        );
    }
}

/*!
This module contains the [`Modes`] flags which are the ways a screen has been asked to behave.
*/
use ansi::Mode;

use bitflags::bitflags;

bitflags! {
    /// The ways a screen has been asked to behave.
    ///
    /// Only the ones which change what a screen does or what is sent to a program are here. A mode
    /// which a screen makes nothing of is not kept, since putting it back would mean nothing
    /// either.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Modes: u16 {
        /// A cluster written past the last column goes onto the row below rather than being
        /// dropped.
        const AUTO_WRAP = 1 << 0;
        /// Rows are counted from the top of the region the text scrolls between rather than from
        /// the top of the screen.
        const ORIGIN = 1 << 1;
        /// The cursor is shown.
        const CURSOR_VISIBLE = 1 << 2;
        /// What is written makes room for itself rather than writing over what is already there.
        const INSERT = 1 << 3;
        /// A line feed moves the cursor to the first column as well as down a row.
        const NEW_LINE = 1 << 4;
        /// The whole screen is written the other way around.
        const REVERSE_VIDEO = 1 << 5;
        /// The arrow keys are sent as the application forms of them.
        const APPLICATION_CURSOR_KEYS = 1 << 6;
        /// The keypad keys are sent as the application forms of them.
        const APPLICATION_KEYPAD = 1 << 7;
        /// Pasted text is wrapped in the markers which say where it starts and ends.
        const BRACKETED_PASTE = 1 << 8;
        /// Pressing a mouse button is reported.
        const REPORT_MOUSE_CLICKS = 1 << 9;
        /// Moving the mouse with a button held is reported.
        const REPORT_MOUSE_DRAGS = 1 << 10;
        /// Moving the mouse at all is reported.
        const REPORT_MOUSE_MOTION = 1 << 11;
        /// Mouse reports are sent as control sequences rather than as single bytes.
        const CONTROL_SEQUENCE_MOUSE_REPORTS = 1 << 12;
        /// The terminal being focused and unfocused is reported.
        const REPORT_FOCUS = 1 << 13;
    }
}

impl Default for Modes {
    fn default() -> Self {
        // NOTE: A screen wraps and shows its cursor until it is asked not to, which is what a
        // terminal does when it is turned on.
        return Self::AUTO_WRAP | Self::CURSOR_VISIBLE;
    }
}

impl Modes {
    /// Return which of these the mode is, which is none of them when it is one a screen makes
    /// nothing of.
    pub fn of(mode: &Mode) -> Option<Self> {
        return match mode {
            Mode::AutoWrap => Some(Self::AUTO_WRAP),
            Mode::Origin => Some(Self::ORIGIN),
            Mode::CursorVisible => Some(Self::CURSOR_VISIBLE),
            Mode::Insert => Some(Self::INSERT),
            Mode::LineFeedNewLine => Some(Self::NEW_LINE),
            Mode::ReverseVideo => Some(Self::REVERSE_VIDEO),
            Mode::ApplicationCursorKeys => Some(Self::APPLICATION_CURSOR_KEYS),
            Mode::BracketedPaste => Some(Self::BRACKETED_PASTE),
            Mode::ReportMouseClicks => Some(Self::REPORT_MOUSE_CLICKS),
            Mode::ReportMouseDrags => Some(Self::REPORT_MOUSE_DRAGS),
            Mode::ReportMouseMotion => Some(Self::REPORT_MOUSE_MOTION),
            Mode::ControlSequenceMouseReports => Some(Self::CONTROL_SEQUENCE_MOUSE_REPORTS),
            Mode::ReportFocus => Some(Self::REPORT_FOCUS),
            _ => None,
        };
    }
}

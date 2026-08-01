/*!
Renders information about files the way that people are used to reading it (which is the same way
that `ls -l` renders it).
*/
#![allow(clippy::needless_return)]

/// Contains functionality for rendering the mode of a file.
mod mode {
    /// The bits of a mode which indicate the type of the file.
    const TYPE_MASK: u32 = 0o170000;
    /// The type bits of a socket.
    const SOCKET: u32 = 0o140000;
    /// The type bits of a symlink.
    const SYMLINK: u32 = 0o120000;
    /// The type bits of a regular file.
    const REGULAR: u32 = 0o100000;
    /// The type bits of a block device.
    const BLOCK_DEVICE: u32 = 0o060000;
    /// The type bits of a directory.
    const DIRECTORY: u32 = 0o040000;
    /// The type bits of a character device.
    const CHARACTER_DEVICE: u32 = 0o020000;
    /// The type bits of a fifo.
    const FIFO: u32 = 0o010000;

    /// The bit which indicates that a file is set-user-id.
    const SET_USER_ID: u32 = 0o4000;
    /// The bit which indicates that a file is set-group-id.
    const SET_GROUP_ID: u32 = 0o2000;
    /// The bit which indicates that a file is sticky.
    const STICKY: u32 = 0o1000;

    /// The number of characters used to render the mode of a file.
    pub const FILE_MODE_WIDTH: usize = 10;

    /// Return the mode of a file rendered the same way that `ls -l` renders it.
    ///
    /// The mode includes the bits which indicate the type of the file.
    pub fn human_friendly_file_mode(mode: u32) -> String {
        let mut string = String::with_capacity(FILE_MODE_WIDTH);

        string.push(type_character(mode));

        string.push(character(mode, 0o400, 'r'));
        string.push(character(mode, 0o200, 'w'));
        string.push(execute_character(mode, 0o100, mode & SET_USER_ID != 0, 's'));

        string.push(character(mode, 0o040, 'r'));
        string.push(character(mode, 0o020, 'w'));
        string.push(execute_character(
            mode,
            0o010,
            mode & SET_GROUP_ID != 0,
            's',
        ));

        string.push(character(mode, 0o004, 'r'));
        string.push(character(mode, 0o002, 'w'));
        string.push(execute_character(mode, 0o001, mode & STICKY != 0, 't'));

        return string;
    }

    /// Return the character used to indicate the type of the file.
    fn type_character(mode: u32) -> char {
        match mode & TYPE_MASK {
            SOCKET => 's',
            SYMLINK => 'l',
            REGULAR => '-',
            BLOCK_DEVICE => 'b',
            DIRECTORY => 'd',
            CHARACTER_DEVICE => 'c',
            FIFO => 'p',
            _ => '?',
        }
    }

    /// Return the `character` if the `bit` of the mode is set, else a dash.
    fn character(mode: u32, bit: u32, character: char) -> char {
        match mode & bit != 0 {
            true => character,
            false => '-',
        }
    }

    /// Return the character used for an execute bit of the mode.
    ///
    /// If the corresponding special bit (set-user-id, set-group-id, or sticky) is set, then the
    /// `special_character` is used (capitalized if the execute bit is not set).
    fn execute_character(mode: u32, bit: u32, special: bool, special_character: char) -> char {
        let executable: bool = mode & bit != 0;
        match (executable, special) {
            (true, true) => special_character,
            (false, true) => special_character.to_ascii_uppercase(),
            (true, false) => 'x',
            (false, false) => '-',
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use test_case::test_case;

        #[test_case(0o100644, "-rw-r--r--"; "a regular file")]
        #[test_case(0o040755, "drwxr-xr-x"; "a directory")]
        #[test_case(0o120777, "lrwxrwxrwx"; "a symlink")]
        #[test_case(0o010664, "prw-rw-r--"; "a fifo")]
        #[test_case(0o140755, "srwxr-xr-x"; "a socket")]
        #[test_case(0o060660, "brw-rw----"; "a block device")]
        #[test_case(0o020666, "crw-rw-rw-"; "a character device")]
        #[test_case(0o000644, "?rw-r--r--"; "an unknown type")]
        #[test_case(0o104755, "-rwsr-xr-x"; "a set-user-id file which is executable")]
        #[test_case(0o104644, "-rwSr--r--"; "a set-user-id file which is not executable")]
        #[test_case(0o102755, "-rwxr-sr-x"; "a set-group-id file which is executable")]
        #[test_case(0o102644, "-rw-r-Sr--"; "a set-group-id file which is not executable")]
        #[test_case(0o041777, "drwxrwxrwt"; "a sticky directory which is executable")]
        #[test_case(0o041666, "drw-rw-rwT"; "a sticky directory which is not executable")]
        #[test_case(0o047777, "drwsrwsrwt"; "a directory with all of the special bits")]
        #[test_case(0o100000, "----------"; "a file with no permissions")]
        fn test_human_friendly_file_mode(mode: u32, expected_string: &str) {
            let string: String = human_friendly_file_mode(mode);

            assert_eq!(string, expected_string);
        }
    }
}
pub use mode::{human_friendly_file_mode, FILE_MODE_WIDTH};

/// Contains functionality for rendering the time that a file was modified.
mod time {
    use chrono::{DateTime, Local, LocalResult, TimeZone};

    /// The number of characters used to render the time that a file was modified.
    pub const FILE_TIME_WIDTH: usize = 12;

    /// The number of seconds in six months. (This is the same approximation that `ls` uses.)
    const SIX_MONTHS: i64 = 15_778_476;

    /// The number of seconds in an hour.
    const AN_HOUR: i64 = 3_600;

    /// The format used for times which are neither too old nor too far in the future.
    const RECENT_FORMAT: &str = "%b %e %H:%M";

    /// The format used for times which are too old or too far in the future.
    const DISTANT_FORMAT: &str = "%b %e  %Y";

    /// Return the time that a file was modified rendered the same way that `ls -l` renders it.
    ///
    /// Both the `modified` time and the `now` are the number of seconds since the Unix epoch.
    ///
    /// Like `ls`, times which are more than six months old or more than an hour in the future are
    /// shown with the year instead of the time of day. The time is always rendered using
    /// [`FILE_TIME_WIDTH`] characters so that the times of multiple files line up.
    pub fn human_friendly_file_time(modified: i64, now: i64) -> String {
        let date_time: DateTime<Local> = match Local.timestamp_opt(modified, 0) {
            LocalResult::Single(date_time) => date_time,
            LocalResult::Ambiguous(date_time, _) => date_time,
            LocalResult::None => {
                return "?".repeat(FILE_TIME_WIDTH);
            }
        };

        let recent: bool = modified > now - SIX_MONTHS && modified < now + AN_HOUR;
        let format: &str = match recent {
            true => RECENT_FORMAT,
            false => DISTANT_FORMAT,
        };

        let string: String = date_time.format(format).to_string();

        return match string.chars().count() > FILE_TIME_WIDTH {
            true => string.chars().take(FILE_TIME_WIDTH).collect(),
            false => format!("{:>width$}", string, width = FILE_TIME_WIDTH),
        };
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use test_case::test_case;

        /// Return the number of seconds since the Unix epoch of a local date and time.
        ///
        /// The times are local so that the tests do not depend on the time zone that they are run
        /// in.
        fn timestamp(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
            Local
                .with_ymd_and_hms(year, month, day, hour, minute, 0)
                .unwrap()
                .timestamp()
        }

        /// Return the number of seconds since the Unix epoch used as the current time by the
        /// tests.
        fn now() -> i64 {
            timestamp(2024, 6, 15, 12, 30)
        }

        #[test_case(timestamp(2024, 6, 15, 12, 30), "Jun 15 12:30"; "a time right now")]
        #[test_case(timestamp(2024, 6, 1, 9, 4), "Jun  1 09:04"; "a recent time")]
        #[test_case(timestamp(2024, 1, 1, 0, 0), "Jan  1 00:00"; "a recent time at the start of a year")]
        #[test_case(timestamp(2019, 3, 5, 8, 4), "Mar  5  2019"; "a distant time")]
        #[test_case(timestamp(2030, 12, 25, 23, 59), "Dec 25  2030"; "a time in the future")]
        fn test_human_friendly_file_time(modified: i64, expected_string: &str) {
            let string: String = human_friendly_file_time(modified, now());

            assert_eq!(string, expected_string);
            assert_eq!(string.chars().count(), FILE_TIME_WIDTH);
        }

        #[test_case(0, 0, true; "a time right now")]
        #[test_case(SIX_MONTHS - 1, 0, true; "a time just less than six months old")]
        #[test_case(SIX_MONTHS, 0, false; "a time exactly six months old")]
        #[test_case(SIX_MONTHS + 1, 0, false; "a time more than six months old")]
        #[test_case(0, AN_HOUR - 1, true; "a time just less than an hour in the future")]
        #[test_case(0, AN_HOUR, false; "a time exactly an hour in the future")]
        #[test_case(0, AN_HOUR + 1, false; "a time more than an hour in the future")]
        fn test_the_time_of_day_is_only_shown_for_recent_times(ago: i64, ahead: i64, recent: bool) {
            let modified: i64 = now() - ago + ahead;

            let string: String = human_friendly_file_time(modified, now());

            assert_eq!(
                string.contains(':'),
                recent,
                "unexpected format for {:?}",
                string
            );
            assert_eq!(string.chars().count(), FILE_TIME_WIDTH);
        }
    }
}
pub use time::{human_friendly_file_time, FILE_TIME_WIDTH};

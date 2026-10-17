/*!
This module contains the [`CharacterSet`] enum which is what the characters a program writes stand
for.
*/

/// What the characters a program writes stand for.
///
/// A program draws a box by asking for the line drawing set and then writing the letters which the
/// lines stand for in it, which is how one drew a box before there were characters for the lines
/// themselves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CharacterSet {
    /// The characters stand for themselves (`B`).
    #[default]
    Ascii,
    /// The characters between `_` and `~` stand for lines and the like (`0`).
    ///
    /// NOTE: This is the DEC special graphics set, which is in no standard and comes from the
    /// VT100.
    LineDrawing,
}

impl CharacterSet {
    /// The first character which stands for something else in the line drawing set.
    const FIRST: char = '_';

    /// What each character from the first one on stands for in the line drawing set.
    ///
    /// The one which `_` stands for is a blank, and the ones past the last of these stand for
    /// themselves.
    const LINES: [char; 32] = [
        ' ', '◆', '▒', '␉', '␌', '␍', '␊', '°', '±', '␤', '␋', '┘', '┐', '┌', '└', '┼', '⎺', '⎻',
        '─', '⎼', '⎽', '├', '┤', '┴', '┬', '│', '≤', '≥', 'π', '≠', '£', '·',
    ];

    /// Return which character set the byte a program designated one with says it is.
    pub fn of(character_set: u8) -> Option<Self> {
        return match character_set {
            b'0' => Some(Self::LineDrawing),
            b'B' => Some(Self::Ascii),
            _ => None,
        };
    }

    /// Return what the character stands for in this set.
    pub fn character(&self, character: char) -> char {
        if *self == Self::Ascii {
            return character;
        }

        let offset: usize = match usize::try_from(u32::from(character))
            .ok()
            .and_then(|character| character.checked_sub(Self::FIRST as usize))
        {
            Some(offset) => offset,
            None => return character,
        };

        return Self::LINES.get(offset).copied().unwrap_or(character);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case('q', '─'; "a horizontal line")]
    #[test_case('x', '│'; "a vertical line")]
    #[test_case('l', '┌'; "a corner")]
    #[test_case('n', '┼'; "a crossing")]
    #[test_case('_', ' '; "the first one, which is a blank")]
    #[test_case('~', '·'; "the last one")]
    #[test_case('a', '▒'; "a shade")]
    #[test_case('A', 'A'; "a character below the ones which stand for lines")]
    #[test_case('🦀', '🦀'; "a character past the ones which stand for lines")]
    fn test_what_a_character_stands_for_when_lines_are_being_drawn(
        character: char,
        expected: char,
    ) {
        assert_eq!(CharacterSet::LineDrawing.character(character), expected);
    }

    #[test]
    fn test_a_character_stands_for_itself_in_the_ascii_set() {
        for character in ['q', 'x', '_', 'A'] {
            assert_eq!(CharacterSet::Ascii.character(character), character);
        }
    }

    #[test_case(b'0', Some(CharacterSet::LineDrawing); "the line drawing set")]
    #[test_case(b'B', Some(CharacterSet::Ascii); "the ascii set")]
    #[test_case(b'%', None; "one which is not known")]
    fn test_which_character_set_a_byte_says(byte: u8, expected: Option<CharacterSet>) {
        assert_eq!(CharacterSet::of(byte), expected);
    }
}

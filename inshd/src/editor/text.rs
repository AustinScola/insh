//! The lines of a file being edited, and the changes made to them.

use insh_api::{EditorMode, Position};

use typed_builder::TypedBuilder;

/// The lines of a file being edited.
///
/// Every position in it is counted in characters rather than in bytes, so that a cursor can never
/// land in the middle of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    /// The lines, without the newlines between them.
    ///
    /// NOTE: There is always at least one, since even an empty file has a line for the cursor to
    /// be on.
    lines: Vec<String>,
}

impl From<&str> for Text {
    fn from(string: &str) -> Self {
        return Self {
            lines: string.split('\n').map(String::from).collect(),
        };
    }
}

impl Text {
    /// Return how many lines there are.
    pub fn len(&self) -> usize {
        return self.lines.len();
    }

    /// Return the lines.
    pub fn lines(&self) -> &[String] {
        return &self.lines;
    }

    /// Return how many characters the line has, which is none for a line there is not.
    pub fn columns(&self, line: usize) -> usize {
        return match self.lines.get(line) {
            Some(line) => line.chars().count(),
            None => 0,
        };
    }

    /// Return the position after the last character.
    pub fn end(&self) -> Position {
        let line: usize = self.lines.len() - 1;
        return Position::new(line, self.columns(line));
    }

    /// Return the position moved back inside the text.
    pub fn clamp(&self, at: Position) -> Position {
        if at.line >= self.lines.len() {
            return self.end();
        }
        return Position::new(at.line, at.column.min(self.columns(at.line)));
    }

    /// Return the last column a cursor in the mode can be on in the line.
    ///
    /// A cursor which is typing text in can be after the last character, since that is where
    /// typing adds to the end of the line, but otherwise it is always on a character.
    pub fn last_column(&self, line: usize, mode: EditorMode) -> usize {
        let columns: usize = self.columns(line);
        return match mode {
            EditorMode::Insert => columns,
            _ => columns.saturating_sub(1),
        };
    }

    /// Return the position moved back to where a cursor in the mode can be.
    pub fn within(&self, at: Position, mode: EditorMode) -> Position {
        let at: Position = self.clamp(at);
        return Position::new(at.line, at.column.min(self.last_column(at.line, mode)));
    }

    /// Return the position after the character at the position, which is the start of the next
    /// line when it is at the end of one.
    pub fn next(&self, at: Position) -> Position {
        let at: Position = self.clamp(at);
        return self.forward(at).unwrap_or(at);
    }

    /// Return where moving to the start of the next word the number of times goes, the way vim's
    /// `w` does, along with its `W` for a big word.
    ///
    /// For an operator the last move stops at the end of the line rather than going on to the
    /// next, which can leave it just past the last character.
    pub fn next_word_start(
        &self,
        at: Position,
        times: usize,
        big: bool,
        stop_at_end: bool,
    ) -> Position {
        let mut at: Position = at;
        for time in (0..times).rev() {
            let stop_at_end: bool = stop_at_end && time == 0;
            let on_last_line: bool = at.line + 1 == self.lines.len();
            let stops = |from: Position, to: Position| -> bool {
                self.stepped_off(from, to) && (on_last_line || stop_at_end)
            };

            let class: Class = self.class(at, big);
            let mut previous: Position = at;
            at = match self.forward(at) {
                Some(next) => next,
                None => return at,
            };
            if stops(previous, at) {
                return at;
            }

            if class != Class::Blank {
                while self.class(at, big) == class {
                    previous = at;
                    at = match self.forward(at) {
                        Some(next) => next,
                        None => return at,
                    };
                    if stop_at_end && self.stepped_off(previous, at) {
                        return at;
                    }
                }
            }

            // NOTE: An empty line counts as a word.
            while self.class(at, big) == Class::Blank && self.columns(at.line) > 0 {
                previous = at;
                at = match self.forward(at) {
                    Some(next) => next,
                    None => return at,
                };
                if stops(previous, at) {
                    return at;
                }
            }
        }
        return at;
    }

    /// Return where moving to the start of the previous word the number of times goes, the way
    /// vim's `b` does, along with its `B` for a big word.
    pub fn previous_word_start(&self, at: Position, times: usize, big: bool) -> Position {
        let mut at: Position = at;
        'times: for _ in 0..times {
            at = match self.backward(at) {
                Some(previous) => previous,
                None => return at,
            };

            while self.class(at, big) == Class::Blank {
                // NOTE: An empty line counts as a word.
                if self.columns(at.line) == 0 {
                    continue 'times;
                }
                at = match self.backward(at) {
                    Some(previous) => previous,
                    None => return at,
                };
            }

            let class: Class = self.class(at, big);
            while self.class(at, big) == class {
                at = match self.backward(at) {
                    Some(previous) => previous,
                    None => return at,
                };
            }
            at = self.forward(at).unwrap_or(at);
        }
        return at;
    }

    /// Return where moving to the end of the word the number of times goes, the way vim's `e`
    /// does, along with its `E` for a big word.
    pub fn word_end(&self, at: Position, times: usize, big: bool) -> Position {
        let mut at: Position = at;
        for _ in 0..times {
            let class: Class = self.class(at, big);
            at = match self.forward(at) {
                Some(next) => next,
                None => return at,
            };

            // NOTE: Unlike moving to the start of a word, an empty line does not count as one.
            if class == Class::Blank || self.class(at, big) != class {
                while self.class(at, big) == Class::Blank {
                    at = match self.forward(at) {
                        Some(next) => next,
                        None => return at,
                    };
                }
            }

            let class: Class = self.class(at, big);
            while self.class(at, big) == class {
                at = match self.forward(at) {
                    Some(next) => next,
                    None => return at,
                };
            }
            at = self.backward(at).unwrap_or(at);
        }
        return at;
    }

    /// Return the next hit of the phrase after the position, or the one before it going back,
    /// wrapping around the end of the text, the way vim's `n` and `N` do.
    pub fn find(&self, phrase: &str, at: Position, forward: bool) -> Option<Hit> {
        if phrase.is_empty() {
            return None;
        }

        let hits = self.lines.iter().enumerate().flat_map(|(line, text)| {
            text.char_indices()
                .enumerate()
                .filter(move |(_, (byte, _))| text[*byte..].starts_with(phrase))
                .map(move |(column, _)| Position::new(line, column))
        });

        let mut wrapped: Option<Position> = None;
        let mut found: Option<Position> = None;
        for hit in hits {
            match forward {
                true if hit > at => return Some(Hit::builder().at(hit).build()),
                true => {
                    wrapped.get_or_insert(hit);
                }
                false => {
                    if hit < at {
                        found = Some(hit);
                    }
                    wrapped = Some(hit);
                }
            }
        }

        if let Some(at) = found {
            return Some(Hit::builder().at(at).build());
        }
        return wrapped.map(|at| Hit::builder().at(at).wrapped(true).build());
    }

    /// Return the position after the one given, counting the end of each line as one, or nothing
    /// at the end of the text.
    fn forward(&self, at: Position) -> Option<Position> {
        if at.column < self.columns(at.line) {
            return Some(Position::new(at.line, at.column + 1));
        }
        if at.line + 1 < self.lines.len() {
            return Some(Position::new(at.line + 1, 0));
        }
        return None;
    }

    /// Return the position before the one given, counting the end of each line as one, or
    /// nothing at the start of the text.
    fn backward(&self, at: Position) -> Option<Position> {
        if at.column > 0 {
            return Some(Position::new(at.line, at.column - 1));
        }
        if at.line > 0 {
            return Some(Position::new(at.line - 1, self.columns(at.line - 1)));
        }
        return None;
    }

    /// Return whether moving forward from the one position to the other went off the last
    /// character of a line, either onto the end of it or onto the next line.
    fn stepped_off(&self, from: Position, to: Position) -> bool {
        return to.line != from.line || to.column == self.columns(to.line);
    }

    /// Return what kind of character is at the position, for moving by words.
    fn class(&self, at: Position, big: bool) -> Class {
        let character: Option<char> = self
            .lines
            .get(at.line)
            .and_then(|line| line.chars().nth(at.column));
        return match character {
            None | Some(' ' | '\t') => Class::Blank,
            // NOTE: A big word is any run of characters which are not blank.
            Some(_) if big => Class::Keyword,
            Some(character) if character.is_alphanumeric() || character == '_' => Class::Keyword,
            Some(_) => Class::Punctuation,
        };
    }

    /// Return the text between the two positions.
    pub fn slice(&self, from: Position, to: Position) -> String {
        let (from, to): (Position, Position) = (self.clamp(from), self.clamp(to));
        if from >= to {
            return String::new();
        }

        if from.line == to.line {
            let line: &str = &self.lines[from.line];
            return line[self.byte(from)..self.byte(to)].to_string();
        }

        let mut slice: String = self.lines[from.line][self.byte(from)..].to_string();
        for line in &self.lines[from.line + 1..to.line] {
            slice.push('\n');
            slice.push_str(line);
        }
        slice.push('\n');
        slice.push_str(&self.lines[to.line][..self.byte(to)]);
        return slice;
    }

    /// Put the string in at the position, and return the change which that was.
    pub fn insert(&mut self, at: Position, string: &str) -> Edit {
        let at: Position = self.clamp(at);
        let byte: usize = self.byte(at);

        let rest: String = self.lines[at.line].split_off(byte);
        let mut pieces = string.split('\n');
        // NOTE: Splitting always gives at least one piece, even of an empty string.
        self.lines[at.line].push_str(pieces.next().unwrap_or_default());

        let mut line: usize = at.line;
        for piece in pieces {
            line += 1;
            self.lines.insert(line, piece.to_string());
        }

        let end: Position = Position::new(line, self.columns(line));
        self.lines[line].push_str(&rest);

        return Edit::Inserted { at, end };
    }

    /// Take out what is between the two positions, and return what it was along with the change
    /// which that was.
    pub fn remove(&mut self, from: Position, to: Position) -> (String, Edit) {
        let (from, to): (Position, Position) = (self.clamp(from), self.clamp(to));
        let removed: String = self.slice(from, to);
        if from >= to {
            return (removed, Edit::Removed { from, to: from });
        }

        let rest: String = self.lines[to.line][self.byte(to)..].to_string();
        let byte: usize = self.byte(from);
        self.lines[from.line].truncate(byte);
        self.lines[from.line].push_str(&rest);
        self.lines.drain(from.line + 1..=to.line);

        return (removed, Edit::Removed { from, to });
    }

    /// Return the byte of its line which the position is at.
    fn byte(&self, at: Position) -> usize {
        let line: &str = &self.lines[at.line];
        return match line.char_indices().nth(at.column) {
            Some((byte, _)) => byte,
            None => line.len(),
        };
    }
}

/// Where a phrase searched for was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TypedBuilder)]
pub struct Hit {
    /// Where it starts.
    pub at: Position,
    /// Whether the search went around the end of the text to get to it.
    #[builder(default)]
    pub wrapped: bool,
}

/// What kind of character something is, for moving by words.
///
/// A word is a run of characters of the same kind which are not blank, the same as in vim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A space, a tab, or the end of a line.
    Blank,
    /// A character which is not a letter, a digit or an underscore.
    Punctuation,
    /// A letter, a digit or an underscore.
    Keyword,
}

/// A change made to the text, which is what says where everything after it has gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    /// Something was put in.
    Inserted {
        /// Where it was put.
        at: Position,
        /// Where it ends.
        end: Position,
    },
    /// Something was taken out.
    Removed {
        /// Where it started.
        from: Position,
        /// Where it ended.
        to: Position,
    },
}

impl Edit {
    /// Return where the text which was changed starts.
    pub fn start(&self) -> Position {
        return match *self {
            Self::Inserted { at, .. } => at,
            Self::Removed { from, .. } => from,
        };
    }

    /// Return where the text which was changed ends now.
    pub fn end(&self) -> Position {
        return match *self {
            Self::Inserted { end, .. } => end,
            Self::Removed { from, .. } => from,
        };
    }

    /// Return where the position is after the change, so that a cursor stays on the text it was
    /// on when someone else changes the text before it.
    pub fn adjust(&self, position: Position) -> Position {
        match *self {
            Self::Inserted { at, end } => {
                if position < at {
                    return position;
                }
                if position.line == at.line {
                    return Position::new(end.line, end.column + position.column - at.column);
                }
                return Position::new(position.line + end.line - at.line, position.column);
            }
            Self::Removed { from, to } => {
                if position <= from {
                    return position;
                }
                if position < to {
                    return from;
                }
                if position.line == to.line {
                    return Position::new(from.line, from.column + position.column - to.column);
                }
                return Position::new(position.line - (to.line - from.line), position.column);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Edit, Hit, Text};

    use insh_api::Position;

    use test_case::test_case;

    #[test_case("", 1; "an empty file has one line")]
    #[test_case("a\nb", 2; "two lines")]
    #[test_case("a\n", 2; "a newline at the end starts another line")]
    fn test_lines(string: &str, expected: usize) {
        assert_eq!(Text::from(string).len(), expected);
    }

    #[test_case("abc", Position::new(0, 1), "X", "aXbc", Position::new(0, 2); "in a line")]
    #[test_case("abc", Position::new(0, 1), "X\nY", "aX\nYbc", Position::new(1, 1); "a newline")]
    #[test_case("abc", Position::new(0, 3), "\n", "abc\n", Position::new(1, 0); "at the end")]
    #[test_case("🙂b", Position::new(0, 1), "X", "🙂Xb", Position::new(0, 2); "after a wide character")]
    fn test_insert(string: &str, at: Position, inserted: &str, expected: &str, end: Position) {
        let mut text: Text = Text::from(string);

        let edit: Edit = text.insert(at, inserted);

        assert_eq!(text, Text::from(expected));
        assert_eq!(edit, Edit::Inserted { at, end });
    }

    #[test_case("abc", Position::new(0, 1), Position::new(0, 2), "ac", "b"; "in a line")]
    #[test_case("ab\ncd", Position::new(0, 1), Position::new(1, 1), "ad", "b\nc"; "across lines")]
    #[test_case("ab\ncd", Position::new(0, 2), Position::new(1, 0), "abcd", "\n"; "a newline")]
    fn test_remove(string: &str, from: Position, to: Position, expected: &str, removed: &str) {
        let mut text: Text = Text::from(string);

        let (actual_removed, _): (String, Edit) = text.remove(from, to);

        assert_eq!(text, Text::from(expected));
        assert_eq!(actual_removed, removed);
    }

    #[test_case(Position::new(0, 0), Position::new(0, 0); "before is left alone")]
    #[test_case(Position::new(0, 4), Position::new(1, 2); "after on the same line moves down")]
    #[test_case(Position::new(2, 1), Position::new(3, 1); "a later line moves down")]
    fn test_adjust_for_insert(position: Position, expected: Position) {
        let edit = Edit::Inserted {
            at: Position::new(0, 2),
            end: Position::new(1, 0),
        };

        assert_eq!(edit.adjust(position), expected);
    }

    #[test_case(Position::new(0, 1), Position::new(0, 1); "before is left alone")]
    #[test_case(Position::new(1, 0), Position::new(0, 2); "inside goes to the start")]
    #[test_case(Position::new(1, 3), Position::new(0, 3); "after on the last line joins")]
    #[test_case(Position::new(2, 1), Position::new(1, 1); "a later line moves up")]
    fn test_adjust_for_remove(position: Position, expected: Position) {
        let edit = Edit::Removed {
            from: Position::new(0, 2),
            to: Position::new(1, 2),
        };

        assert_eq!(edit.adjust(position), expected);
    }

    #[test_case("abc abc", "abc", Position::new(0, 0), true, Some((Position::new(0, 4), false)); "forward")]
    #[test_case("abc\nabc", "abc", Position::new(0, 0), true, Some((Position::new(1, 0), false)); "forward onto the next line")]
    #[test_case("abc abc", "abc", Position::new(0, 4), true, Some((Position::new(0, 0), true)); "forward wraps around")]
    #[test_case("abc", "abc", Position::new(0, 0), true, Some((Position::new(0, 0), true)); "forward onto the only hit")]
    #[test_case("abc abc", "abc", Position::new(0, 4), false, Some((Position::new(0, 0), false)); "back")]
    #[test_case("abc abc", "abc", Position::new(0, 0), false, Some((Position::new(0, 4), true)); "back wraps around")]
    #[test_case("aaa", "aa", Position::new(0, 0), true, Some((Position::new(0, 1), false)); "hits overlap")]
    #[test_case("🙂abc", "abc", Position::new(0, 0), true, Some((Position::new(0, 1), false)); "after a wide character")]
    #[test_case("abc", "x", Position::new(0, 0), true, None; "no hit")]
    fn test_find(
        string: &str,
        phrase: &str,
        at: Position,
        forward: bool,
        expected: Option<(Position, bool)>,
    ) {
        let text: Text = Text::from(string);

        let hit: Option<Hit> = text.find(phrase, at, forward);

        assert_eq!(hit.map(|hit| (hit.at, hit.wrapped)), expected);
    }
}

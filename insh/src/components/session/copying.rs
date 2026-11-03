//! Contains the [`Copying`] struct, which is where in a pane is being looked at and taken from
//! while it is being looked back through.

/// Contains the [`Moving`] enum.
mod moving {
    /// Where in the pane to go.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Moving {
        /// A column to the left.
        Left,
        /// A row down.
        Down,
        /// A row up.
        Up,
        /// A column to the right.
        Right,
        /// To the first column of the row.
        Start,
        /// To the last column of the row which has anything written in it.
        End,
        /// To the start of the next word.
        NextWord,
        /// To the start of the next run of anything but blanks.
        NextBigWord,
        /// To the start of the word, or of the one before it when already there.
        PreviousWord,
        /// To the start of the run of anything but blanks, or of the one before it when already
        /// there.
        PreviousBigWord,
        /// To the end of the word, or of the one after it when already there.
        EndOfWord,
        /// To the end of the run of anything but blanks, or of the one after it when already
        /// there.
        EndOfBigWord,
    }
}
pub use moving::Moving;

/// Contains the [`Class`] enum.
mod class {
    use rend::Cell;

    /// What sort of character is written in a spot, which is what says where a word starts and
    /// ends.
    ///
    /// A word is a run of letters, digits and underscores, or a run of anything else which is not
    /// a blank, the way it is in vi and in tmux. A big word is a run of anything at all but blanks.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Class {
        /// Nothing, or a space.
        Blank,
        /// A letter, a digit or an underscore, or anything at all which is not a blank in a big
        /// word.
        Word,
        /// Anything else.
        Punctuation,
    }

    impl Class {
        /// Return the sort of character written in the cell, where a big word is made of every
        /// one but a blank.
        pub fn of(cell: &Cell, big: bool) -> Self {
            let class: Self = match cell {
                Cell::Char(character) if character.is_whitespace() => Self::Blank,
                Cell::Char(character) if character.is_alphanumeric() || *character == '_' => {
                    Self::Word
                }
                Cell::Char(_) => Self::Punctuation,
                Cell::Cluster(_) | Cell::Continuation => Self::Word,
            };

            return match (class, big) {
                (Self::Punctuation, true) => Self::Word,
                (class, _) => class,
            };
        }
    }
}
use class::Class;

/// Contains the [`Copying`] struct.
mod copying {
    use super::{Class, Moving};
    use crate::color::Color;

    use rend::{Cell, Fabric, Location, Size, Style};

    /// Where in a pane is being looked at, and what of it is being taken.
    ///
    /// A pane which is being looked back through has a spot in it which stands for where in it is
    /// being read, the way the cursor of a program stands for where it is writing. Marking a spot
    /// is what begins taking from there, and everything from that spot to this one is taken.
    ///
    /// The spot being read is somewhere on the screen of the pane, and stays there as the pane is
    /// looked further back through. The spot which was marked stays with what was written there
    /// instead, so it is counted from the first row there ever was in the pane rather than from the
    /// top of what is being looked at, which is what lets what is taken be more than fits on the
    /// screen.
    #[derive(Debug)]
    pub struct Copying {
        /// How big the pane was when it was last drawn, which is as far as there is to go in it.
        size: Size,
        /// Where in the pane is being looked at.
        at: Location,
        /// Where taking from the pane was begun, counted from the first row there ever was in it,
        /// which is nowhere until it is begun.
        from: Option<Location>,
        /// Whether the first of the two keys which go the whole way back has been pressed.
        ///
        /// NOTE: It takes two of them, the way it does in vim and in tmux, so one on its own waits
        /// to see whether the other follows rather than going anywhere.
        waiting: bool,
        /// Whether looking back through the pane stops once the bottom of it is reached again,
        /// which is what it does when it was scrolled into rather than asked for.
        leaving_at_bottom: bool,
    }

    impl Copying {
        /// Return the given spot in a pane of the given size.
        ///
        /// Where to start is where the pane has its cursor, so that looking back through a pane
        /// begins where whatever is in it was last writing rather than somewhere else.
        pub fn of(size: Size, at: Location) -> Self {
            let mut copying = Self {
                size,
                at,
                from: None,
                waiting: false,
                leaving_at_bottom: false,
            };
            copying.at = copying.inside(at);

            return copying;
        }

        /// Return the same, but stopping once the bottom of the pane is scrolled back to, the way
        /// tmux does when the mouse wheel is what began looking back.
        pub fn leaving_at_bottom(mut self) -> Self {
            self.leaving_at_bottom = true;
            return self;
        }

        /// Return whether scrolling back to the bottom of the pane stops looking back through it.
        ///
        /// It never does while something is being taken, since that would throw it away.
        pub fn leaves_at_bottom(&self) -> bool {
            return self.leaving_at_bottom && self.from.is_none();
        }

        /// Take note that the key which goes the whole way back was pressed, and return whether it
        /// was the second of the two it takes.
        pub fn go_back(&mut self) -> bool {
            let waiting: bool = self.waiting;
            self.waiting = !waiting;

            return waiting;
        }

        /// Take note that a key which is not the one which goes the whole way back was pressed, so
        /// that one of those which came before it no longer counts toward the two it takes.
        pub fn other_key(&mut self) {
            self.waiting = false;
        }

        /// Take note of how big the pane is now, keeping the spot inside it.
        ///
        /// NOTE: A pane changes size while it is being looked back through whenever a line between
        /// two of them is moved or the terminal itself does, so where there is to go in it is
        /// taken from what was last drawn rather than from what it was when this began. What is
        /// being taken is let go of when it does, since a program's rows are wrapped again for the
        /// new width and the spot which was marked no longer says where it was.
        pub fn resize(&mut self, size: Size) {
            if size != self.size {
                self.from = None;
            }
            self.size = size;
            self.at = self.inside(self.at);
        }

        /// Return the spot, moved to the nearest one which is in the pane.
        fn inside(&self, at: Location) -> Location {
            return Location {
                row: at.row.min(self.size.rows.saturating_sub(1)),
                column: at.column.min(self.size.columns.saturating_sub(1)),
            };
        }

        /// Begin taking from where it is now, in a pane whose top row is the given one counted
        /// from the first row there ever was in it.
        pub fn mark(&mut self, top: usize) {
            self.from = Some(Location {
                row: top + self.at.row,
                column: self.at.column,
            });
        }

        /// Stop taking anything at all, leaving where in the pane is being looked at where it is.
        pub fn unmark(&mut self) {
            self.from = None;
        }

        /// Go the given way in the pane, which is drawn as given, and return how many rows of what
        /// has scrolled off the top to look further back through.
        ///
        /// Going up from the top row or down from the bottom one is what moves the pane itself,
        /// since there is nowhere left in it to go that way. Going by words only goes as far as
        /// what is drawn, and stays where it is when there is no word left on it to go to.
        pub fn move_toward(&mut self, moving: Moving, fabric: &Fabric) -> isize {
            let last: Location = Location {
                row: self.size.rows.saturating_sub(1),
                column: self.size.columns.saturating_sub(1),
            };

            match moving {
                Moving::Left => self.at.column = self.at.column.saturating_sub(1),
                Moving::Right => self.at.column = (self.at.column + 1).min(last.column),
                Moving::Start => self.at.column = 0,
                Moving::End => self.at.column = Self::end(fabric, self.at.row).min(last.column),
                Moving::NextWord
                | Moving::NextBigWord
                | Moving::PreviousWord
                | Moving::PreviousBigWord
                | Moving::EndOfWord
                | Moving::EndOfBigWord => self.by_word(moving, fabric),
                Moving::Up => match self.at.row {
                    0 => return 1,
                    row => self.at.row = row - 1,
                },
                Moving::Down => match self.at.row {
                    row if row == last.row => return -1,
                    row => self.at.row = row + 1,
                },
            }

            return 0;
        }

        /// Return the column of the last thing written in the row, which is the first column when
        /// nothing is.
        ///
        /// NOTE: A terminal writes a row out to its full width with blanks, so the last column of
        /// the row is past the end of whatever is in it, which is not where the end of it is.
        fn end(fabric: &Fabric, row: usize) -> usize {
            let columns: usize = fabric.size().columns;
            if row >= fabric.size().rows {
                return 0;
            }

            let mut end: usize = match (0..columns)
                .rev()
                .find(|column| *fabric.spot(row, *column).cell() != Cell::BLANK)
            {
                Some(end) => end,
                None => return 0,
            };

            // The last column of a wide cluster is not one of its own, so it goes to the column
            // the cluster starts in.
            while end > 0 && fabric.spot(row, end).cell().is_continuation() {
                end -= 1;
            }

            return end;
        }

        /// Go to the word the given way along what is drawn.
        fn by_word(&mut self, moving: Moving, fabric: &Fabric) {
            let big: bool = matches!(
                moving,
                Moving::NextBigWord | Moving::PreviousBigWord | Moving::EndOfBigWord
            );
            let spots: Vec<(Option<Location>, Class)> = Self::read(fabric, big);
            if spots.is_empty() {
                return;
            }

            // NOTE: Where it is may be the second column of a wide cluster, which is not a spot of
            // its own, so it is taken as being at the last spot which is not after it.
            let here: usize = spots
                .iter()
                .rposition(|(at, _)| {
                    at.is_some_and(|at| (at.row, at.column) <= (self.at.row, self.at.column))
                })
                .unwrap_or(0);

            let to: usize = match moving {
                Moving::NextWord | Moving::NextBigWord => Self::next_word(&spots, here),
                Moving::PreviousWord | Moving::PreviousBigWord => Self::previous_word(&spots, here),
                _ => Self::end_of_word(&spots, here),
            };

            if let (Some(at), _) = spots[to] {
                self.at = at;
            }
        }

        /// Return every spot which is drawn, in the order it is read in, along with the sort of
        /// character written in it.
        ///
        /// The end of each row is a blank which is nowhere, since a word never carries on from
        /// one row onto the next.
        fn read(fabric: &Fabric, big: bool) -> Vec<(Option<Location>, Class)> {
            let mut spots: Vec<(Option<Location>, Class)> = Vec::new();

            for row in 0..fabric.size().rows {
                for column in 0..fabric.size().columns {
                    let cell: Cell = fabric.spot(row, column).cell().clone();
                    if cell.is_continuation() {
                        continue;
                    }
                    spots.push((Some(Location { row, column }), Class::of(&cell, big)));
                }
                spots.push((None, Class::Blank));
            }

            return spots;
        }

        /// Return the start of the word after the one at the given spot, which is the spot itself
        /// when there is none.
        fn next_word(spots: &[(Option<Location>, Class)], here: usize) -> usize {
            let class: Class = spots[here].1;

            let mut to: usize = here;
            if class != Class::Blank {
                while to + 1 < spots.len() && spots[to + 1].1 == class {
                    to += 1;
                }
            }
            to += 1;
            while to < spots.len() && spots[to].1 == Class::Blank {
                to += 1;
            }

            return match to < spots.len() {
                true => to,
                false => here,
            };
        }

        /// Return the start of the word at the given spot, or of the one before it when the spot
        /// is the start of one already, which is the spot itself when there is none.
        fn previous_word(spots: &[(Option<Location>, Class)], here: usize) -> usize {
            if here == 0 {
                return here;
            }

            let mut to: usize = here - 1;
            while to > 0 && spots[to].1 == Class::Blank {
                to -= 1;
            }
            if spots[to].1 == Class::Blank {
                return here;
            }
            while to > 0 && spots[to - 1].1 == spots[to].1 {
                to -= 1;
            }

            return to;
        }

        /// Return the end of the word at the given spot, or of the one after it when the spot is
        /// the end of one already, which is the spot itself when there is none.
        fn end_of_word(spots: &[(Option<Location>, Class)], here: usize) -> usize {
            let mut to: usize = here + 1;
            while to < spots.len() && spots[to].1 == Class::Blank {
                to += 1;
            }
            if to >= spots.len() {
                return here;
            }
            while to + 1 < spots.len() && spots[to + 1].1 == spots[to].1 {
                to += 1;
            }

            return to;
        }

        /// Mark what is being taken, and where in the pane is being looked at, in a pane whose top
        /// row is the given one counted from the first row there ever was in it.
        ///
        /// The spot is marked whether or not anything is being taken from it, since it is the one
        /// thing which says where going along the pane goes from.
        pub fn show(&self, fabric: &mut Fabric, top: usize) {
            let mut style = Style::default();
            style.set_background(Some(Color::Highlight.into()));
            style.set_color(Some(Color::InvertedText.into()));

            for (row, columns) in self.taken(top) {
                fabric.style(style, row, columns);
            }
        }

        /// Return the spots what is being taken goes from and to, in order and counted from the
        /// first row there ever was in a pane whose top row is the given one.
        ///
        /// Nothing is taken until a spot has been marked, and what is taken then is everything
        /// from that spot to this one however they are ordered, so that going back on yourself
        /// takes less rather than nothing.
        pub fn selection(&self, top: usize) -> Option<(Location, Location)> {
            let from: Location = self.from?;
            let at: Location = Location {
                row: top + self.at.row,
                column: self.at.column,
            };

            return match (from.row, from.column) <= (at.row, at.column) {
                true => Some((from, at)),
                false => Some((at, from)),
            };
        }

        /// Return what is being taken from the pane, which is what was drawn in it.
        ///
        /// This is only all of what is being taken when it is all on the screen, which it always is
        /// for a pane which has nothing which has scrolled off the top of it.
        pub fn text(&self, fabric: &Fabric, top: usize) -> Option<String> {
            self.from?;

            // NOTE: The rows are joined with newlines and each has what is written in the columns
            // it is taken across, with the blanks at the end of it left off. A terminal writes a
            // row out to its full width with blanks, so keeping them would put a run of spaces
            // after every line of what was taken.
            let taken: Vec<String> = self
                .taken(top)
                .map(|(row, columns)| fabric.text(row, columns).trim_end().to_string())
                .collect();

            return Some(taken.join("\n"));
        }

        /// Return the columns of each row on the screen which are being taken, in a pane whose top
        /// row is the given one counted from the first row there ever was in it.
        ///
        /// A row between the first and the last is taken whole, and the ones at either end are
        /// taken from or up to the spot which is in them, which is how a terminal takes a run of
        /// text rather than a block of it.
        fn taken(&self, top: usize) -> impl Iterator<Item = (usize, std::ops::Range<usize>)> + '_ {
            let at: Location = Location {
                row: top + self.at.row,
                column: self.at.column,
            };
            let (first, last): (Location, Location) = self.selection(top).unwrap_or((at, at));

            let columns: usize = self.size.columns;
            let shown: std::ops::Range<usize> = top..top + self.size.rows;

            return (first.row.max(shown.start)..=last.row.min(shown.end.saturating_sub(1))).map(
                move |row| {
                    let start: usize = match row == first.row {
                        true => first.column,
                        false => 0,
                    };
                    let end: usize = match row == last.row {
                        true => (last.column + 1).min(columns),
                        false => columns,
                    };

                    (row - top, start..end)
                },
            );
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use test_case::test_case;

        /// The spot which a pane whose cursor is not shown is looked back through from.
        const TOP_LEFT: Location = Location { row: 0, column: 0 };

        #[test_case(&[], Some("b"); "where it was begun, which is the one column")]
        #[test_case(&[Moving::Right], Some("bc"); "a column along")]
        #[test_case(&[Moving::Right, Moving::Right], Some("bcd"); "two columns along")]
        #[test_case(&[Moving::Left], Some("ab"); "a column back, which takes the one before it")]
        #[test_case(&[Moving::Down], Some("bcd\nef"); "a row down, which takes the rest of the row and the start of the next")]
        #[test_case(&[Moving::Down, Moving::Down], Some("bcd\nefgh\nij"); "two rows down, which takes the one between them whole")]
        #[test_case(&[Moving::End], Some("bcd"); "to the end of the row")]
        #[test_case(&[Moving::Down, Moving::Start], Some("bcd\ne"); "to the start of the row below")]
        fn test_text(moves: &[Moving], expected: Option<&str>) {
            let fabric = Fabric::from(vec!["abcd", "efgh", "ijkl"]);
            let mut copying = Copying::of(fabric.size(), TOP_LEFT);

            // Taking is begun a column in, so that going back on yourself can be told from not
            // having moved at all.
            copying.move_toward(Moving::Right, &fabric);
            copying.mark(0);
            for moving in moves {
                copying.move_toward(*moving, &fabric);
            }

            assert_eq!(copying.text(&fabric, 0).as_deref(), expected);
        }

        #[test_case(Location { row: 1, column: 2 }, "gh"; "where the pane has its cursor")]
        #[test_case(Location { row: 9, column: 9 }, "l"; "a spot which is not in the pane, brought inside it")]
        fn test_of(at: Location, expected: &str) {
            let fabric = Fabric::from(vec!["abcd", "efgh", "ijkl"]);
            let mut copying = Copying::of(fabric.size(), at);

            copying.mark(0);
            copying.move_toward(Moving::End, &fabric);

            assert_eq!(copying.text(&fabric, 0).as_deref(), Some(expected));
        }

        #[test_case(1, false; "one of them, which waits for the other")]
        #[test_case(2, true; "both of them")]
        #[test_case(3, false; "three of them, which is one into the next two")]
        fn test_go_back(times: usize, expected: bool) {
            let mut copying = Copying::of(Size::new(2, 4), TOP_LEFT);

            let mut went: bool = false;
            for _ in 0..times {
                went = copying.go_back();
            }

            assert_eq!(went, expected);
        }

        #[test]
        fn test_other_key_stops_the_one_before_it_counting_toward_the_two() {
            let mut copying = Copying::of(Size::new(2, 4), TOP_LEFT);

            copying.go_back();
            copying.other_key();

            assert!(!copying.go_back(), "it should wait for another");
        }

        #[test]
        fn test_text_is_nothing_until_a_spot_has_been_marked() {
            let fabric = Fabric::from(vec!["abcd", "efgh"]);
            let copying = Copying::of(fabric.size(), TOP_LEFT);

            assert_eq!(copying.text(&fabric, 0), None);
        }

        #[test]
        fn test_unmark_stops_anything_being_taken() {
            let fabric = Fabric::from(vec!["abcd", "efgh"]);
            let mut copying = Copying::of(fabric.size(), TOP_LEFT);

            copying.mark(0);
            copying.unmark();

            assert_eq!(copying.text(&fabric, 0), None);
        }

        #[test_case(Moving::Up, 1; "up from the top row")]
        #[test_case(Moving::Down, 0; "down from the top row, which is a row to go to")]
        fn test_move_toward(moving: Moving, expected: isize) {
            let fabric = Fabric::new(Size::new(2, 4));
            let mut copying = Copying::of(fabric.size(), TOP_LEFT);

            assert_eq!(copying.move_toward(moving, &fabric), expected);
        }

        #[test]
        fn test_moving_down_from_the_last_row_looks_back_the_other_way() {
            let fabric = Fabric::new(Size::new(2, 4));
            let mut copying = Copying::of(fabric.size(), TOP_LEFT);

            copying.move_toward(Moving::Down, &fabric);

            assert_eq!(copying.move_toward(Moving::Down, &fabric), -1);
        }

        #[test_case(Location { row: 0, column: 0 }, 3; "a row with blanks after what is written in it")]
        #[test_case(Location { row: 1, column: 0 }, 0; "a row with nothing written in it")]
        #[test_case(Location { row: 2, column: 0 }, 1; "a row which ends with a wide cluster")]
        fn test_moving_to_the_end_goes_to_the_last_thing_written(at: Location, expected: usize) {
            let fabric = Fabric::from(vec!["ab c  ", "", "a🦀"]);
            let mut copying = Copying::of(Size::new(3, 6), at);

            copying.move_toward(Moving::End, &fabric);
            copying.mark(0);

            assert_eq!(
                copying.selection(0).map(|(_, to)| to.column),
                Some(expected)
            );
        }

        #[test_case(Moving::NextWord, (0, 3); "the next word, which punctuation is one of")]
        #[test_case(Moving::NextBigWord, (0, 8); "the next big word, which runs past punctuation")]
        #[test_case(Moving::EndOfWord, (0, 2); "the end of the word")]
        #[test_case(Moving::EndOfBigWord, (0, 6); "the end of the big word")]
        #[test_case(Moving::PreviousWord, (0, 0); "the start of the word")]
        fn test_moving_by_words(moving: Moving, expected: (usize, usize)) {
            let fabric = Fabric::from(vec!["foo.bar baz", "qux"]);
            let mut copying = Copying::of(fabric.size(), Location { row: 0, column: 1 });

            copying.move_toward(moving, &fabric);
            copying.mark(0);

            let at: Location = copying.selection(0).unwrap().0;
            assert_eq!((at.row, at.column), expected);
        }

        #[test]
        fn test_moving_by_words_goes_onto_the_next_row() {
            let fabric = Fabric::from(vec!["foo.bar baz", "qux"]);
            let mut copying = Copying::of(fabric.size(), Location { row: 0, column: 9 });

            copying.move_toward(Moving::NextWord, &fabric);
            copying.mark(0);
            assert_eq!(
                copying.selection(0).unwrap().0,
                Location { row: 1, column: 0 }
            );

            copying.move_toward(Moving::PreviousBigWord, &fabric);
            copying.mark(0);
            assert_eq!(
                copying.selection(0).unwrap().0,
                Location { row: 0, column: 8 }
            );
        }

        #[test]
        fn test_moving_by_words_stays_put_when_there_is_no_word_left() {
            let fabric = Fabric::from(vec!["foo  "]);
            let mut copying = Copying::of(fabric.size(), Location { row: 0, column: 1 });

            copying.move_toward(Moving::NextWord, &fabric);
            copying.mark(0);

            assert_eq!(
                copying.selection(0).unwrap().0,
                Location { row: 0, column: 1 }
            );
        }

        #[test]
        fn test_resizing_keeps_the_spot_inside_the_pane() {
            let fabric = Fabric::from(vec!["abcd", "efgh", "ijkl"]);
            let mut copying = Copying::of(fabric.size(), TOP_LEFT);

            copying.move_toward(Moving::Down, &fabric);
            copying.move_toward(Moving::End, &fabric);
            copying.mark(0);
            copying.resize(Size::new(1, 2));

            // Where it is is brought back inside the pane, and what was being taken is let go of,
            // since what is in the pane has been wrapped again for its new width.
            assert_eq!(copying.text(&fabric, 0), None);
            copying.mark(0);
            assert_eq!(copying.text(&fabric, 0).as_deref(), Some("b"));
        }

        #[test]
        fn test_what_is_taken_stays_with_what_was_written_as_the_pane_is_looked_back_through() {
            let fabric = Fabric::new(Size::new(2, 4));
            let mut copying = Copying::of(fabric.size(), TOP_LEFT);

            // Taking is begun on the bottom row of a pane whose top row is the tenth there was.
            copying.move_toward(Moving::Down, &fabric);
            copying.mark(10);
            // Looking two rows further back moves the pane, not where in it is being looked at.
            copying.move_toward(Moving::Up, &fabric);

            assert_eq!(
                copying.selection(8),
                Some((
                    Location { row: 8, column: 0 },
                    Location { row: 11, column: 0 }
                ))
            );
        }
    }
}
pub use copying::Copying;

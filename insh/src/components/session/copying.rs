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
        /// To the last column of the row.
        End,
    }
}
pub use moving::Moving;

/// Contains the [`Copying`] struct.
mod copying {
    use super::Moving;
    use crate::color::Color;

    use rend::{Fabric, Location, Size, Style};

    /// Where in a pane is being looked at, and what of it is being taken.
    ///
    /// A pane which is being looked back through has a spot in it which stands for where in it is
    /// being read, the way the cursor of a program stands for where it is writing. Marking a spot
    /// is what begins taking from there, and everything from that spot to this one is taken.
    #[derive(Debug)]
    pub struct Copying {
        /// How big the pane was when it was last drawn, which is as far as there is to go in it.
        size: Size,
        /// Where in the pane is being looked at.
        at: Location,
        /// Where taking from the pane was begun, which is nowhere until it is begun.
        from: Option<Location>,
        /// Whether the first of the two keys which go the whole way back has been pressed.
        ///
        /// NOTE: It takes two of them, the way it does in vim and in tmux, so one on its own waits
        /// to see whether the other follows rather than going anywhere.
        waiting: bool,
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
            };
            copying.at = copying.inside(at);

            return copying;
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
        /// taken from what was last drawn rather than from what it was when this began.
        pub fn resize(&mut self, size: Size) {
            self.size = size;
            self.at = self.inside(self.at);
            self.from = self.from.map(|from| self.inside(from));
        }

        /// Return the spot, moved to the nearest one which is in the pane.
        fn inside(&self, at: Location) -> Location {
            return Location {
                row: at.row.min(self.size.rows.saturating_sub(1)),
                column: at.column.min(self.size.columns.saturating_sub(1)),
            };
        }

        /// Begin taking from where it is now.
        pub fn mark(&mut self) {
            self.from = Some(self.at);
        }

        /// Stop taking anything at all, leaving where in the pane is being looked at where it is.
        pub fn unmark(&mut self) {
            self.from = None;
        }

        /// Go the given way in the pane, and return how many rows of what has scrolled off the top
        /// to look further back through.
        ///
        /// Going up from the top row or down from the bottom one is what moves the pane itself,
        /// since there is nowhere left in it to go that way.
        pub fn move_toward(&mut self, moving: Moving) -> isize {
            let last: Location = Location {
                row: self.size.rows.saturating_sub(1),
                column: self.size.columns.saturating_sub(1),
            };

            match moving {
                Moving::Left => self.at.column = self.at.column.saturating_sub(1),
                Moving::Right => self.at.column = (self.at.column + 1).min(last.column),
                Moving::Start => self.at.column = 0,
                Moving::End => self.at.column = last.column,
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

        /// Mark what is being taken, and where in the pane is being looked at.
        ///
        /// The spot is marked whether or not anything is being taken from it, since it is the one
        /// thing which says where going along the pane goes from.
        pub fn show(&self, fabric: &mut Fabric) {
            let mut style = Style::default();
            style.set_background(Some(Color::Highlight.into()));
            style.set_color(Some(Color::InvertedText.into()));

            for (row, columns) in self.taken() {
                fabric.style(style, row, columns);
            }
        }

        /// Return what is being taken from the pane, which is what was drawn in it.
        ///
        /// Nothing is taken until a spot has been marked, and what is taken then is everything
        /// from that spot to this one however they are ordered, so that going back on yourself
        /// takes less rather than nothing.
        pub fn text(&self, fabric: &Fabric) -> Option<String> {
            self.from?;

            // NOTE: The rows are joined with newlines and each has what is written in the columns
            // it is taken across, with the blanks at the end of it left off. A terminal writes a
            // row out to its full width with blanks, so keeping them would put a run of spaces
            // after every line of what was taken.
            let taken: Vec<String> = self
                .taken()
                .map(|(row, columns)| fabric.text(row, columns).trim_end().to_string())
                .collect();

            return Some(taken.join("\n"));
        }

        /// Return the columns of each row which are being taken.
        ///
        /// A row between the first and the last is taken whole, and the ones at either end are
        /// taken from or up to the spot which is in them, which is how a terminal takes a run of
        /// text rather than a block of it.
        fn taken(&self) -> impl Iterator<Item = (usize, std::ops::Range<usize>)> + '_ {
            let from: Location = self.from.unwrap_or(self.at);
            let (first, last): (Location, Location) =
                match (from.row, from.column) <= (self.at.row, self.at.column) {
                    true => (from, self.at),
                    false => (self.at, from),
                };

            let columns: usize = self.size.columns;

            return (first.row..=last.row).map(move |row| {
                let start: usize = match row == first.row {
                    true => first.column,
                    false => 0,
                };
                let end: usize = match row == last.row {
                    true => (last.column + 1).min(columns),
                    false => columns,
                };

                (row, start..end)
            });
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
            copying.move_toward(Moving::Right);
            copying.mark();
            for moving in moves {
                copying.move_toward(*moving);
            }

            assert_eq!(copying.text(&fabric).as_deref(), expected);
        }

        #[test_case(Location { row: 1, column: 2 }, "gh"; "where the pane has its cursor")]
        #[test_case(Location { row: 9, column: 9 }, "l"; "a spot which is not in the pane, brought inside it")]
        fn test_of(at: Location, expected: &str) {
            let fabric = Fabric::from(vec!["abcd", "efgh", "ijkl"]);
            let mut copying = Copying::of(fabric.size(), at);

            copying.mark();
            copying.move_toward(Moving::End);

            assert_eq!(copying.text(&fabric).as_deref(), Some(expected));
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

            assert_eq!(copying.text(&fabric), None);
        }

        #[test]
        fn test_unmark_stops_anything_being_taken() {
            let fabric = Fabric::from(vec!["abcd", "efgh"]);
            let mut copying = Copying::of(fabric.size(), TOP_LEFT);

            copying.mark();
            copying.unmark();

            assert_eq!(copying.text(&fabric), None);
        }

        #[test_case(Moving::Up, 1; "up from the top row")]
        #[test_case(Moving::Down, 0; "down from the top row, which is a row to go to")]
        fn test_move_toward(moving: Moving, expected: isize) {
            let mut copying = Copying::of(Size::new(2, 4), TOP_LEFT);

            assert_eq!(copying.move_toward(moving), expected);
        }

        #[test]
        fn test_moving_down_from_the_last_row_looks_back_the_other_way() {
            let mut copying = Copying::of(Size::new(2, 4), TOP_LEFT);

            copying.move_toward(Moving::Down);

            assert_eq!(copying.move_toward(Moving::Down), -1);
        }

        #[test]
        fn test_resizing_keeps_the_spot_inside_the_pane() {
            let fabric = Fabric::from(vec!["abcd", "efgh", "ijkl"]);
            let mut copying = Copying::of(fabric.size(), TOP_LEFT);

            copying.move_toward(Moving::Down);
            copying.move_toward(Moving::End);
            copying.mark();
            copying.resize(Size::new(1, 2));

            // Both where it is and where it was begun are brought back inside the pane, so what
            // is taken is the one spot they have both come to rather than a run across rows which
            // are no longer there.
            assert_eq!(copying.text(&fabric).as_deref(), Some("b"));
        }
    }
}
pub use copying::Copying;

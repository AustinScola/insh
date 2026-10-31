//! The lines drawn between the panes of a window.

/// Contains the [`Borders`] struct.
mod borders {
    use std::collections::HashMap;

    use crate::color::Color;

    use insh_api::PaneLayout;
    use rend::{Fabric, Location, Size, Yarn};

    use bitflags::bitflags;
    use uuid::Uuid;

    bitflags! {
        /// Which of the four ways a line carries on from a spot.
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct Sides: u8 {
            /// The line carries on upwards.
            const UP = 0b0001;
            /// The line carries on downwards.
            const DOWN = 0b0010;
            /// The line carries on to the left.
            const LEFT = 0b0100;
            /// The line carries on to the right.
            const RIGHT = 0b1000;
        }
    }

    /// The lines drawn between the panes of a window.
    ///
    /// The lines are worked out a spot at a time rather than a pane at a time, because which
    /// character a spot is drawn as depends on which of its neighbours are lines too. A spot where
    /// two lines cross is drawn as a cross and one where a line runs into another is drawn as a
    /// tee, which is what makes the lines look like they were drawn together rather than one pane
    /// at a time.
    #[derive(Default)]
    pub struct Borders {
        /// Which spots of the window a line is drawn in.
        spots: HashMap<Location, Sides>,
        /// The spots which are drawn along the edge of the pane what is typed goes to.
        focused: Vec<Location>,
    }

    impl Borders {
        /// Return the lines drawn between the panes of a window of the given size.
        ///
        /// A line goes wherever the panes do not. The arrangement of a window leaves a row or a
        /// column between two parts of it and the panes fill in everything else, so whatever is
        /// left over is exactly the lines between them. Working it out that way rather than from
        /// the edges of each pane is what gets the spot where two lines cross, which is on the
        /// edge of no pane at all.
        pub fn of(panes: &[PaneLayout], focused: &Uuid, size: Size) -> Self {
            let mut borders = Self::default();

            let covers = |pane: &PaneLayout, row: usize, column: usize| {
                let (at, pane_size) = (pane.at(), pane.size());
                row >= at.row
                    && row < at.row + pane_size.rows
                    && column >= at.column
                    && column < at.column + pane_size.columns
            };

            // NOTE: Only the part of the window the panes reach across is looked at. A line goes
            // wherever they do not, and a spot where two lines cross has no pane against it at
            // all, so going by what is next to a pane would lose the crossings; going by the
            // whole window would carpet it in lines if the panes ever did not reach across all of
            // it, which is what a session drawn for a terminal of another size looks like.
            let rows: usize = panes
                .iter()
                .map(|pane| pane.at().row + pane.size().rows)
                .max()
                .unwrap_or(0)
                .min(size.rows);
            let columns: usize = panes
                .iter()
                .map(|pane| pane.at().column + pane.size().columns)
                .max()
                .unwrap_or(0)
                .min(size.columns);

            for row in 0..rows {
                for column in 0..columns {
                    if panes.iter().any(|pane| covers(pane, row, column)) {
                        continue;
                    }

                    let beside = |pane: &PaneLayout| {
                        (row > 0 && covers(pane, row - 1, column))
                            || covers(pane, row + 1, column)
                            || (column > 0 && covers(pane, row, column - 1))
                            || covers(pane, row, column + 1)
                    };

                    let at: Location = Location::builder().row(row).column(column).build();
                    borders.spots.insert(at, Sides::empty());

                    // A line is drawn in the color of the pane what is typed goes to when it runs
                    // along the edge of it, which is what says where what is typed is going.
                    if panes
                        .iter()
                        .any(|pane| pane.pane() == focused && beside(pane))
                    {
                        borders.focused.push(at);
                    }
                }
            }

            borders.join();

            borders
        }

        /// Work out which ways the line carries on from each spot.
        fn join(&mut self) {
            let spots: Vec<Location> = self.spots.keys().copied().collect();

            for at in spots {
                let mut sides = Sides::empty();

                if at.row > 0 && self.has(at.row - 1, at.column) {
                    sides.insert(Sides::UP);
                }
                if self.has(at.row + 1, at.column) {
                    sides.insert(Sides::DOWN);
                }
                if at.column > 0 && self.has(at.row, at.column - 1) {
                    sides.insert(Sides::LEFT);
                }
                if self.has(at.row, at.column + 1) {
                    sides.insert(Sides::RIGHT);
                }

                self.spots.insert(at, sides);
            }
        }

        /// Return whether a line is drawn in the spot.
        fn has(&self, row: usize, column: usize) -> bool {
            return self
                .spots
                .contains_key(&Location::builder().row(row).column(column).build());
        }

        /// Draw the lines on the fabric.
        pub fn draw(&self, fabric: &mut Fabric) {
            for (at, sides) in &self.spots {
                let focused: bool = self.focused.contains(at);
                let color: Color = match focused {
                    true => Color::FocusedBorder,
                    false => Color::Border,
                };

                let mut yarn = Yarn::from(Self::character(*sides));
                yarn.color(color.into());

                fabric.blit(&Fabric::from(yarn), *at);
            }
        }

        /// Return the character which a spot the line carries on from the given ways is drawn as.
        fn character(sides: Sides) -> &'static str {
            let up: bool = sides.contains(Sides::UP);
            let down: bool = sides.contains(Sides::DOWN);
            let left: bool = sides.contains(Sides::LEFT);
            let right: bool = sides.contains(Sides::RIGHT);

            return match (up, down, left, right) {
                (true, true, true, true) => "┼",
                (true, true, true, false) => "┤",
                (true, true, false, true) => "├",
                (true, false, true, true) => "┴",
                (false, true, true, true) => "┬",
                (true, false, true, false) => "┘",
                (true, false, false, true) => "└",
                (false, true, true, false) => "┐",
                (false, true, false, true) => "┌",
                // A line which carries on either way is the straight run of it, and one which
                // carries on neither way is a stub which is drawn as the run it is part of.
                (_, _, true, true) => "─",
                (true, true, _, _) => "│",
                (true, false, _, _) | (false, true, _, _) => "│",
                (false, false, _, _) => "─",
            };
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use insh_api::PaneKind;

        use test_case::test_case;

        /// Return the pane with the given number, so that a test says which one it means.
        fn pane(number: u128) -> Uuid {
            Uuid::from_u128(number)
        }

        /// Return a pane of the given number at the given spot and size.
        fn layout(
            number: u128,
            row: usize,
            column: usize,
            rows: usize,
            columns: usize,
        ) -> PaneLayout {
            PaneLayout::builder()
                .pane(pane(number))
                .at(Location::builder().row(row).column(column).build())
                .size(Size::new(rows, columns))
                .kind(PaneKind::Component)
                .build()
        }

        /// Return what the lines of the window look like, a row per line.
        fn drawn(panes: &[PaneLayout], size: Size) -> Vec<String> {
            let mut fabric = Fabric::new(size);
            Borders::of(panes, &pane(0), size).draw(&mut fabric);

            (0..size.rows)
                .map(|row| {
                    (0..size.columns)
                        .map(|column| fabric.spot(row, column).cell().to_string())
                        .collect()
                })
                .collect()
        }

        #[test]
        fn test_a_window_the_panes_do_not_reach_across_is_not_carpeted_in_lines() {
            // The panes only reach across the left half, which is what a session being drawn for
            // a terminal of another size looks like.
            let panes = vec![layout(1, 0, 0, 3, 2)];

            assert_eq!(
                drawn(&panes, Size::new(3, 9)),
                vec!["   ", "   ", "   "]
                    .into_iter()
                    .map(|row| format!("{:width$}", row.trim_end(), width = 9))
                    .collect::<Vec<String>>()
            );
        }

        #[test]
        fn test_one_pane_has_no_lines_around_it() {
            let panes = vec![layout(1, 0, 0, 3, 3)];

            assert_eq!(drawn(&panes, Size::new(3, 3)), vec!["   ", "   ", "   "]);
        }

        #[test]
        fn test_two_panes_side_by_side_have_a_line_down_between_them() {
            let panes = vec![layout(1, 0, 0, 3, 2), layout(2, 0, 3, 3, 2)];

            assert_eq!(
                drawn(&panes, Size::new(3, 5)),
                vec!["  │  ", "  │  ", "  │  "]
            );
        }

        #[test]
        fn test_two_panes_one_above_the_other_have_a_line_across_between_them() {
            let panes = vec![layout(1, 0, 0, 1, 3), layout(2, 2, 0, 1, 3)];

            assert_eq!(drawn(&panes, Size::new(3, 3)), vec!["   ", "───", "   "]);
        }

        #[test]
        fn test_a_line_running_into_another_is_drawn_as_a_tee() {
            // One pane on the left and two stacked on the right, so the line between the two runs
            // into the one down the middle.
            let panes = vec![
                layout(1, 0, 0, 3, 2),
                layout(2, 0, 3, 1, 2),
                layout(3, 2, 3, 1, 2),
            ];

            assert_eq!(
                drawn(&panes, Size::new(3, 5)),
                vec!["  │  ", "  ├──", "  │  "]
            );
        }

        #[test]
        fn test_two_lines_crossing_are_drawn_as_a_cross() {
            // Four panes in a square, so the lines between them cross in the middle.
            let panes = vec![
                layout(1, 0, 0, 1, 2),
                layout(2, 0, 3, 1, 2),
                layout(3, 2, 0, 1, 2),
                layout(4, 2, 3, 1, 2),
            ];

            assert_eq!(
                drawn(&panes, Size::new(3, 5)),
                vec!["  │  ", "──┼──", "  │  "]
            );
        }

        #[test_case(1, vec!["  │  ", "  ├──", "  │  "]; "the pane on the left")]
        #[test_case(2, vec!["  │  ", "  ├──", "  │  "]; "a pane on the right")]
        fn test_the_lines_are_the_same_whichever_pane_is_focused(
            focused: u128,
            expected: Vec<&str>,
        ) {
            let panes = vec![
                layout(1, 0, 0, 3, 2),
                layout(2, 0, 3, 1, 2),
                layout(3, 2, 3, 1, 2),
            ];
            let size = Size::new(3, 5);

            let mut fabric = Fabric::new(size);
            Borders::of(&panes, &pane(focused), size).draw(&mut fabric);

            let drawn: Vec<String> = (0..size.rows)
                .map(|row| {
                    (0..size.columns)
                        .map(|column| fabric.spot(row, column).cell().to_string())
                        .collect()
                })
                .collect();

            assert_eq!(drawn, expected);
        }

        #[test]
        fn test_the_edge_of_the_pane_which_is_focused_is_drawn_in_its_own_color() {
            let panes = vec![layout(1, 0, 0, 3, 2), layout(2, 0, 3, 3, 2)];
            let size = Size::new(3, 5);

            let mut fabric = Fabric::new(size);
            // The pane on the right is the one what is typed goes to, and the line is along its
            // left edge, which is the same line as the one along the left pane's right edge.
            Borders::of(&panes, &pane(2), size).draw(&mut fabric);

            assert_eq!(
                fabric.spot(0, 2).style().color(),
                Some(Color::FocusedBorder.into())
            );
        }
    }
}
pub use borders::Borders;

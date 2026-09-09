/*!
This module contains the [`Loom`] struct which works out what has to be sent to a terminal to draw
a fabric.
*/
use std::mem;
use std::ops::Range;

use super::{Engine, Frame, Instruction, Text};
use crate::fabric::Fabric;
use crate::row::Row;
use crate::spot::Spot;
use crate::style::Style;

use ansi::{ControlFunction, GraphicRendition};

use typed_builder::TypedBuilder;

/// Works out what has to be sent to a terminal to draw a fabric.
#[derive(TypedBuilder)]
pub struct Loom<'fabric> {
    /// The fabric which is being drawn.
    fabric: &'fabric Fabric,
    /// How much of the fabric is drawn.
    #[builder(default)]
    engine: Engine,
    /// The fabric which the terminal is showing, when what it is showing is known.
    ///
    /// The rows of it which scroll are moved on it as well as on the terminal, so that what is
    /// drawn afterwards is only what moving them did not put where it belongs.
    #[builder(default)]
    screen: Option<&'fabric mut Fabric>,
    /// The colors which the terminal is writing text in and on, when they are known.
    #[builder(default)]
    style: Option<Style>,
    /// What has been worked out so far.
    #[builder(setter(skip), default)]
    instructions: Vec<Instruction>,
}

impl<'fabric> Loom<'fabric> {
    /// The fewest rows worth scrolling rather than drawing again.
    ///
    /// Scrolling costs the sequence which sets the rows it happens between, the one which does it
    /// and the one which puts the rows back, which is about a dozen bytes, and a row of a screen
    /// is worth much more than that.
    const SCROLL_ROWS: usize = 2;

    /// Return the frame which draws the fabric.
    pub fn weave(&mut self) -> Frame {
        let mut runs: Vec<Vec<Range<usize>>> = self.all_runs();

        self.scroll(&mut runs);

        for (row, runs) in runs.into_iter().enumerate() {
            for run in runs {
                self.weave_run(row, run);
            }
        }

        return Frame::from(mem::take(&mut self.instructions));
    }

    /// Return the stretches of every row which have to be drawn.
    fn all_runs(&self) -> Vec<Vec<Range<usize>>> {
        return (0..self.fabric.size().rows)
            .map(|row| self.runs(row))
            .collect();
    }

    /// Return what the terminal was showing before the frame, when what it was showing is worth
    /// going on.
    ///
    /// Nothing is worth going on when all of the fabric is being drawn whatever the terminal is
    /// showing, and nor is a fabric of another size: that means the terminal was resized, and what
    /// it has done with what it was showing is not something which can be worked out.
    fn shown(&self) -> Option<&Fabric> {
        return match self.engine {
            Engine::Full => None,
            Engine::Incremental => self
                .screen
                .as_deref()
                .filter(|screen| screen.size() == self.fabric.size()),
        };
    }

    /// Return the colors which the terminal is left writing text in and on.
    pub fn style(&self) -> Option<Style> {
        return self.style;
    }

    /// Return the stretches of the row which have to be drawn, in the order they are drawn in.
    fn runs(&self, number: usize) -> Vec<Range<usize>> {
        let columns: usize = self.fabric.size().columns;

        let screen: &Fabric = match self.shown() {
            Some(screen) => screen,
            // Nothing is known about what the terminal is showing, so all of the row is drawn.
            None => {
                let mut all: Vec<Range<usize>> = Vec::new();
                if columns > 0 {
                    all.push(0..columns);
                }
                return all;
            }
        };
        let row: Row = self.fabric.row(number);
        let shown: Row = screen.row(number);

        let mut runs: Vec<Range<usize>> = Vec::new();
        let mut column: usize = 0;
        while column < columns {
            let cluster: Range<usize> = self.cluster(&row, column);

            if self.changed(&row, &shown, cluster.clone()) {
                match runs.last_mut() {
                    // The cluster carries straight on from the run before it.
                    Some(run) if run.end == cluster.start => run.end = cluster.end,
                    _ => runs.push(cluster.clone()),
                }
            }

            column = cluster.end;
        }

        return runs;
    }

    /// Return the columns which the cluster starting in the given column is written in, which are
    /// that column and the ones it continues into.
    fn cluster(&self, row: &Row, column: usize) -> Range<usize> {
        let columns: usize = self.fabric.size().columns;

        let mut end: usize = column + 1;
        while end < columns && row.spot(end).cell().is_continuation() {
            end += 1;
        }

        return column..end;
    }

    /// Return whether any column of the cluster is not what the terminal is showing there.
    fn changed(&self, row: &Row, shown: &Row, cluster: Range<usize>) -> bool {
        for column in cluster {
            let spot: Spot = row.spot(column);
            let shown: Spot = shown.spot(column);

            if spot.cell() != shown.cell() {
                return true;
            }

            // NOTE: Nothing is written for the second column of a cluster, so the colors it is
            // given are never sent and are not worth comparing. A yarn is colored a column at a
            // time, continuations along with the rest, so they differ whenever the columns around
            // them do.
            if !spot.cell().is_continuation() && spot.style() != shown.style() {
                return true;
            }
        }

        return false;
    }

    /// Move the rows of the screen which have scrolled rather than drawing all of them again.
    ///
    /// The rows are moved on the screen itself as well as on the terminal, and the stretches to be
    /// drawn are worked out again for the rows the move did not settle: the ones it brought in
    /// blank. The rest of the band was put where it belongs by the move, and nothing outside the
    /// band changed in the first place, so neither has anything left to draw.
    fn scroll(&mut self, runs: &mut [Vec<Range<usize>>]) {
        let band: Range<usize> = match Self::band(runs) {
            Some(band) => band,
            None => return,
        };
        let rows: isize = match self.shift(&band) {
            Some(rows) => rows,
            None => return,
        };

        // NOTE: A terminal which erases in the color it is writing on brings the rows which
        // scrolling makes room for in in that color, so the colors are put back first. The pass
        // which shortens the colors takes this out again when it is already writing in them,
        // which it is unless something else has written to the terminal.
        self.instructions
            .push(Instruction::from(ControlFunction::SelectGraphicRendition(
                vec![GraphicRendition::Reset],
            )));
        self.style = Some(Style::default());

        // The rows of a fabric are counted from zero and a terminal counts them from one.
        self.instructions
            .push(Instruction::from(ControlFunction::SetScrollingRegion {
                top: (band.start + 1).try_into().unwrap(),
                bottom: Some(band.end.try_into().unwrap()),
            }));
        self.instructions.push(Instruction::from(match rows > 0 {
            true => ControlFunction::ScrollUp(rows.unsigned_abs().try_into().unwrap()),
            false => ControlFunction::ScrollDown(rows.unsigned_abs().try_into().unwrap()),
        }));
        self.instructions
            .push(Instruction::from(ControlFunction::SetScrollingRegion {
                top: 1,
                bottom: None,
            }));

        let moved: usize = rows.unsigned_abs();
        let (settled, brought_in): (Range<usize>, Range<usize>) = match rows > 0 {
            // Moving the rows up brings the blank ones in at the bottom of the band.
            true => (band.start..(band.end - moved), (band.end - moved)..band.end),
            false => (
                (band.start + moved)..band.end,
                band.start..(band.start + moved),
            ),
        };

        if let Some(screen) = self.screen.as_deref_mut() {
            screen.scroll(band, rows);
        }

        for row in settled {
            runs[row].clear();
        }
        for row in brought_in {
            runs[row] = self.runs(row);
        }
    }

    /// Return the rows between the first one which changed and the last, which are the only ones
    /// which scrolling could account for.
    fn band(runs: &[Vec<Range<usize>>]) -> Option<Range<usize>> {
        let changed = |row: &usize| !runs[*row].is_empty();

        let first: usize = (0..runs.len()).find(changed)?;
        let last: usize = (0..runs.len()).rev().find(changed)?;

        return Some(first..last + 1);
    }

    /// Return how many rows the rows of the band moved up, or down when that is negative, which is
    /// nothing when they did not move or when moving them is not worth it.
    fn shift(&self, band: &Range<usize>) -> Option<isize> {
        let screen: &Fabric = self.shown()?;

        let rows: usize = band.end - band.start;
        if rows <= Self::SCROLL_ROWS {
            return None;
        }

        for moved in 1..(rows - Self::SCROLL_ROWS + 1) {
            // The row at the top of the band is the one which was that far below it.
            if self.same_row(band.start, band.start + moved, screen) {
                let shifted = (band.start..band.end - moved)
                    .all(|row| self.same_row(row, row + moved, screen));
                if shifted {
                    return Some(moved.try_into().unwrap());
                }
            }

            // The row at the bottom of the band is the one which was that far above it.
            if self.same_row(band.end - 1, band.end - 1 - moved, screen) {
                let shifted = (band.start + moved..band.end)
                    .all(|row| self.same_row(row, row - moved, screen));
                if shifted {
                    return Some(-isize::try_from(moved).unwrap());
                }
            }
        }

        return None;
    }

    /// Return whether the row of the fabric is written the same as the row of the screen.
    fn same_row(&self, number: usize, shown_number: usize, screen: &Fabric) -> bool {
        let columns: usize = self.fabric.size().columns;
        let row: Row = self.fabric.row(number);
        let shown: Row = screen.row(shown_number);

        return !self.changed(&row, &shown, 0..columns);
    }

    /// Draw the stretch of the row.
    fn weave_run(&mut self, row: usize, run: Range<usize>) {
        self.move_cursor(row, run.start);

        let woven: Row = self.fabric.row(row);

        let mut text = Text::default();
        text.reserve(run.end - run.start);

        for column in run {
            let spot: Spot = woven.spot(column);

            // NOTE: Nothing is written for the second column of a cluster and the colors it is
            // given are never sent, because writing the cluster covered that column as well as the
            // one before it. It still counts towards how far along the cursor ends up, which is
            // what pushing it onto the text keeps track of.
            if !spot.cell().is_continuation() {
                let style: Style = spot.style();
                if Some(style) != self.style {
                    self.print(&mut text);
                    self.restyle(style);
                }
            }

            text.push(spot.cell());
        }

        self.print(&mut text);
    }

    /// Move the cursor to the given row and column.
    fn move_cursor(&mut self, row: usize, column: usize) {
        // The rows and columns of a fabric are counted from zero and a terminal counts them from
        // one.
        self.instructions
            .push(Instruction::from(ControlFunction::CursorPosition {
                row: (row + 1).try_into().unwrap(),
                column: (column + 1).try_into().unwrap(),
            }));
    }

    /// Write what follows in and on the given colors.
    fn restyle(&mut self, style: Style) {
        let renditions = style.renditions_from(self.style);
        if !renditions.is_empty() {
            self.instructions
                .push(Instruction::from(ControlFunction::SelectGraphicRendition(
                    renditions,
                )));
        }

        self.style = Some(style);
    }

    /// Send the text which has piled up, if there is any.
    fn print(&mut self, text: &mut Text) {
        if text.is_empty() {
            return;
        }

        self.instructions.push(Instruction::from(mem::take(text)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::yarn::Yarn;

    use ansi::{Color, GraphicRendition};

    use test_case::test_case;

    /// The color used for testing.
    const COLOR: Color = Color::Red;

    /// The background color used for testing.
    const BACKGROUND: Color = Color::Blue;

    /// Return the instruction which moves the cursor to the given row and column, counted from
    /// zero the way a fabric counts them.
    fn move_to(row: u16, column: u16) -> Instruction {
        Instruction::from(ControlFunction::CursorPosition {
            row: row + 1,
            column: column + 1,
        })
    }

    /// Return the instruction which writes what follows in and on the given colors.
    fn restyle(renditions: Vec<GraphicRendition>) -> Instruction {
        Instruction::from(ControlFunction::SelectGraphicRendition(renditions))
    }

    /// Return the instruction which writes the text.
    fn print(string: &str) -> Instruction {
        Instruction::from(Text::from(string))
    }

    /// Return the yarn of the text with all of it colored.
    fn colored(string: &str, color: Color) -> Yarn {
        let mut yarn = Yarn::from(string);
        yarn.color(color);
        yarn
    }

    /// Return the yarn of the text with the color set from the position on.
    fn colored_after(string: &str, color: Color, position: usize) -> Yarn {
        let mut yarn = Yarn::from(string);
        yarn.color_after(color, position);
        yarn
    }

    /// Return the frame which draws the fabric on a terminal which is writing in the colors it
    /// uses when none have been picked.
    fn weave(fabric: Fabric) -> Frame {
        Loom::builder()
            .fabric(&fabric)
            .style(Some(Style::default()))
            .build()
            .weave()
    }

    #[test_case(Fabric::from(Yarn::from("abc")), vec![move_to(0, 0), print("abc")]; "text in the colors the terminal is already writing in")]
    #[test_case(Fabric::from(colored("abc", COLOR)), vec![move_to(0, 0), restyle(vec![GraphicRendition::Foreground(COLOR)]), print("abc")]; "the colors set once rather than for every character")]
    #[test_case(Fabric::from(colored_after("abcd", COLOR, 2)), vec![move_to(0, 0), print("ab"), restyle(vec![GraphicRendition::Foreground(COLOR)]), print("cd")]; "the colors set again only where they change")]
    #[test_case(Fabric::from(Yarn::from("a🦀b")), vec![move_to(0, 0), print("a🦀b")]; "nothing written for the second column of a wide cluster")]
    #[test_case(Fabric::from(vec![Yarn::from("ab"), Yarn::from("cd")]), vec![move_to(0, 0), print("ab"), move_to(1, 0), print("cd")]; "each row written after moving the cursor to it")]
    #[test_case(Fabric::default(), vec![]; "nothing at all for a fabric of no size")]
    fn test_weave(fabric: Fabric, expected_instructions: Vec<Instruction>) {
        assert_eq!(weave(fabric), Frame::from(expected_instructions));
    }

    #[test]
    fn test_both_colors_are_set_when_what_the_terminal_is_writing_in_is_not_known() {
        let fabric = Fabric::from(Yarn::from("abc"));

        let frame: Frame = Loom::builder().fabric(&fabric).build().weave();

        assert_eq!(
            frame,
            Frame::from(vec![
                move_to(0, 0),
                restyle(vec![
                    GraphicRendition::Foreground(Color::Default),
                    GraphicRendition::Background(Color::Default),
                ]),
                print("abc"),
            ])
        );
    }

    #[test]
    fn test_the_colors_are_left_where_the_frame_leaves_them() {
        let fabric = Fabric::from(colored("abc", COLOR));

        let mut loom: Loom = Loom::builder()
            .fabric(&fabric)
            .style(Some(Style::default()))
            .build();
        loom.weave();

        assert_eq!(
            loom.style(),
            Some(Style::builder().color(Some(COLOR)).build())
        );
    }

    /// Return the fabric of the rows.
    fn fabric(rows: Vec<&str>) -> Fabric {
        Fabric::from(rows.into_iter().map(Yarn::from).collect::<Vec<Yarn>>())
    }

    #[test_case(fabric(vec!["ab", "cd", "ef", "gh"]), fabric(vec!["cd", "ef", "gh", "ij"]), Some(1); "rows which moved up")]
    #[test_case(fabric(vec!["ab", "cd", "ef", "gh"]), fabric(vec!["zz", "ab", "cd", "ef"]), Some(-1); "rows which moved down")]
    #[test_case(fabric(vec!["ab", "cd", "ef", "gh"]), fabric(vec!["ef", "gh", "ij", "kl"]), Some(2); "rows which moved more than one row")]
    #[test_case(fabric(vec!["ab", "cd", "ef", "gh"]), fabric(vec!["gh", "ij", "kl", "mn"]), None; "too few rows left over to be worth moving them")]
    #[test_case(fabric(vec!["ab", "cd", "ef", "gh"]), fabric(vec!["zz", "yy", "xx", "ww"]), None; "rows which did not move")]
    #[test_case(fabric(vec!["ab", "cd"]), fabric(vec!["cd", "ef"]), None; "too few rows to be worth moving any of them")]
    fn test_shift(mut screen: Fabric, fabric: Fabric, expected_shift: Option<isize>) {
        let loom: Loom = Loom::builder()
            .fabric(&fabric)
            .screen(Some(&mut screen))
            .build();
        let band: Range<usize> = 0..fabric.size().rows;

        assert_eq!(loom.shift(&band), expected_shift);
    }

    #[test]
    fn test_the_rows_which_scrolled_are_moved_rather_than_drawn_again() {
        let mut screen = fabric(vec!["ab", "cd", "ef", "gh"]);
        let scrolled = fabric(vec!["cd", "ef", "gh", "ij"]);

        let frame: Frame = Loom::builder()
            .fabric(&scrolled)
            .screen(Some(&mut screen))
            .style(Some(Style::default()))
            .build()
            .weave();

        assert_eq!(
            frame,
            Frame::from(vec![
                restyle(vec![GraphicRendition::Reset]),
                Instruction::from(ControlFunction::SetScrollingRegion {
                    top: 1,
                    bottom: Some(4),
                }),
                Instruction::from(ControlFunction::ScrollUp(1)),
                Instruction::from(ControlFunction::SetScrollingRegion {
                    top: 1,
                    bottom: None,
                }),
                move_to(3, 0),
                print("ij"),
            ])
        );
    }

    #[test_case(Yarn::from("abc"), None, vec![0..3]; "all of a row when what the terminal is showing is not known")]
    #[test_case(Yarn::from("abc"), Some(Yarn::from("abc")), vec![]; "none of a row which did not change")]
    #[test_case(Yarn::from("abc"), Some(Yarn::from("abX")), vec![2..3]; "only the column which changed")]
    #[test_case(Yarn::from("abcd"), Some(Yarn::from("XYcd")), vec![0..2]; "columns which changed beside each other in the one run")]
    #[test_case(Yarn::from("abcd"), Some(Yarn::from("XbcY")), vec![0..1, 3..4]; "a run for each stretch which changed")]
    #[test_case(colored("abc", COLOR), Some(Yarn::from("abc")), vec![0..3]; "columns which are only written in another color")]
    #[test_case(Yarn::from("a🦀b"), Some(Yarn::from("a🦀b")), vec![]; "none of a row of a wide cluster which did not change")]
    #[test_case(Yarn::from("🦀b"), Some(Yarn::from("ccb")), vec![0..2]; "both columns of a wide cluster which changed")]
    #[test_case(Yarn::from("ab🦀"), Some(Yarn::from("ab🦀")), vec![]; "none of a row ending in a wide cluster which did not change")]
    #[test_case(colored("ab🦀", BACKGROUND), Some(colored("ab🦀", COLOR)), vec![0..4]; "the whole of a wide cluster which is only written in another color")]
    fn test_runs(yarn: Yarn, shown: Option<Yarn>, expected_runs: Vec<Range<usize>>) {
        let fabric = Fabric::from(yarn);
        let mut screen: Option<Fabric> = shown.map(Fabric::from);

        let loom: Loom = Loom::builder()
            .fabric(&fabric)
            .screen(screen.as_mut())
            .build();

        assert_eq!(loom.runs(0), expected_runs);
    }
}

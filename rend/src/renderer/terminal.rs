/*!
This module contains the [`Terminal`] struct, which is a terminal for frames to be drawn on so that
what they draw can be checked against what they were meant to draw.
*/
use std::ops::Range;

use super::{Frame, Instruction};
use crate::cell::Cell;
use crate::fabric::Fabric;
use crate::location::Location;
use crate::style::Style;

use ansi::{Color, ControlFunction, EraseInLine, GraphicRendition};
use size::Size;

/// A terminal for frames to be drawn on.
///
/// Drawing a frame on one and comparing what it is showing against the fabric the frame was woven
/// from is what says that the frame draws what it was meant to, which is not something which can
/// be told by reading the instructions of it one at a time.
pub struct Terminal {
    /// The size of the screen.
    size: Size,
    /// What each column of each row is showing.
    screen: Vec<Vec<Shown>>,
    /// Where the cursor is.
    cursor: Location,
    /// The rows which the text scrolls between.
    region: Range<usize>,
    /// The colors which text is being written in and on.
    style: Style,
}

impl Terminal {
    /// Draw the frame on the terminal.
    pub fn draw(&mut self, frame: &Frame) {
        for instruction in frame.instructions() {
            match instruction {
                Instruction::Function(function) => self.perform(function),
                Instruction::Text(text) => self.print(text.string()),
            }
        }
    }

    /// Return what each column of each row is showing.
    pub fn screen(&self) -> &Vec<Vec<Shown>> {
        return &self.screen;
    }

    /// Do what the control function says.
    fn perform(&mut self, function: &ControlFunction) {
        match function {
            ControlFunction::CursorPosition { row, column } => {
                // A terminal counts its rows and columns from one.
                self.cursor = Location::builder()
                    .row(usize::from(*row) - 1)
                    .column(usize::from(*column) - 1)
                    .build();
            }
            ControlFunction::SelectGraphicRendition(renditions) => {
                for rendition in renditions {
                    self.render(rendition);
                }
            }
            ControlFunction::CursorColumn(column) => {
                self.move_relative();
                self.cursor.column = usize::from(*column) - 1;
            }
            ControlFunction::CursorForward(columns) => {
                self.move_relative();
                self.cursor.column += usize::from(*columns);
            }
            ControlFunction::CursorBack(columns) => {
                self.move_relative();
                self.cursor.column -= usize::from(*columns);
            }
            ControlFunction::CursorDown(rows) => {
                self.move_relative();
                self.cursor.row += usize::from(*rows);
            }
            ControlFunction::CursorNextLine(rows) => {
                self.move_relative();
                self.cursor.row += usize::from(*rows);
                self.cursor.column = 0;
            }
            ControlFunction::SetScrollingRegion { top, bottom } => {
                self.region = (usize::from(*top) - 1)
                    ..bottom.map_or(self.size.rows, |bottom| usize::from(bottom));

                // NOTE: Setting the rows which the text scrolls between puts the cursor back to
                // the first column of the first row, which is a thing terminals do that the
                // standard says nothing about.
                self.cursor = Location::default();
            }
            ControlFunction::ScrollUp(rows) => self.scroll(isize::try_from(*rows).unwrap()),
            ControlFunction::ScrollDown(rows) => self.scroll(-isize::try_from(*rows).unwrap()),
            ControlFunction::EraseInLine(EraseInLine::ToEnd) => {
                // NOTE: Whether a terminal erases in the color it is writing on or in the one it
                // uses when none has been picked is a thing terminals disagree about. This one
                // erases in the color it is writing on, so that anything which erases while it is
                // writing on another color is caught.
                let erased = Shown {
                    cell: Cell::BLANK,
                    style: Style::builder().background(self.style.background()).build(),
                };
                for column in self.cursor.column..self.size.columns {
                    self.screen[self.cursor.row][column] = erased.clone();
                }
            }
            _ => panic!("The terminal does not know what to do with {:?}.", function),
        }
    }

    /// Move the rows which the text scrolls between the given number of rows up, or down when
    /// that is negative.
    ///
    /// The rows which are made room for are brought in in the color the terminal is writing on,
    /// which is what a terminal which erases in that color does, so that anything which scrolls
    /// while it is writing on another color is caught.
    fn scroll(&mut self, rows: isize) {
        let blank: Vec<Shown> = vec![
            Shown {
                cell: Cell::BLANK,
                style: Style::builder().background(self.style.background()).build(),
            };
            self.size.columns
        ];

        let region: Range<usize> = self.region.clone();
        let moved: usize = rows.unsigned_abs().min(region.end - region.start);

        match rows > 0 {
            true => {
                for row in region.start..(region.end - moved) {
                    self.screen.swap(row, row + moved);
                }
                for row in (region.end - moved)..region.end {
                    self.screen[row] = blank.clone();
                }
            }
            false => {
                for row in ((region.start + moved)..region.end).rev() {
                    self.screen.swap(row, row - moved);
                }
                for row in region.start..(region.start + moved) {
                    self.screen[row] = blank.clone();
                }
            }
        }
    }

    /// Check that the cursor is somewhere a move which says how far to go rather than where to go
    /// can be made from.
    ///
    /// Terminals do not agree about where the cursor is once something has been written into the
    /// last column of a row: some leave it in that column and wrap when the next character
    /// arrives, and some have wrapped already. Only saying where to go lands in the same place on
    /// both of them.
    fn move_relative(&self) {
        assert!(
            self.cursor.column < self.size.columns,
            "The frame moves the cursor by how far from the end of row {}.",
            self.cursor.row
        );
    }

    /// Write what follows in and on the colors the rendition says.
    fn render(&mut self, rendition: &GraphicRendition) {
        let color = |color: &Color| match color {
            Color::Default => None,
            color => Some(*color),
        };

        self.style = match rendition {
            GraphicRendition::Reset => Style::default(),
            GraphicRendition::Foreground(foreground) => Style::builder()
                .color(color(foreground))
                .background(self.style.background())
                .build(),
            GraphicRendition::Background(background) => Style::builder()
                .color(self.style.color())
                .background(color(background))
                .build(),
            _ => panic!("The terminal does not know how to write {:?}.", rendition),
        };
    }

    /// Write the text where the cursor is, moving it along.
    fn print(&mut self, string: &str) {
        for cell in Cell::all(string) {
            assert!(
                self.cursor.column < self.size.columns,
                "The frame writes past the last column of row {}.",
                self.cursor.row
            );

            self.screen[self.cursor.row][self.cursor.column] = Shown {
                cell,
                style: self.style,
            };
            self.cursor.column += 1;
        }
    }
}

impl From<Size> for Terminal {
    fn from(size: Size) -> Self {
        Terminal {
            size,
            screen: vec![vec![Shown::default(); size.columns]; size.rows],
            cursor: Location::default(),
            region: 0..size.rows,
            style: Style::default(),
        }
    }
}

/// What one column of a terminal is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// What is written there.
    cell: Cell,
    /// The colors it is written in and on.
    style: Style,
}

impl Shown {
    /// Return what a terminal showing the fabric would be showing.
    ///
    /// The second column of a cluster is shown in the colors of the cluster rather than in the
    /// ones the fabric gives it, because nothing is written for it and the colors it is given are
    /// never sent.
    pub fn all(fabric: &Fabric) -> Vec<Vec<Self>> {
        let size: Size = fabric.size();

        (0..size.rows)
            .map(|row| {
                let mut shown: Vec<Self> = Vec::with_capacity(size.columns);
                for column in 0..size.columns {
                    let spot = fabric.spot(row, column);
                    let style: Style = match spot.cell().is_continuation() && column > 0 {
                        true => shown[column - 1].style,
                        false => spot.style(),
                    };
                    shown.push(Shown {
                        cell: spot.cell().clone(),
                        style,
                    });
                }
                shown
            })
            .collect()
    }
}

impl Default for Shown {
    fn default() -> Self {
        Shown {
            cell: Cell::BLANK,
            style: Style::default(),
        }
    }
}

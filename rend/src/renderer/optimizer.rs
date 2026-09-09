/*!
This module contains the [`Optimizer`] struct which shortens a frame without changing what it
draws.
*/
use super::{Frame, Instruction, Text};
use crate::location::Location;
use crate::style::Style;

use ansi::{Color, ControlFunction, EraseInLine, GraphicRendition};
use size::Size;

use typed_builder::TypedBuilder;

/// Shortens a frame without changing what it draws.
///
/// Every pass leaves the frame drawing exactly what it drew before, so which of them are run and
/// in what order is only a question of how short the frame ends up.
#[derive(TypedBuilder)]
pub struct Optimizer {
    /// The size of the fabric which the frame draws.
    size: Size,
    /// The colors which the terminal is writing text in and on before the frame, when they are
    /// known.
    #[builder(default)]
    style: Option<Style>,
    /// Where the cursor is before the frame, when that is known.
    #[builder(default)]
    cursor: Option<Location>,
}

impl Optimizer {
    /// How many bytes the sequence which erases the rest of a row is sent as.
    ///
    /// Erasing is only worth it for more blanks than that, and for the same number of them the
    /// blanks are worth more, because writing them leaves the cursor somewhere known.
    const ERASE_BYTES: usize = 3;

    /// Return the frame shortened.
    pub fn optimize(&mut self, frame: Frame) -> Frame {
        let mut instructions: Vec<Instruction> = frame.into();

        instructions = self.merge_text(instructions);
        instructions = self.simplify_styles(instructions);
        instructions = self.erase_row_tails(instructions);
        instructions = self.shorten_moves(instructions);

        let mut cursor: Option<Location> = self.cursor;
        for instruction in &instructions {
            cursor = self.moved(cursor, instruction);
        }
        self.cursor = cursor;

        return Frame::from(instructions);
    }

    /// Return where the frame leaves the cursor.
    pub fn cursor(&self) -> Option<Location> {
        return self.cursor;
    }

    /// Join the runs of text which nothing separates into one.
    ///
    /// Nothing between two runs is what the passes which take instructions out leave behind, and
    /// what a run which was only broken in two so that the columns between them did not have to be
    /// moved over looks like.
    fn merge_text(&self, instructions: Vec<Instruction>) -> Vec<Instruction> {
        let mut merged: Vec<Instruction> = Vec::with_capacity(instructions.len());

        for instruction in instructions {
            match (merged.last_mut(), &instruction) {
                (Some(Instruction::Text(text)), Instruction::Text(next)) => text.append(next),
                _ => merged.push(instruction),
            }
        }

        return merged;
    }

    /// Say what the colors are in as few bytes as they can be said in, and stop saying it where it
    /// is already being written in them.
    fn simplify_styles(&self, instructions: Vec<Instruction>) -> Vec<Instruction> {
        let mut simplified: Vec<Instruction> = Vec::with_capacity(instructions.len());

        let mut style: Option<Style> = self.style;
        for instruction in instructions {
            let renditions: &Vec<GraphicRendition> = match &instruction {
                Instruction::Function(ControlFunction::SelectGraphicRendition(renditions)) => {
                    renditions
                }
                _ => {
                    simplified.push(instruction);
                    continue;
                }
            };

            let restyled: Option<Style> = Self::restyled(style, renditions);
            match restyled {
                // It is already writing in them, so there is nothing to say.
                Some(restyled) if Some(restyled) == style => continue,
                Some(restyled) => {
                    // NOTE: A reset is three bytes and putting even one color back on its own is
                    // five, and the renderer is the only thing which has written anything for
                    // there to be anything else for the reset to undo.
                    let renditions: Vec<GraphicRendition> = match restyled.is_default() {
                        true => vec![GraphicRendition::Reset],
                        false => restyled.renditions_from(style),
                    };
                    simplified.push(Instruction::from(ControlFunction::SelectGraphicRendition(
                        renditions,
                    )));
                }
                // What it would leave it writing in is not known, so it is left alone.
                None => simplified.push(instruction),
            }

            style = restyled;
        }

        return simplified;
    }

    /// Erase the blanks which a row ends with rather than writing them.
    ///
    /// Erasing only leaves the same thing behind as writing blanks does when the color the blanks
    /// would be written on is the one the terminal uses when none has been picked, because
    /// terminals do not agree about which color they erase in.
    fn erase_row_tails(&self, instructions: Vec<Instruction>) -> Vec<Instruction> {
        let mut erased: Vec<Instruction> = Vec::with_capacity(instructions.len());

        let mut style: Option<Style> = self.style;
        let mut cursor: Option<Location> = self.cursor;
        for instruction in instructions {
            let ends_the_row: Option<(&Text, Location)> = match (&instruction, cursor, style) {
                (Instruction::Text(text), Some(at), Some(style))
                    if style.background().is_none()
                        && at.column + text.columns() == self.size.columns =>
                {
                    Some((text, at))
                }
                _ => None,
            };
            let blanks: usize = match ends_the_row {
                Some((text, _)) => Self::trailing_blanks(text),
                None => 0,
            };

            if blanks <= Self::ERASE_BYTES {
                style = self.styled(style, &instruction);
                cursor = self.moved(cursor, &instruction);
                erased.push(instruction);
                continue;
            }

            let at: Location = ends_the_row.unwrap().1;
            let text: &Text = ends_the_row.unwrap().0;

            let written: &str = &text.string()[..text.string().len() - blanks];
            if !written.is_empty() {
                erased.push(Instruction::from(Text::from(written)));
            }
            erased.push(Instruction::from(ControlFunction::EraseInLine(
                EraseInLine::ToEnd,
            )));

            // Erasing leaves the cursor where the blanks it took the place of start rather than
            // past them, which is somewhere known even though the end of a row is not.
            cursor = Some(
                Location::builder()
                    .row(at.row)
                    .column(self.size.columns - blanks)
                    .build(),
            );
        }

        return erased;
    }

    /// Move the cursor in as few bytes as it can be moved in, and stop moving it where it is
    /// already where it is being moved to.
    fn shorten_moves(&self, instructions: Vec<Instruction>) -> Vec<Instruction> {
        let mut shortened: Vec<Instruction> = Vec::with_capacity(instructions.len());

        let mut cursor: Option<Location> = self.cursor;
        for instruction in instructions {
            let to: Location = match (&instruction, cursor) {
                (
                    Instruction::Function(ControlFunction::CursorPosition { row, column }),
                    Some(_),
                ) => Location::builder()
                    .row(usize::from(*row) - 1)
                    .column(usize::from(*column) - 1)
                    .build(),
                _ => {
                    cursor = self.moved(cursor, &instruction);
                    shortened.push(instruction);
                    continue;
                }
            };
            let at: Location = cursor.unwrap();

            cursor = Some(to);
            match Self::shortest_move(at, to) {
                Some(function) => shortened.push(Instruction::from(function)),
                // It is already there.
                None => continue,
            }
        }

        return shortened;
    }

    /// Return the shortest way of moving the cursor from where it is to where it is going, which
    /// is no way at all when it is already there.
    fn shortest_move(at: Location, to: Location) -> Option<ControlFunction> {
        if at == to {
            return None;
        }

        // The rows and columns of a fabric are counted from zero and a terminal counts them from
        // one.
        let mut ways: Vec<ControlFunction> = vec![ControlFunction::CursorPosition {
            row: (to.row + 1).try_into().unwrap(),
            column: (to.column + 1).try_into().unwrap(),
        }];

        if at.row == to.row {
            ways.push(ControlFunction::CursorColumn(
                (to.column + 1).try_into().unwrap(),
            ));
            ways.push(match to.column > at.column {
                true => ControlFunction::CursorForward((to.column - at.column).try_into().unwrap()),
                false => ControlFunction::CursorBack((at.column - to.column).try_into().unwrap()),
            });
        }
        if to.column == 0 && to.row > at.row {
            ways.push(ControlFunction::CursorNextLine(
                (to.row - at.row).try_into().unwrap(),
            ));
        }
        if to.column == at.column && to.row > at.row {
            ways.push(ControlFunction::CursorDown(
                (to.row - at.row).try_into().unwrap(),
            ));
        }

        return ways
            .into_iter()
            .min_by_key(|way| Instruction::from(way.clone()).bytes());
    }

    /// Return how many blanks the text ends with.
    fn trailing_blanks(text: &Text) -> usize {
        return text.string().len() - text.string().trim_end_matches(' ').len();
    }

    /// Return the colors which the renditions leave the terminal writing text in and on, which is
    /// not known when they say something about how it writes which is not a color.
    fn restyled(style: Option<Style>, renditions: &[GraphicRendition]) -> Option<Style> {
        let color = |color: &Color| match color {
            Color::Default => None,
            color => Some(*color),
        };

        let mut restyled: Option<Style> = style;
        for rendition in renditions {
            restyled = match rendition {
                GraphicRendition::Reset => Some(Style::default()),
                GraphicRendition::Foreground(foreground) => Some(
                    Style::builder()
                        .color(color(foreground))
                        .background(restyled?.background())
                        .build(),
                ),
                GraphicRendition::Background(background) => Some(
                    Style::builder()
                        .color(restyled?.color())
                        .background(color(background))
                        .build(),
                ),
                _ => None,
            };
        }

        return restyled;
    }

    /// Return the colors which the instruction leaves the terminal writing text in and on.
    fn styled(&self, style: Option<Style>, instruction: &Instruction) -> Option<Style> {
        return match instruction {
            Instruction::Function(ControlFunction::SelectGraphicRendition(renditions)) => {
                Self::restyled(style, renditions)
            }
            _ => style,
        };
    }

    /// Return where the instruction leaves the cursor.
    fn moved(&self, cursor: Option<Location>, instruction: &Instruction) -> Option<Location> {
        let at: Location = match instruction {
            // The rows and columns of a fabric are counted from zero and a terminal counts them
            // from one.
            Instruction::Function(ControlFunction::CursorPosition { row, column }) => {
                return Some(
                    Location::builder()
                        .row(usize::from(*row) - 1)
                        .column(usize::from(*column) - 1)
                        .build(),
                )
            }
            _ => cursor?,
        };

        let moved: Location = match instruction {
            Instruction::Function(function) => match function {
                ControlFunction::CursorColumn(column) => Location::builder()
                    .row(at.row)
                    .column(usize::from(*column) - 1)
                    .build(),
                ControlFunction::CursorForward(columns) => Location::builder()
                    .row(at.row)
                    .column(at.column + usize::from(*columns))
                    .build(),
                ControlFunction::CursorBack(columns) => Location::builder()
                    .row(at.row)
                    .column(at.column - usize::from(*columns))
                    .build(),
                ControlFunction::CursorDown(rows) => Location::builder()
                    .row(at.row + usize::from(*rows))
                    .column(at.column)
                    .build(),
                ControlFunction::CursorNextLine(rows) => Location::builder()
                    .row(at.row + usize::from(*rows))
                    .column(0)
                    .build(),
                // Neither the colors nor erasing the rest of a row move the cursor.
                ControlFunction::SelectGraphicRendition(_) => at,
                ControlFunction::EraseInLine(_) => at,
                _ => return None,
            },
            Instruction::Text(text) => Location::builder()
                .row(at.row)
                .column(at.column + text.columns())
                .build(),
        };

        // NOTE: Where the cursor is once text has been written into the last column of a row is
        // not something which can be told: a terminal either leaves it in that column and wraps
        // when the next character arrives or has wrapped already. Either way the next move has to
        // be one which says where to go rather than how far.
        if moved.column >= self.size.columns {
            return None;
        }

        return Some(moved);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    /// The color used for testing.
    const COLOR: Color = Color::Red;

    /// The background color used for testing.
    const BACKGROUND: Color = Color::Blue;

    /// How many columns wide the frames being shortened are.
    const COLUMNS: usize = 6;

    /// Return an optimizer for a frame drawn on a terminal which is writing in the given colors
    /// with the cursor in the given place.
    fn optimizer(style: Option<Style>, cursor: Option<Location>) -> Optimizer {
        Optimizer::builder()
            .size(Size::new(3, COLUMNS))
            .style(style)
            .cursor(cursor)
            .build()
    }

    /// Return the place with the given row and column.
    fn at(row: usize, column: usize) -> Location {
        Location::builder().row(row).column(column).build()
    }

    /// Return the style with the given colors.
    fn style(color: Option<Color>, background: Option<Color>) -> Style {
        Style::builder().color(color).background(background).build()
    }

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

    /// Return the instruction which erases the rest of the row.
    fn erase() -> Instruction {
        Instruction::from(ControlFunction::EraseInLine(EraseInLine::ToEnd))
    }

    #[test_case(vec![print("ab"), print("cd")], vec![print("abcd")]; "runs of text with nothing between them")]
    #[test_case(vec![print("a"), print("🦀"), print("b")], vec![print("a🦀b")]; "runs of text which are wider than they are long")]
    #[test_case(vec![print("ab"), move_to(0, 4), print("cd")], vec![print("ab"), move_to(0, 4), print("cd")]; "runs of text with the cursor moved between them")]
    #[test_case(vec![], vec![]; "nothing at all")]
    fn test_merge_text(instructions: Vec<Instruction>, expected: Vec<Instruction>) {
        assert_eq!(optimizer(None, None).merge_text(instructions), expected);
    }

    #[test_case(
        Some(Style::default()),
        vec![restyle(vec![GraphicRendition::Foreground(COLOR), GraphicRendition::Background(Color::Default)])],
        vec![restyle(vec![GraphicRendition::Foreground(COLOR)])];
        "only the color which changes"
    )]
    #[test_case(
        Some(Style::default()),
        vec![restyle(vec![GraphicRendition::Foreground(Color::Default), GraphicRendition::Background(Color::Default)])],
        vec![];
        "nothing where it is already writing in them"
    )]
    #[test_case(
        Some(style(Some(COLOR), Some(BACKGROUND))),
        vec![restyle(vec![GraphicRendition::Foreground(Color::Default), GraphicRendition::Background(Color::Default)])],
        vec![restyle(vec![GraphicRendition::Reset])];
        "a reset rather than both colors put back"
    )]
    #[test_case(
        Some(style(Some(COLOR), None)),
        vec![restyle(vec![GraphicRendition::Foreground(Color::Default)])],
        vec![restyle(vec![GraphicRendition::Reset])];
        "a reset rather than the one color put back"
    )]
    #[test_case(
        None,
        vec![restyle(vec![GraphicRendition::Foreground(COLOR)])],
        vec![restyle(vec![GraphicRendition::Foreground(COLOR)])];
        "as it was when what it is writing in is not known"
    )]
    fn test_simplify_styles(
        style: Option<Style>,
        instructions: Vec<Instruction>,
        expected: Vec<Instruction>,
    ) {
        assert_eq!(
            optimizer(style, None).simplify_styles(instructions),
            expected
        );
    }

    #[test_case(
        Some(Style::default()),
        vec![move_to(0, 0), print("ab    ")],
        vec![move_to(0, 0), print("ab"), erase()];
        "the blanks a row ends with"
    )]
    #[test_case(
        Some(Style::default()),
        vec![move_to(0, 0), print("      ")],
        vec![move_to(0, 0), erase()];
        "a row of nothing but blanks"
    )]
    #[test_case(
        Some(Style::default()),
        vec![move_to(0, 0), print("abc   ")],
        vec![move_to(0, 0), print("abc   ")];
        "as it was for no more blanks than erasing them costs"
    )]
    #[test_case(
        Some(Style::default()),
        vec![move_to(0, 0), print("ab    "), move_to(1, 0), print("cd    ")],
        vec![move_to(0, 0), print("ab"), erase(), move_to(1, 0), print("cd"), erase()];
        "the blanks every row ends with"
    )]
    #[test_case(
        Some(Style::default()),
        vec![move_to(0, 2), print("ab  ")],
        vec![move_to(0, 2), print("ab  ")];
        "as it was where the blanks do not reach the end of the row"
    )]
    #[test_case(
        Some(style(None, Some(BACKGROUND))),
        vec![move_to(0, 0), print("ab    ")],
        vec![move_to(0, 0), print("ab    ")];
        "as it was where the blanks are written on a color of their own"
    )]
    fn test_erase_row_tails(
        style: Option<Style>,
        instructions: Vec<Instruction>,
        expected: Vec<Instruction>,
    ) {
        assert_eq!(
            optimizer(style, None).erase_row_tails(instructions),
            expected
        );
    }

    #[test_case(
        Some(at(0, 0)),
        vec![move_to(0, 0), print("ab")],
        vec![print("ab")];
        "no move at all where the cursor already is"
    )]
    #[test_case(
        Some(at(0, 0)),
        vec![print("a"), move_to(0, 2)],
        vec![print("a"), Instruction::from(ControlFunction::CursorForward(1))];
        "how far along a row rather than where in it"
    )]
    #[test_case(
        Some(at(0, 0)),
        vec![print("ab"), move_to(1, 0)],
        vec![print("ab"), Instruction::from(ControlFunction::CursorNextLine(1))];
        "the start of the next row"
    )]
    #[test_case(
        None,
        vec![move_to(1, 3)],
        vec![move_to(1, 3)];
        "where to go when where the cursor is is not known"
    )]
    #[test_case(
        Some(at(0, 0)),
        vec![print("abcdef"), move_to(1, 0)],
        vec![print("abcdef"), move_to(1, 0)];
        "where to go from the end of a row"
    )]
    fn test_shorten_moves(
        cursor: Option<Location>,
        instructions: Vec<Instruction>,
        expected: Vec<Instruction>,
    ) {
        assert_eq!(
            optimizer(None, cursor).shorten_moves(instructions),
            expected
        );
    }
}

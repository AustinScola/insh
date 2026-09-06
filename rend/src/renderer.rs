/*!
This module contains the [`Renderer`] struct which is used for terminal rendering.
*/
use super::fabric::Fabric;

use std::io::{self, Stdout, Write};

use super::cell::Cell;

use ansi::{Color, ControlFunction, EraseInDisplay, GraphicRendition};

/// Renders [`Fabric`]s on a terminal.
///
/// The terminal is the standard output unless another one is given, which is what lets what a
/// renderer writes be read back and checked.
pub struct Renderer<Writer: Write = Stdout> {
    /// What is written to.
    writer: Writer,
    /// The colours which the terminal is writing text in and on, so that the sequence which sets
    /// them is only written when they actually change.
    style: Style,
}

impl Renderer<Stdout> {
    /// Return a new renderer which writes to the standard output.
    pub fn new() -> Self {
        Self::writing_to(io::stdout())
    }
}

impl<Writer: Write> Renderer<Writer> {
    /// Return a new renderer which writes to the given writer.
    pub fn writing_to(writer: Writer) -> Self {
        Renderer {
            writer,
            style: Style::default(),
        }
    }

    /// Return what is written to.
    pub fn writer(&self) -> &Writer {
        &self.writer
    }

    /// Render the fabric on the terminal.
    pub fn render(&mut self, fabric: Fabric) {
        // NOTE: Something else may have written to the terminal since the last render — a program
        // which took it over leaves it however it liked — so what it is writing in is put back to
        // the default rather than assumed. That is three bytes a frame for not having to trust it.
        self.lazy_control_function(&ControlFunction::SelectGraphicRendition(vec![
            GraphicRendition::Reset,
        ]));
        self.style = Style::default();

        let attributes =
            itertools::izip!(0.., fabric.cells(), fabric.colors(), fabric.backgrounds(),);

        for (row_number, row, row_colors, row_backgrounds) in attributes {
            self.lazy_move_cursor(row_number, 0);

            let mut cells_iter = row.iter();
            let mut row_colors_iter = row_colors.iter();
            let mut row_backgrounds_iter = row_backgrounds.iter();
            loop {
                let cell: Option<&Cell> = cells_iter.next();
                match cell {
                    Some(cell) => {
                        // NOTE: The style vectors are allowed to be shorter than the row, and a
                        // character which is off the end of them has no colour of its own.
                        let color: Option<Color> = row_colors_iter.next().copied().flatten();
                        let background: Option<Color> =
                            row_backgrounds_iter.next().copied().flatten();

                        // NOTE: A continuation is the second column of a cluster which is two wide,
                        // and writing that cluster covered both columns and moved the cursor past
                        // them, so there is nothing to write and no colours to set for it.
                        if cell.is_continuation() {
                            continue;
                        }

                        self.lazy_style(color, background);
                        self.lazy_print_cell(cell);
                    }
                    None => break,
                }
            }

            self.lazy_style(None, None);
        }

        self.update_terminal();
    }

    /// Queue the sequence which sets the colours to write the text in and on, but don't send it.
    ///
    /// Nothing is queued when the terminal is already writing in those colours, which is what most
    /// of the characters on a screen have in common with the one before them.
    fn lazy_style(&mut self, color: Option<Color>, background: Option<Color>) {
        let style = Style { color, background };
        if self.style == style {
            return;
        }

        // The two colours go in the one sequence, which is shorter than one sequence each.
        let mut renditions: Vec<GraphicRendition> = Vec::with_capacity(2);
        if self.style.color != style.color {
            renditions.push(GraphicRendition::Foreground(
                color.unwrap_or(Color::Default),
            ));
        }
        if self.style.background != style.background {
            renditions.push(GraphicRendition::Background(
                background.unwrap_or(Color::Default),
            ));
        }

        self.lazy_control_function(&ControlFunction::SelectGraphicRendition(renditions));
        self.style = style;
    }

    /// Queue the escape code to move the cursor to the given `row` and `column` but don't send it.
    fn lazy_move_cursor(&mut self, row: usize, column: usize) {
        // The rows and columns of a fabric are counted from zero and a terminal counts them from
        // one.
        self.lazy_control_function(&ControlFunction::CursorPosition {
            row: (row + 1).try_into().unwrap(),
            column: (column + 1).try_into().unwrap(),
        });
    }

    /// Queue the escape code to clear the screen of the terminal, but don't send it.
    #[allow(dead_code)]
    fn lazy_clear_screen(&mut self) {
        self.lazy_control_function(&ControlFunction::EraseInDisplay(EraseInDisplay::All));
    }

    /// Queue the escape code for the control function, but don't send it.
    fn lazy_control_function(&mut self, function: &ControlFunction) {
        let bytes: Vec<u8> = Vec::from(function);
        self.writer.write_all(&bytes).unwrap();
    }

    /// Queue what is written in the cell to be sent to the terminal, but don't send it.
    fn lazy_print_cell(&mut self, cell: &Cell) {
        write!(self.writer, "{}", cell).unwrap();
    }

    /// Queue the string to be sent the terminal, but don't send it.
    #[allow(dead_code)]
    fn lazy_print_string(&mut self, string: &str) {
        self.writer.write_all(string.as_bytes()).unwrap();
    }

    /// Update the terminal screen by flushing stdout.
    fn update_terminal(&mut self) {
        self.writer.flush().unwrap();
    }
}

impl Default for Renderer<Stdout> {
    fn default() -> Self {
        Self::new()
    }
}

/// The colours which text is being written in and on, where `None` is whichever colour the terminal
/// uses when none has been picked.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Style {
    /// The colour the text is written in.
    color: Option<Color>,
    /// The colour the text is written on.
    background: Option<Color>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::Yarn;

    /// Return what a renderer writes for the fabric.
    fn render(fabric: Fabric) -> String {
        let mut renderer: Renderer<Vec<u8>> = Renderer::writing_to(Vec::new());
        renderer.render(fabric);

        String::from_utf8(renderer.writer().clone()).unwrap()
    }

    /// Return a yarn of the given text with the colours set on all of it.
    fn styled(string: &str, color: Option<Color>, background: Option<Color>) -> Yarn {
        let mut yarn = Yarn::from(string);
        if let Some(color) = color {
            yarn.color(color);
        }
        if let Some(background) = background {
            yarn.background(background);
        }
        yarn
    }

    #[test]
    fn test_text_with_no_colours_is_written_without_any_styling() {
        let written = render(Fabric::from(Yarn::from("abc")));

        // The reset at the start, the cursor moved to the first row, and then just the text.
        assert_eq!(written, "\x1b[m\x1b[Habc");
    }

    #[test]
    fn test_the_colours_are_set_once_rather_than_for_every_character() {
        let yarn = styled("abc", Some(Color::Red), Some(Color::Blue));

        let written = render(Fabric::from(yarn));

        // Both colours in the one sequence before the text, and one to put them back after it.
        assert_eq!(written, "\x1b[m\x1b[H\x1b[31;44mabc\x1b[39;49m");
    }

    #[test]
    fn test_the_colours_are_only_set_again_where_they_change() {
        let mut yarn = Yarn::from("abcd");
        yarn.color_after(Color::Red, 2);

        let written = render(Fabric::from(yarn));

        assert_eq!(written, "\x1b[m\x1b[Hab\x1b[31mcd\x1b[39m");
    }

    #[test]
    fn test_nothing_is_written_for_the_second_column_of_a_wide_character() {
        let written = render(Fabric::from(Yarn::from("a🦀b")));

        // The crab is written once even though it takes up two of the four columns.
        assert_eq!(written, "\x1b[m\x1b[Ha🦀b");
        assert_eq!(written.matches('🦀').count(), 1);
    }

    #[test]
    fn test_each_row_is_written_after_moving_the_cursor_to_it() {
        let fabric = Fabric::from(vec![Yarn::from("ab"), Yarn::from("cd")]);

        let written = render(fabric);

        assert_eq!(written, "\x1b[m\x1b[Hab\x1b[2Hcd");
    }

    #[test]
    fn test_a_screen_of_plain_text_costs_little_more_than_the_text() {
        // NOTE: Setting the colours for every character rather than only where they change is what
        // this guards against: doing that costs ten bytes a column, which is twenty thousand bytes
        // a frame on a screen this size and is what made holding a key down fall behind.
        let rows = 24;
        let columns = 80;
        let fabric = Fabric::from(vec![Yarn::from("a".repeat(columns)); rows]);

        let written = render(fabric);

        let text = rows * columns;
        assert!(
            written.len() < text + (rows * 10) + 8,
            "{} bytes written for {} of text",
            written.len(),
            text
        );
    }
}

/*!
This module contains the [`Renderer`] struct which is used for terminal rendering.
*/
use super::fabric::Fabric;

use std::io::{self, Stdout, Write};

use crossterm::cursor::MoveTo as MoveCursorTo;
use crossterm::style::{Color, Print, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{Clear as ClearTerminal, ClearType as TerminalClearType};
use crossterm::QueueableCommand;

/// Renders [`Fabric`]s on the standard output.
pub struct Renderer {
    /// The standard output.
    stdout: Stdout,
}

impl Renderer {
    /// Return a new renderer.
    pub fn new() -> Self {
        let stdout = io::stdout();
        Renderer { stdout }
    }

    /// Render the fabric on the terminal.
    pub fn render(&mut self, fabric: Fabric) {
        let attributes = itertools::izip!(
            0..,
            fabric.characters(),
            fabric.colors(),
            fabric.backgrounds(),
        );

        for (row_number, row, row_colors, row_backgrounds) in attributes {
            self.lazy_move_cursor(row_number, 0);

            let mut characters_iter = row.iter();
            let mut row_colors_iter = row_colors.iter();
            let mut row_backgrounds_iter = row_backgrounds.iter();
            loop {
                let character: Option<&char> = characters_iter.next();
                match character {
                    Some(character) => {
                        let character_color: Option<&Option<Color>> = row_colors_iter.next();
                        let character_background: Option<&Option<Color>> =
                            row_backgrounds_iter.next();

                        match character_color {
                            Some(Some(color)) => self.lazy_start_text_color(*color),
                            _ => self.lazy_reset_text_color(),
                        }
                        match character_background {
                            Some(Some(color)) => self.lazy_start_background_color(*color),
                            _ => self.lazy_reset_background_color(),
                        }
                        self.lazy_print_character(character);
                    }
                    None => break,
                }
            }
            self.lazy_reset_text_color();
            self.lazy_reset_background_color();
        }

        self.update_terminal();
    }

    /// Queue the escape code to move the cursor to the given `row` and `column` but don't send it.
    fn lazy_move_cursor(&mut self, row: usize, column: usize) {
        self.stdout
            .queue(MoveCursorTo(
                column.try_into().unwrap(),
                row.try_into().unwrap(),
            ))
            .unwrap();
    }

    /// Queue the escape code to clear the screen of the terminal, but don't send it.
    #[allow(dead_code)]
    fn lazy_clear_screen(&mut self) {
        self.stdout
            .queue(ClearTerminal(TerminalClearType::All))
            .unwrap();
    }

    /// Queue the character to be sent to the terminal, but don't send it.
    fn lazy_print_character(&mut self, character: &char) {
        self.stdout.queue(Print(character)).unwrap();
    }

    /// Queue the string to be sent the terminal, but don't send it.
    #[allow(dead_code)]
    fn lazy_print_string(&mut self, string: &str) {
        self.stdout.queue(Print(string)).unwrap();
    }

    /// Queue the escape code to change the text color of the terminal from the position of the
    /// cursor onwards to the given `color`, but don't send it.
    fn lazy_start_text_color(&mut self, color: Color) {
        self.stdout.queue(SetForegroundColor(color)).unwrap();
    }

    /// Queue the escape code to reset the text color of the terminal, but don't send it.
    fn lazy_reset_text_color(&mut self) {
        self.stdout.queue(SetForegroundColor(Color::Reset)).unwrap();
    }

    /// Queue the escape code to change the background color of the terminal from the position of
    /// the cursor onwards to the given `color`, but don't send it.
    fn lazy_start_background_color(&mut self, color: Color) {
        self.stdout.queue(SetBackgroundColor(color)).unwrap();
    }

    /// Queue the escape code to reset the background color of the terminal, but don't send it.
    fn lazy_reset_background_color(&mut self) {
        self.stdout.queue(SetBackgroundColor(Color::Reset)).unwrap();
    }

    /// Update the terminal screen by flushing stdout.
    fn update_terminal(&mut self) {
        self.stdout.flush().unwrap();
    }
}

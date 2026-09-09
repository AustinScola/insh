/*!
This module contains the [`Renderer`] struct which is used for terminal rendering.
*/
use std::io::{Stdout, Write};

use super::{Engine, Frame, Instruction, Loom, Optimizer};
use crate::fabric::Fabric;
use crate::location::Location;
use crate::style::Style;

use ansi::{ControlFunction, GraphicRendition};

use typed_builder::TypedBuilder;

/// Renders [`Fabric`]s on a terminal.
///
/// The terminal is whatever is written to, which is what lets what a renderer sends be read back
/// and checked.
#[derive(TypedBuilder)]
pub struct Renderer<Writer: Write = Stdout> {
    /// What is written to.
    writer: Writer,
    /// How much of a fabric is drawn.
    #[builder(default)]
    engine: Engine,
    /// The fabric which was drawn last, which is what the terminal is showing, when what it is
    /// showing is known.
    #[builder(setter(skip), default)]
    screen: Option<Fabric>,
    /// The colors which the terminal is writing text in and on, when they are known.
    #[builder(setter(skip), default)]
    style: Option<Style>,
    /// Where the cursor is, when that is known.
    #[builder(setter(skip), default)]
    cursor: Option<Location>,
}

impl<Writer: Write> Renderer<Writer> {
    /// Return what is written to.
    pub fn writer(&self) -> &Writer {
        return &self.writer;
    }

    /// Draw the fabric on the terminal, and return what was sent to draw it.
    pub fn render(&mut self, fabric: Fabric) -> Frame {
        let mut loom: Loom = Loom::builder()
            .fabric(&fabric)
            .engine(self.engine)
            .screen(self.screen.as_mut())
            .style(self.style)
            .build();
        let woven: Frame = loom.weave();

        // NOTE: The optimizer is told what the terminal is writing in before the frame, which is
        // what the loom was told, rather than what the frame leaves it writing in.
        let mut optimizer: Optimizer = Optimizer::builder()
            .size(fabric.size())
            .style(self.style)
            .cursor(self.cursor)
            .build();
        let frame: Frame = optimizer.optimize(woven);

        self.style = loom.style();
        self.cursor = optimizer.cursor();

        self.send(&frame);
        self.screen = Some(fabric);

        return frame;
    }

    /// Put the terminal back to writing in the colors it uses when none have been picked, and
    /// forget what it is showing.
    ///
    /// This is for when something else has written to the terminal — a program which took it over
    /// leaves it however it liked — so that the next frame draws all of it rather than trusting
    /// what was there before.
    pub fn reset(&mut self) {
        let frame = Frame::from(vec![Instruction::from(
            ControlFunction::SelectGraphicRendition(vec![GraphicRendition::Reset]),
        )]);
        self.send(&frame);

        self.style = Some(Style::default());
        self.screen = None;
        self.cursor = None;
    }

    /// Forget what the terminal is showing and where the cursor is, keeping what it is writing in.
    ///
    /// This is for when the terminal has been resized. What it has done with what it was showing
    /// and where it has put the cursor are not things which can be worked out, but it goes on
    /// writing in the colors it was writing in.
    pub fn forget(&mut self) {
        self.screen = None;
        self.cursor = None;
    }

    /// Send the frame to the terminal.
    fn send(&mut self, frame: &Frame) {
        self.writer.write_all(&Vec::from(frame)).unwrap();
        self.writer.flush().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::renderer::terminal::{Shown, Terminal};
    use crate::renderer::text::Text;
    use crate::yarn::Yarn;

    use ansi::Color;
    use size::Size;

    use test_case::test_case;

    /// The size of the screen the fabrics are drawn on.
    fn size() -> Size {
        Size::new(3, 6)
    }

    /// Return the yarn of the text, colored from the position on when it is colored at all.
    fn yarn(string: &str, color: Option<Color>, background: Option<Color>) -> Yarn {
        let mut yarn = Yarn::from(string);
        if let Some(color) = color {
            yarn.color_after(color, 2);
        }
        if let Some(background) = background {
            yarn.background(background);
        }
        yarn
    }

    /// Return the fabrics which are drawn one after another, which between them cover a column
    /// changing, colors coming and going, and a wide cluster taking the place of narrow ones and
    /// giving it back.
    fn fabrics() -> Vec<Fabric> {
        let plain = |string: &str| yarn(string, None, None);

        vec![
            Fabric::from(vec![plain("abcdef"), plain("ghijkl"), plain("mnopqr")]),
            Fabric::from(vec![plain("abcdef"), plain("ghiXkl"), plain("mnopqr")]),
            Fabric::from(vec![
                yarn("abcdef", Some(Color::Red), None),
                plain("ghiXkl"),
                yarn("mnopqr", None, Some(Color::Blue)),
            ]),
            Fabric::from(vec![
                yarn("abcdef", Some(Color::Red), None),
                plain("a🦀bcd"),
                yarn("mnopqr", None, Some(Color::Blue)),
            ]),
            Fabric::from(vec![plain("abcdef"), plain("ghijkl"), plain("mnopqr")]),
            Fabric::from(vec![
                Yarn::blank(6),
                Yarn::blank(6),
                yarn("mnopqr", Some(Color::Green), Some(Color::Blue)),
            ]),
            Fabric::from(vec![plain("abcdef"), plain("ghijkl"), plain("mnopqr")]),
            // The rows above moved up one, which is what scrolling a list looks like.
            Fabric::from(vec![plain("ghijkl"), plain("mnopqr"), plain("stuvwx")]),
            // And back down again.
            Fabric::from(vec![plain("abcdef"), plain("ghijkl"), plain("mnopqr")]),
        ]
    }

    #[test_case(Engine::Full; "drawing all of every fabric")]
    #[test_case(Engine::Incremental; "drawing only what changed")]
    fn test_render(engine: Engine) {
        let mut renderer: Renderer<Vec<u8>> = Renderer::builder()
            .writer(Vec::new())
            .engine(engine)
            .build();
        let mut terminal = Terminal::from(size());

        for fabric in fabrics() {
            let frame: Frame = renderer.render(fabric.clone());

            terminal.draw(&frame);

            assert_eq!(terminal.screen(), &Shown::all(&fabric));
        }
    }

    #[test]
    fn test_nothing_is_drawn_for_a_fabric_which_is_already_on_the_screen() {
        let mut renderer: Renderer<Vec<u8>> = Renderer::builder().writer(Vec::new()).build();
        let fabric = Fabric::from(Yarn::from("abc"));
        renderer.render(fabric.clone());

        let frame: Frame = renderer.render(fabric);

        assert_eq!(frame, Frame::default());
    }

    /// How many rows the screen which the cost of a frame is measured on has.
    const ROWS: usize = 24;

    /// How many columns of each of those rows it has.
    const COLUMNS: usize = 80;

    /// Return how many bytes a frame costs once the screen is already showing one of the fabrics,
    /// which is what it costs for as long as the app runs.
    fn cost(engine: Engine, fabrics: &[Fabric]) -> usize {
        let mut renderer: Renderer<Vec<u8>> = Renderer::builder()
            .writer(Vec::new())
            .engine(engine)
            .build();

        for fabric in fabrics.iter().cloned() {
            renderer.render(fabric);
        }
        let drawn: usize = renderer.writer().len();

        for fabric in fabrics.iter().cloned() {
            renderer.render(fabric);
        }

        (renderer.writer().len() - drawn) / fabrics.len()
    }

    // NOTE: Setting the colors for every character rather than only where they change is what the
    // budget for drawing all of a screen guards against: doing that costs ten bytes a column,
    // which is twenty thousand bytes a frame on a screen this size and is what made holding a key
    // down fall behind. The budget for drawing only what changed guards the whole point of it.
    #[test_case(Engine::Full, (ROWS * COLUMNS) + (ROWS * 10); "drawing all of a screen costs little more than the text on it")]
    #[test_case(Engine::Incremental, 16; "drawing the one column which changed costs almost nothing")]
    fn test_what_a_frame_costs(engine: Engine, budget: usize) {
        let mut rows: Vec<String> = vec!["ab".repeat(COLUMNS / 2); ROWS];
        let screen = Fabric::from(rows.iter().map(String::as_str).collect::<Vec<&str>>());
        rows[ROWS / 2].replace_range(0..1, "X");
        let changed = Fabric::from(rows.iter().map(String::as_str).collect::<Vec<&str>>());

        let bytes: usize = cost(engine, &[screen, changed]);

        assert!(bytes <= budget, "{} bytes a frame, not {}", bytes, budget);
    }

    #[test]
    fn test_reset() {
        let mut renderer: Renderer<Vec<u8>> = Renderer::builder().writer(Vec::new()).build();
        let fabric = Fabric::from(Yarn::from("abc"));
        renderer.render(fabric.clone());

        renderer.reset();
        let frame: Frame = renderer.render(fabric);

        // The sequence which puts the colors back is sent, and then all of the fabric is drawn
        // again even though none of it changed.
        assert_eq!(renderer.writer().ends_with(b"\x1b[m\x1b[Habc"), true);
        assert_eq!(
            frame,
            Frame::from(vec![
                Instruction::from(ControlFunction::CursorPosition { row: 1, column: 1 }),
                Instruction::from(Text::from("abc")),
            ])
        );
    }
}

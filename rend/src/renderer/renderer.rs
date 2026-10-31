/*!
This module contains the [`Renderer`] struct which is used for terminal rendering.
*/
use std::io::{Stdout, Write};

use super::{Engine, Frame, Instruction, Loom, Optimizer};
use crate::fabric::Fabric;
use crate::location::Location;
use crate::style::Style;

use ansi::{ControlFunction, GraphicRendition, Mode};

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
    /// Whether the terminal is showing the cursor, when that is known.
    #[builder(setter(skip), default)]
    shown: Option<bool>,
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
        // NOTE: What the loom leaves the terminal writing in is taken before anything else, so
        // that it is done borrowing the screen by the time the cursor is put on the frame.
        let style: Option<Style> = loom.style();

        let instructions: Vec<Instruction> = self.with_cursor(&fabric, woven.into());

        // NOTE: The optimizer is told what the terminal is writing in before the frame, which is
        // what the loom was told, rather than what the frame leaves it writing in.
        let mut optimizer: Optimizer = Optimizer::builder()
            .size(fabric.size())
            .style(self.style)
            .cursor(self.cursor)
            .build();
        let frame: Frame = optimizer.optimize(Frame::from(instructions));

        self.style = style;
        self.cursor = optimizer.cursor();

        self.send(&frame);
        self.screen = Some(fabric);

        return frame;
    }

    /// Return the instructions along with the ones which put the cursor where the fabric shows it.
    ///
    /// The cursor is put away while the frame is drawn so that it is not seen going about the
    /// screen on the way, and only when there is something being drawn for it to be seen during.
    /// Whether it is shown is only said where that changes.
    fn with_cursor(
        &mut self,
        fabric: &Fabric,
        mut instructions: Vec<Instruction>,
    ) -> Vec<Instruction> {
        let shown: bool = fabric.cursor().is_some();

        let hide: bool = !instructions.is_empty() && self.shown != Some(false);
        if hide {
            instructions.insert(0, Self::cursor_shown(false));
        }

        // The rows and columns of a fabric are counted from zero and a terminal counts them from
        // one.
        if let Some(at) = fabric.cursor() {
            instructions.push(Instruction::from(ControlFunction::CursorPosition {
                row: (at.row + 1).try_into().unwrap(),
                column: (at.column + 1).try_into().unwrap(),
            }));
        }

        let after: Option<bool> = match hide {
            true => Some(false),
            false => self.shown,
        };
        if after != Some(shown) {
            instructions.push(Self::cursor_shown(shown));
        }
        self.shown = Some(shown);

        return instructions;
    }

    /// Return the instruction which shows the cursor or puts it away.
    fn cursor_shown(shown: bool) -> Instruction {
        return Instruction::from(ControlFunction::SetMode {
            modes: vec![Mode::CursorVisible],
            set: shown,
        });
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
        self.shown = None;
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
    use crate::style::Attributes;
    use crate::yarn::Yarn;

    use ansi::{Color, Underline};
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

    /// Return the yarn of the text written the given way from the position on.
    fn styled(string: &str, style: Style, position: usize) -> Yarn {
        let mut yarn = Yarn::from(string);
        let len: usize = yarn.len();
        yarn.style(style, position..len);
        yarn
    }

    /// Return the style with the given attributes on.
    fn attributed(attributes: Attributes) -> Style {
        Style::builder().attributes(attributes).build()
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
            // Attributes coming on partway along a row, and an underline along with a color.
            Fabric::from(vec![
                styled("abcdef", attributed(Attributes::BOLD), 2),
                styled(
                    "ghijkl",
                    attributed(Attributes::REVERSE | Attributes::STRIKE),
                    0,
                ),
                styled(
                    "mnopqr",
                    Style::builder()
                        .underline(Underline::Curly)
                        .color(Some(Color::Red))
                        .build(),
                    3,
                ),
            ]),
            // Bold going off while faint stays on, which is the one pair the same rendition turns
            // off, and an underline going off on its own.
            Fabric::from(vec![
                styled(
                    "abcdef",
                    attributed(Attributes::BOLD | Attributes::FAINT),
                    2,
                ),
                styled("ghijkl", attributed(Attributes::FAINT), 2),
                styled("mnopqr", Style::default(), 3),
            ]),
            // And everything back off again.
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
    fn test_a_row_which_goes_blank_is_blanked_on_the_screen() {
        // A screen which is mostly blank is what a list drawn over everything else looks like, and
        // the one row of it which had something on it has to be cleared when it no longer does.
        let centered = |text: &str| {
            let mut yarns: Vec<Yarn> = vec![Yarn::blank(COLUMNS); ROWS];
            yarns[ROWS / 2] = Yarn::center(text, COLUMNS);
            Fabric::from(yarns)
        };
        let listed = || {
            let mut yarns: Vec<Yarn> = vec![Yarn::blank(COLUMNS); ROWS];
            yarns[0] = {
                let mut yarn = Yarn::from("a session");
                yarn.resize(COLUMNS);
                yarn
            };
            Fabric::from(yarns)
        };

        let mut renderer: Renderer<Vec<u8>> = Renderer::builder().writer(Vec::new()).build();
        let mut terminal = Terminal::from(Size::new(ROWS, COLUMNS));

        terminal.draw(&renderer.render(centered("No sessions.")));
        terminal.draw(&renderer.render(listed()));

        assert_eq!(terminal.screen(), &Shown::all(&listed()));
    }

    #[test]
    fn test_reset() {
        let mut renderer: Renderer<Vec<u8>> = Renderer::builder().writer(Vec::new()).build();
        let fabric = Fabric::from(Yarn::from("abc"));
        renderer.render(fabric.clone());

        renderer.reset();
        let frame: Frame = renderer.render(fabric);

        // The sequence which puts the colors back is sent, and then all of the fabric is drawn
        // again even though none of it changed. The cursor is put away again along with it,
        // because a reset is for when something else has had the terminal and whether it left the
        // cursor showing is not something which can be worked out.
        assert_eq!(
            renderer.writer().ends_with(b"\x1b[m\x1b[?25l\x1b[Habc"),
            true
        );
        assert_eq!(
            frame,
            Frame::from(vec![
                Instruction::from(ControlFunction::SetMode {
                    modes: vec![Mode::CursorVisible],
                    set: false,
                }),
                Instruction::from(ControlFunction::CursorPosition { row: 1, column: 1 }),
                Instruction::from(Text::from("abc")),
            ])
        );
    }

    /// Return the fabric of the text with the cursor shown in the given column.
    fn with_cursor(string: &str, column: Option<usize>) -> Fabric {
        let mut fabric = Fabric::from(Yarn::from(string));
        fabric.set_cursor(column.map(|column| Location::builder().row(0).column(column).build()));
        fabric
    }

    #[test]
    fn test_the_cursor_ends_up_where_the_fabric_shows_it() {
        let mut renderer: Renderer<Vec<u8>> = Renderer::builder().writer(Vec::new()).build();
        let mut terminal = Terminal::from(Size::new(1, 3));

        // It starts out being put away, since nothing is showing one.
        terminal.draw(&renderer.render(with_cursor("abc", None)));
        assert_eq!(terminal.cursor(), None);

        // Then it is shown where the fabric says.
        terminal.draw(&renderer.render(with_cursor("abd", Some(1))));
        assert_eq!(
            terminal.cursor(),
            Some(Location::builder().row(0).column(1).build())
        );

        // And moves along with it.
        terminal.draw(&renderer.render(with_cursor("abe", Some(2))));
        assert_eq!(
            terminal.cursor(),
            Some(Location::builder().row(0).column(2).build())
        );

        // And is put away again when nothing is showing one.
        terminal.draw(&renderer.render(with_cursor("abf", None)));
        assert_eq!(terminal.cursor(), None);
    }

    #[test]
    fn test_nothing_is_said_about_the_cursor_when_nothing_about_it_changed() {
        let mut renderer: Renderer<Vec<u8>> = Renderer::builder().writer(Vec::new()).build();
        let fabric = with_cursor("abc", Some(1));
        renderer.render(fabric.clone());

        let frame: Frame = renderer.render(fabric);

        assert_eq!(frame, Frame::default());
    }
}

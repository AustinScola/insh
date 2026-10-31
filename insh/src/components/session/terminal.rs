//! Contains the [`Terminal`] component, which shows the screen of a program running in the daemon.

/// Contains the [`Props`] struct.
mod props {
    use insh_api::PaneLayout;

    use typed_builder::TypedBuilder;

    /// The properties of a terminal.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// Where it is and how big it is.
        pub layout: PaneLayout,
    }
}
pub use props::Props;

/// Contains the [`Terminal`] component.
mod terminal {
    use super::{Props, State};

    use insh_api::{PaneLayout, PaneScreen};
    use rend::{Fabric, Location, Size, Yarn};
    use terminal_emulator::Modes;
    use til::Component;

    /// Shows the screen of a program which is running in the daemon.
    ///
    /// Nothing is worked out here. The daemon says what is on the screen, where the cursor is and
    /// how the program has asked its terminal to behave; this only draws it and says how what is
    /// typed should be sent.
    pub struct Terminal {
        /// The state of the terminal.
        state: State,
    }

    impl Component<Props, PaneScreen, ()> for Terminal {
        fn new(props: Props) -> Self {
            Self {
                state: State::from(props),
            }
        }

        fn handle(&mut self, screen: PaneScreen) -> Option<()> {
            self.state.apply(&screen);
            None
        }

        /// Draw what is on the screen, with the cursor wherever the program has it.
        ///
        /// The size which is drawn is the one the daemon gave the pane rather than whatever is
        /// asked for here, since the program has been told that is how big it is and has written
        /// for that. Anything else would be drawing it somewhere it does not fit.
        fn render(&self, _size: Size) -> Fabric {
            let yarns: Vec<Yarn> = self.state.lines().iter().map(Yarn::from).collect();

            let mut fabric: Fabric = match yarns.is_empty() {
                true => Fabric::new(self.state.size()),
                false => Fabric::from(yarns),
            };
            fabric.set_cursor(self.state.cursor());

            fabric
        }
    }

    impl Terminal {
        /// Take note of where it is and how big it is now.
        pub fn relayout(&mut self, layout: PaneLayout) {
            self.state.relayout(layout);
        }

        /// Return the ways the program has asked its terminal to behave.
        pub fn modes(&self) -> Modes {
            self.state.modes()
        }

        /// Return where the cursor is in the pane, which is nowhere at all when the program is
        /// not showing one.
        pub fn cursor(&self) -> Option<Location> {
            self.state.cursor()
        }
    }
}
pub use terminal::Terminal;

/// Contains the [`State`] struct.
mod state {
    use super::Props;

    use insh_api::{PaneLayout, PaneScreen};
    use rend::{Location, Size};
    use terminal_emulator::{Line, Modes};

    /// The state of a pane.
    pub struct State {
        /// Where it is and how big it is.
        layout: PaneLayout,
        /// What is written in each row of it.
        lines: Vec<Line>,
        /// Where the cursor is, which is nowhere at all when it is not shown.
        cursor: Option<Location>,
        /// The ways the program has asked its terminal to behave.
        modes: Modes,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                layout: props.layout,
                lines: Vec::new(),
                cursor: None,
                modes: Modes::default(),
            }
        }
    }

    impl State {
        /// Return how big it is.
        pub fn size(&self) -> Size {
            self.layout.size()
        }

        /// Return what is written in each row of it.
        pub fn lines(&self) -> &[Line] {
            &self.lines
        }

        /// Return where the cursor is, which is nowhere at all when it is not shown.
        pub fn cursor(&self) -> Option<Location> {
            self.cursor
        }

        /// Return the ways the program has asked its terminal to behave.
        pub fn modes(&self) -> Modes {
            self.modes
        }

        /// Take note of where it is and how big it is now.
        ///
        /// The rows are cut down or filled out to match, so that what was on the screen is drawn
        /// in the right place until the daemon sends the rows it has been written into.
        pub fn relayout(&mut self, layout: PaneLayout) {
            let rows: usize = layout.size().rows;
            let columns: usize = layout.size().columns;

            self.lines
                .resize(rows, Line::blank(columns, Default::default()));
            self.layout = layout;
        }

        /// Take note of what has changed about the screen.
        pub fn apply(&mut self, screen: &PaneScreen) {
            let rows: usize = self.layout.size().rows;
            let columns: usize = self.layout.size().columns;
            if self.lines.len() != rows {
                self.lines
                    .resize(rows, Line::blank(columns, Default::default()));
            }

            for row in screen.rows() {
                if let Some(line) = self.lines.get_mut(row.row()) {
                    *line = row.line().clone();
                }
            }

            self.cursor = screen.cursor();
            self.modes = screen.modes();
        }
    }
}
use state::State;

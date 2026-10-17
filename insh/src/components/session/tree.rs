//! Contains the [`Tree`] component, which lists the sessions and the windows in them.

/// Contains the [`Effect`] enum.
mod effect {
    use uuid::Uuid;

    /// Something which the tree asks for.
    pub enum Effect {
        /// Show the given window of the given session.
        Show {
            /// The session to show.
            session: Uuid,
            /// The window of it to look at.
            window: Uuid,
            /// The pane of that window to send what is typed to, which is whichever one it
            /// already goes to when there is none.
            pane: Option<Uuid>,
        },
        /// Stop showing the tree.
        Close,
    }
}
pub use effect::Effect;

/// Contains the [`Tree`] component.
mod tree {
    use super::{Effect, Row};
    use crate::color::Color;

    use insh_api::SessionSummary;
    use rend::{Fabric, Size, Yarn};
    use term::{Key, KeyEvent, KeyMods, TermEvent};

    use uuid::Uuid;

    /// Lists the sessions there are and the windows in them.
    ///
    /// Only the names are listed. What is in a window is not shown alongside it the way tmux
    /// shows it, because a pane is the daemon's to draw and asking for every one of them in order
    /// to show a corner of each would be asking for a great deal to show very little.
    #[derive(Default)]
    pub struct Tree {
        /// The sessions there are.
        sessions: Vec<SessionSummary>,
        /// The sessions whose windows are not shown.
        shut: Vec<Uuid>,
        /// The windows whose panes are shown.
        ///
        /// NOTE: A session shows its windows until it is asked not to and a window shows its
        /// panes only once it is asked to, so one of these says which are shut and the other
        /// which are open.
        opened: Vec<Uuid>,
        /// The sessions and the windows in them, a row each.
        rows: Vec<Row>,
        /// Which row is selected.
        selected: usize,
    }

    impl Tree {
        /// Take note of the sessions there are.
        ///
        /// Whichever row was selected stays selected if it is still there, so that the list being
        /// sent again does not move the selection out from under whoever is looking at it.
        pub fn show(&mut self, sessions: &[SessionSummary]) {
            let was: Option<Row> = self.rows.get(self.selected).cloned();

            self.sessions = sessions.to_vec();
            self.lay_out();

            self.selected = was
                .and_then(|was| self.rows.iter().position(|row| row.is(&was)))
                .unwrap_or_else(|| self.showing());
        }

        /// Work out the rows again from the sessions and which of them are shut.
        fn lay_out(&mut self) {
            self.rows = Row::of(&self.sessions, &self.shut, &self.opened);
            self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        }

        /// Return the row of the window which is being looked at in the session being shown, so
        /// that the list opens on where you already are.
        fn showing(&self) -> usize {
            return self.rows.iter().position(Row::here).unwrap_or_default();
        }

        /// Do what the key says, which is either moving about the list or picking from it.
        pub fn handle(&mut self, term_event: TermEvent) -> Option<Effect> {
            let key_event: KeyEvent = match term_event {
                TermEvent::KeyEvent(key_event) => key_event,
                _ => return None,
            };

            match (key_event.key, key_event.mods) {
                (Key::Char('j') | Key::Down, _) => self.move_by(1),
                (Key::Char('k') | Key::Up, _) => self.move_by(-1),
                (Key::Char('l') | Key::Right, _) => return self.open(),
                (Key::Char('h') | Key::Left, _) => self.shut(),
                (Key::CarriageReturn, _) => return self.pick(),
                (Key::Char('q') | Key::Escape, _) => return Some(Effect::Close),
                (Key::Char('c'), KeyMods::CONTROL) => return Some(Effect::Close),
                _ => {}
            }

            None
        }

        /// Move the selection along by the given number of rows.
        fn move_by(&mut self, by: isize) {
            let at: isize = self.selected as isize + by;

            if at >= 0 && (at as usize) < self.rows.len() {
                self.selected = at as usize;
            }
        }

        /// Show what is in whatever is selected, or go into it when it is already showing.
        ///
        /// Going right over and over walks down into a session, then into one of its windows, and
        /// then shows the pane which is reached.
        fn open(&mut self) -> Option<Effect> {
            let row: &Row = self.rows.get(self.selected)?;

            // A pane is the end of the way down, so there is nothing in it to show.
            if row.pane().is_some() {
                return self.pick();
            }

            if let Some(window) = row.window() {
                return match self.opened.contains(&window) {
                    true => self.pick(),
                    false => {
                        self.opened.push(window);
                        self.lay_out();
                        None
                    }
                };
            }

            let session: Uuid = row.session();
            match self.shut.iter().position(|shut| *shut == session) {
                Some(at) => {
                    self.shut.remove(at);
                    self.lay_out();
                }
                None => self.move_by(1),
            }

            None
        }

        /// Stop showing what is in whatever is selected, or go out to whatever it is in.
        fn shut(&mut self) {
            let row: &Row = match self.rows.get(self.selected) {
                Some(row) => row,
                None => return,
            };
            let (session, window, pane) = (row.session(), row.window(), row.pane());

            // Going left from a pane goes to the window it is in.
            if pane.is_some() {
                self.select(|row| row.window() == window && row.pane().is_none());
                return;
            }

            if let Some(window) = window {
                // A window which is showing its panes stops showing them, and one which is not
                // goes out to the session it is in.
                match self.opened.iter().position(|open| *open == window) {
                    Some(at) => {
                        self.opened.remove(at);
                        self.lay_out();
                    }
                    None => self.select(|row| row.session() == session && row.window().is_none()),
                }
                return;
            }

            if !self.shut.contains(&session) {
                self.shut.push(session);
                self.lay_out();
            }
        }

        /// Select the first row which is the one looked for.
        fn select(&mut self, which: impl Fn(&Row) -> bool) {
            if let Some(at) = self.rows.iter().position(which) {
                self.selected = at;
            }
        }

        /// Return what showing the window which is selected asks for.
        ///
        /// A session which is selected rather than a window shows whichever of its windows is the
        /// one being looked at, which is what picking a session means.
        fn pick(&self) -> Option<Effect> {
            let row: &Row = self.rows.get(self.selected)?;

            if let Some(window) = row.window() {
                return Some(Effect::Show {
                    session: row.session(),
                    window,
                    pane: row.pane(),
                });
            }

            let session: &SessionSummary = self
                .sessions
                .iter()
                .find(|session| session.session() == &row.session())?;
            let window = session
                .windows()
                .iter()
                .find(|window| window.active())
                .or_else(|| session.windows().first())?;

            return Some(Effect::Show {
                session: row.session(),
                window: *window.window(),
                pane: None,
            });
        }

        /// Draw the list.
        pub fn render(&self, size: Size) -> Fabric {
            if self.rows.is_empty() {
                return Fabric::center("No sessions.", size);
            }

            // The rows are scrolled so that the one which is selected is always shown.
            let first: usize = self.selected.saturating_sub(size.rows.saturating_sub(1));

            let mut yarns: Vec<Yarn> = Vec::with_capacity(size.rows);
            for (at, row) in self.rows.iter().enumerate().skip(first).take(size.rows) {
                let mut yarn = Yarn::from(row.text());
                yarn.resize(size.columns);

                // NOTE: Everything is written the same way but for the row which is selected, so
                // that the list reads as one thing rather than the sessions standing apart from
                // the windows in them.
                if at == self.selected {
                    yarn.color(Color::InvertedText.into());
                    yarn.background(Color::Highlight.into());
                }

                yarns.push(yarn);
            }

            let mut fabric = Fabric::from(yarns);
            fabric.pad_bottom(size.rows);

            return fabric;
        }
    }
}
pub use tree::Tree;

/// Contains the [`Row`] struct.
mod row {
    use insh_api::SessionSummary;

    use uuid::Uuid;

    /// One row of the list of sessions and the windows in them.
    #[derive(Clone, PartialEq, Eq)]
    pub struct Row {
        /// Which session the row is for.
        session: Uuid,
        /// Which window of it the row is for, which is none at all for the row which names the
        /// session itself.
        window: Option<Uuid>,
        /// Which pane of that window the row is for, which is none at all for the row which names
        /// the window itself.
        pane: Option<Uuid>,
        /// Whether it is the window being looked at in the session being shown, which is the row
        /// the list opens on.
        here: bool,
        /// What the row says.
        text: String,
    }

    impl Row {
        /// What is drawn before something which is not the last of what it is in.
        const BRANCH: &'static str = "├─ ";

        /// What is drawn before the last of what it is in.
        const LAST: &'static str = "└─ ";

        /// What is drawn to the left of a pane of a window which is not the last of its session,
        /// to carry the line of that session on down past it.
        const THROUGH: &'static str = "│  ";

        /// What is drawn to the left of a pane of the last window of a session.
        const PAST: &'static str = "   ";

        /// What is drawn before something whose own are shown.
        const OPEN: &'static str = "▾ ";

        /// What is drawn before something whose own are not shown.
        const SHUT: &'static str = "▸ ";

        /// Return a row for each session, and for each window of the sessions which are open.
        ///
        /// Only the names are shown. How many windows a session has is plain from the ones listed
        /// under it, and which session and window are the ones being looked at is said by the
        /// list opening on them rather than by marking them.
        pub fn of(sessions: &[SessionSummary], shut: &[Uuid], opened: &[Uuid]) -> Vec<Self> {
            let mut rows: Vec<Self> = Vec::new();

            for session in sessions {
                let open: bool = !shut.contains(session.session());
                let windows: usize = session.windows().len();

                rows.push(Self {
                    session: *session.session(),
                    window: None,
                    pane: None,
                    here: false,
                    text: format!(
                        "{}{}",
                        match open {
                            true => Self::OPEN,
                            false => Self::SHUT,
                        },
                        session.name()
                    ),
                });

                if !open {
                    continue;
                }

                for (at, window) in session.windows().iter().enumerate() {
                    let last: bool = at + 1 == windows;
                    // NOTE: A window shows its panes only once it has been asked to, since a
                    // window is usually picked whole and listing every pane of every window
                    // would be a great deal to read through to find one.
                    let showing_panes: bool = opened.contains(window.window());

                    rows.push(Self {
                        session: *session.session(),
                        window: Some(*window.window()),
                        pane: None,
                        here: session.showing() && window.active(),
                        text: format!(
                            "{}{}{}",
                            match last {
                                true => Self::LAST,
                                false => Self::BRANCH,
                            },
                            match showing_panes {
                                true => Self::OPEN,
                                false => Self::SHUT,
                            },
                            window.name()
                        ),
                    });

                    if !showing_panes {
                        continue;
                    }

                    let panes: usize = window.panes().len();
                    for (number, pane) in window.panes().iter().enumerate() {
                        rows.push(Self {
                            session: *session.session(),
                            window: Some(*window.window()),
                            pane: Some(*pane.pane()),
                            here: false,
                            text: format!(
                                "{}{}{}",
                                match last {
                                    true => Self::PAST,
                                    false => Self::THROUGH,
                                },
                                match number + 1 == panes {
                                    true => Self::LAST,
                                    false => Self::BRANCH,
                                },
                                number
                            ),
                        });
                    }
                }
            }

            return rows;
        }

        /// Return which session the row is for.
        pub fn session(&self) -> Uuid {
            return self.session;
        }

        /// Return which window of it the row is for.
        pub fn window(&self) -> Option<Uuid> {
            return self.window;
        }

        /// Return which pane of that window the row is for.
        pub fn pane(&self) -> Option<Uuid> {
            return self.pane;
        }

        /// Return what the row says.
        pub fn text(&self) -> &str {
            return &self.text;
        }

        /// Return whether the row is the window being looked at in the session being shown.
        pub fn here(&self) -> bool {
            return self.here;
        }

        /// Return whether the row is for the same thing as the other one.
        pub fn is(&self, other: &Self) -> bool {
            return self.session == other.session
                && self.window == other.window
                && self.pane == other.pane;
        }
    }
}
use row::Row;

#[cfg(test)]
mod tests {
    use super::*;

    use insh_api::{PaneSummary, SessionSummary, WindowSummary};
    use rend::Size;
    use term::{Key, KeyEvent, KeyMods, TermEvent};

    use uuid::Uuid;

    /// Return the session with the given number, so that a test says which one it means.
    fn session(number: u128) -> Uuid {
        Uuid::from_u128(number)
    }

    /// Return the window with the given number.
    fn window(number: u128) -> Uuid {
        Uuid::from_u128(1000 + number)
    }

    /// Return the pane with the given number.
    fn pane(number: u128) -> Uuid {
        Uuid::from_u128(2000 + number)
    }

    /// Return the given number of panes, the first of which is the one typed in.
    fn panes(numbers: &[u128]) -> Vec<PaneSummary> {
        numbers
            .iter()
            .enumerate()
            .map(|(at, number)| {
                PaneSummary::builder()
                    .pane(pane(*number))
                    .active(at == 0)
                    .build()
            })
            .collect()
    }

    /// Return two sessions, the first with two windows and the second with one.
    fn sessions() -> Vec<SessionSummary> {
        vec![
            SessionSummary::builder()
                .session(session(1))
                .name("0")
                .windows(vec![
                    WindowSummary::builder()
                        .window(window(1))
                        .name("1")
                        .panes(panes(&[1, 2]))
                        .active(true)
                        .build(),
                    WindowSummary::builder()
                        .window(window(2))
                        .name("2")
                        .panes(panes(&[3]))
                        .build(),
                ])
                .clients(1)
                .showing(true)
                .build(),
            SessionSummary::builder()
                .session(session(2))
                .name("1")
                .windows(vec![WindowSummary::builder()
                    .window(window(3))
                    .name("1")
                    .panes(panes(&[4, 5, 6]))
                    .build()])
                .clients(0)
                .build(),
        ]
    }

    /// Return what the tree is showing, a row per line.
    fn shown(tree: &Tree, rows: usize) -> Vec<String> {
        let fabric = tree.render(Size::new(rows, 30));

        (0..rows)
            .map(|row| {
                (0..30)
                    .map(|column| fabric.spot(row, column).cell().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    /// Press the key.
    fn press(tree: &mut Tree, key: Key) -> Option<Effect> {
        tree.handle(TermEvent::KeyEvent(KeyEvent {
            key,
            mods: KeyMods::NONE,
        }))
    }

    #[test]
    fn test_every_session_and_every_window_of_each_is_listed() {
        let mut tree = Tree::default();
        tree.show(&sessions());

        assert_eq!(
            shown(&tree, 5),
            vec!["▾ 0", "├─ ▸ 1", "└─ ▸ 2", "▾ 1", "└─ ▸ 1"]
        );
    }

    #[test]
    fn test_the_list_opens_on_the_window_which_is_being_looked_at() {
        let mut tree = Tree::default();
        tree.show(&sessions());

        assert!(matches!(
            press(&mut tree, Key::CarriageReturn),
            Some(Effect::Show { window, .. }) if window == self::window(1)
        ));
    }

    #[test]
    fn test_a_window_lists_its_panes_once_it_is_asked_to() {
        let mut tree = Tree::default();
        tree.show(&sessions());

        // The first window is selected, and going right shows what is in it.
        press(&mut tree, Key::Char('l'));

        assert_eq!(
            shown(&tree, 6),
            vec!["▾ 0", "├─ ▾ 1", "│  ├─ 0", "│  └─ 1", "└─ ▸ 2", "▾ 1"]
        );
    }

    #[test]
    fn test_a_pane_of_a_window_is_picked() {
        let mut tree = Tree::default();
        tree.show(&sessions());
        press(&mut tree, Key::Char('l'));
        press(&mut tree, Key::Char('j'));
        press(&mut tree, Key::Char('j'));

        assert!(matches!(
            press(&mut tree, Key::CarriageReturn),
            Some(Effect::Show { window, pane, .. })
                if window == self::window(1) && pane == Some(self::pane(2))
        ));
    }

    #[test]
    fn test_going_left_from_a_pane_goes_to_the_window_it_is_in() {
        let mut tree = Tree::default();
        tree.show(&sessions());
        press(&mut tree, Key::Char('l'));
        press(&mut tree, Key::Char('j'));

        press(&mut tree, Key::Char('h'));

        // The window is selected, so picking it takes the window rather than one of its panes.
        assert!(matches!(
            press(&mut tree, Key::CarriageReturn),
            Some(Effect::Show { window, pane, .. })
                if window == self::window(1) && pane.is_none()
        ));
    }

    #[test]
    fn test_a_window_stops_listing_its_panes() {
        let mut tree = Tree::default();
        tree.show(&sessions());
        press(&mut tree, Key::Char('l'));

        press(&mut tree, Key::Char('h'));

        assert_eq!(shown(&tree, 4), vec!["▾ 0", "├─ ▸ 1", "└─ ▸ 2", "▾ 1"]);
    }

    #[test]
    fn test_a_session_which_is_shut_does_not_list_its_windows() {
        let mut tree = Tree::default();
        tree.show(&sessions());

        // Left from a window goes to the session it is in, and left again shuts it.
        press(&mut tree, Key::Char('h'));
        press(&mut tree, Key::Char('h'));

        assert_eq!(shown(&tree, 3), vec!["▸ 0", "▾ 1", "└─ ▸ 1"]);
    }

    #[test]
    fn test_a_session_which_is_shut_is_opened_again() {
        let mut tree = Tree::default();
        tree.show(&sessions());
        press(&mut tree, Key::Char('h'));
        press(&mut tree, Key::Char('h'));

        press(&mut tree, Key::Char('l'));

        assert_eq!(shown(&tree, 2)[1], "├─ ▸ 1");
    }

    #[test]
    fn test_going_right_from_a_session_which_is_open_walks_into_it() {
        let mut tree = Tree::default();
        tree.show(&sessions());
        press(&mut tree, Key::Char('h'));

        // The session is selected and already open, so right goes to the first window of it.
        press(&mut tree, Key::Char('l'));

        assert!(matches!(
            press(&mut tree, Key::CarriageReturn),
            Some(Effect::Show { window, .. }) if window == self::window(1)
        ));
    }

    #[test]
    fn test_picking_a_session_shows_the_window_being_looked_at_in_it() {
        let mut tree = Tree::default();
        tree.show(&sessions());
        press(&mut tree, Key::Char('h'));

        assert!(matches!(
            press(&mut tree, Key::CarriageReturn),
            Some(Effect::Show { session, window, .. })
                if session == self::session(1) && window == self::window(1)
        ));
    }

    #[test]
    fn test_moving_down_goes_through_the_sessions_as_well_as_the_windows() {
        let mut tree = Tree::default();
        tree.show(&sessions());

        // From the first window: its sibling, the second session, and then its window.
        for _ in 0..3 {
            press(&mut tree, Key::Char('j'));
        }

        assert!(matches!(
            press(&mut tree, Key::CarriageReturn),
            Some(Effect::Show { session, window, .. })
                if session == self::session(2) && window == self::window(3)
        ));
    }

    #[test]
    fn test_the_selection_does_not_move_past_either_end() {
        let mut tree = Tree::default();
        tree.show(&sessions());

        for _ in 0..10 {
            press(&mut tree, Key::Char('k'));
        }
        assert_eq!(shown(&tree, 1)[0], "▾ 0");

        for _ in 0..10 {
            press(&mut tree, Key::Char('j'));
        }
        assert!(matches!(
            press(&mut tree, Key::CarriageReturn),
            Some(Effect::Show { window, .. }) if window == self::window(3)
        ));
    }

    #[test]
    fn test_the_same_window_stays_selected_when_the_sessions_are_sent_again() {
        let mut tree = Tree::default();
        tree.show(&sessions());
        press(&mut tree, Key::Char('j'));

        tree.show(&sessions());

        assert!(matches!(
            press(&mut tree, Key::CarriageReturn),
            Some(Effect::Show { window, .. }) if window == self::window(2)
        ));
    }

    #[test]
    fn test_the_tree_is_closed_without_showing_anything() {
        let mut tree = Tree::default();
        tree.show(&sessions());

        assert!(matches!(
            press(&mut tree, Key::Char('q')),
            Some(Effect::Close)
        ));
    }
}

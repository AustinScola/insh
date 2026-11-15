//! Contains the [`Searcher`] component.

/// Contains the [`Searcher`] component.
mod searcher {
    use super::super::{Chats, Entry, Movement};
    use super::{Effect, Focus};
    use crate::components::common::{
        Footer, FooterProps, Hover, PaneContentsEvent as Event, Phrase, PhraseEffect, PhraseEvent,
        PhraseProps,
    };

    use insh_api::{
        ChatSearchMode, Request, RequestParams, Response, ResponseParams, SearchChatsRequestParams,
    };
    use rend::{Fabric, Location, Size};
    use term::{Key, KeyEvent, KeyMods, MouseButton, MouseEvent, MouseEventKind, TermEvent};
    use til::Component;

    use uuid::Uuid;

    /// The most chats to ask for at a time.
    const CHAT_LIMIT: usize = 100;

    /// The rows which the box what to search for is typed into and the footer take between them.
    const CHROME_ROWS: usize = 2;

    /// The rows above the chats, which the box what to search for is typed into takes.
    const PHRASE_ROWS: usize = 1;

    /// A search of the past chats.
    pub struct Searcher {
        /// The chats which were found.
        chats: Chats,
        /// The box which what to search for is typed into.
        phrase: Phrase,
        /// Which part of the searcher has focus.
        focus: Focus,
        /// The search which was made last.
        ///
        /// NOTE: Every pane is told about every response, and this is the only search wanted here.
        searching: Option<Uuid>,
        /// Whether what is typed goes to some pane other than the one this is in.
        pane_unfocused: bool,
    }

    impl Component<Size, Event, Effect> for Searcher {
        fn new(size: Size) -> Self {
            let mut phrase = Phrase::new(PhraseProps::builder().build());
            // There is nothing to look through yet, so this opens ready to be typed in.
            phrase.handle(PhraseEvent::Focus);

            let mut chats = Chats::default();
            // How much of the list is on the screen is what keeps the selected chat in view, and
            // it is only ever told that again when the terminal changes size.
            chats.resize(size.rows.saturating_sub(CHROME_ROWS));

            Self {
                chats,
                phrase,
                focus: Focus::Phrase,
                searching: None,
                pane_unfocused: false,
            }
        }

        fn name(&self) -> String {
            String::from("inshie history searcher")
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            match event {
                Event::SetFocus { focused } => {
                    self.pane_unfocused = !focused;
                    self.phrase.handle(PhraseEvent::SetPaneFocus { focused });
                    None
                }
                Event::Response(response) => self.handle_response(response),
                Event::TermEvent(TermEvent::Resize(size)) => {
                    self.chats.resize(size.rows.saturating_sub(CHROME_ROWS));
                    None
                }
                Event::TermEvent(TermEvent::Mouse(mouse_event)) => self.handle_mouse(mouse_event),
                Event::TermEvent(term_event) => match self.focus {
                    Focus::Phrase => self.handle_phrase(term_event),
                    Focus::Chats => match term_event {
                        TermEvent::KeyEvent(key_event) => self.handle_chats(key_event),
                        _ => None,
                    },
                },
            }
        }

        fn render(&self, size: Size) -> Fabric {
            if size.rows == 0 || size.columns == 0 {
                return Fabric::new(size);
            }

            let mut fabric: Fabric = self.phrase.render(Size::new(1, size.columns));

            if size.rows == 1 {
                return fabric;
            }

            if size.rows > CHROME_ROWS {
                fabric = fabric.quilt_bottom(self.chats.render(
                    Size::new(size.rows - CHROME_ROWS, size.columns),
                    matches!(self.focus, Focus::Chats) && !self.pane_unfocused,
                ));
            }

            let footer_props = FooterProps::builder()
                .name(self.name())
                .info(&self.chats)
                .build();
            fabric.quilt_bottom(Footer::new(footer_props).render(Size::new(1, size.columns)))
        }
    }

    impl Searcher {
        /// Return where in it looking back through it starts, which is the end of what is being
        /// typed or the start of the selected chat.
        pub fn copy_start(&self) -> Option<Location> {
            let (row, column): (usize, usize) = match self.focus {
                Focus::Phrase => (0, self.phrase.end()),
                // The box which what to search for is typed into is drawn above the chats.
                Focus::Chats => (self.chats.selected_row()? + 1, 0),
            };

            Some(Location::builder().row(row).column(column).build())
        }

        /// Return how the row is shown as being under the mouse when it is.
        ///
        /// The box what to search for is typed into is while it is not being typed in, and
        /// otherwise only a row with a chat in it is.
        pub fn hovers(&self, row: usize) -> Option<Hover> {
            match row.checked_sub(PHRASE_ROWS) {
                None => (self.focus != Focus::Phrase).then_some(Hover::Row),
                Some(row) => self
                    .chats
                    .hovers(row, self.focus == Focus::Chats && !self.pane_unfocused)
                    .then_some(Hover::Text),
            }
        }

        /// Handle something typed into the box which what to search for goes in.
        fn handle_phrase(&mut self, term_event: TermEvent) -> Option<Effect> {
            match self.phrase.handle(PhraseEvent::TermEvent(term_event)) {
                Some(PhraseEffect::Enter { phrase }) => {
                    // What was typed is what is being looked through now, so the box stops being
                    // what has focus and stops being colored as if it were.
                    self.phrase.handle(PhraseEvent::Unfocus);
                    self.focus = Focus::Chats;

                    if phrase.is_empty() {
                        return None;
                    }

                    let request: Request = Request::builder()
                        .params(RequestParams::SearchChats(
                            SearchChatsRequestParams::builder()
                                .text(phrase)
                                .mode(ChatSearchMode::Both)
                                .limit(CHAT_LIMIT)
                                .build(),
                        ))
                        .build();
                    self.searching = Some(*request.uuid());

                    Some(Effect::Request(request))
                }
                Some(PhraseEffect::Quit) => Some(Effect::Quit),
                Some(PhraseEffect::Bell) => Some(Effect::Bell),
                _ => None,
            }
        }

        /// Handle something typed while what was found has focus.
        fn handle_chats(&mut self, key_event: KeyEvent) -> Option<Effect> {
            match self.chats.handle(&key_event) {
                Movement::Moved => return None,
                Movement::Stuck => return Some(Effect::Bell),
                Movement::Ignored => {}
            }

            match (key_event.key, key_event.mods) {
                // Leaving what was found goes back to what was searched for, and leaving that
                // again is what goes back to the past chats.
                (Key::Char('q'), KeyMods::CONTROL) | (Key::Char('/'), KeyMods::NONE) => {
                    self.phrase.handle(PhraseEvent::Focus);
                    self.focus = Focus::Phrase;
                    None
                }
                (Key::CarriageReturn, _) | (Key::Char('l'), KeyMods::NONE) => {
                    let entry: &Entry = self.chats.selected()?;
                    Some(Effect::Open { id: entry.id })
                }
                _ => None,
            }
        }

        /// Handle something done with the mouse.
        ///
        /// Pressing on the box what to search for is typed into goes back to it, and pressing on
        /// the chats goes to them, so the mouse goes between the two the way the keys do.
        fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Option<Effect> {
            match mouse_event.kind {
                MouseEventKind::Press(MouseButton::Left) if mouse_event.row < PHRASE_ROWS => {
                    if self.focus == Focus::Chats {
                        self.phrase.handle(PhraseEvent::Focus);
                        self.focus = Focus::Phrase;
                    }
                    None
                }
                MouseEventKind::Press(MouseButton::Left) => {
                    let focused: bool = self.focus == Focus::Chats && !self.pane_unfocused;
                    let row: usize = mouse_event.row - PHRASE_ROWS;
                    let opened: Option<i64> = self.chats.press(row, focused).map(|entry| entry.id);

                    // Pressing below the chats which were found is not pressing on them.
                    if self.focus == Focus::Phrase && self.chats.shows(row) {
                        self.phrase.handle(PhraseEvent::Unfocus);
                        self.focus = Focus::Chats;
                    }

                    Some(Effect::Open { id: opened? })
                }
                MouseEventKind::ScrollUp => {
                    self.chats.wheel(true);
                    None
                }
                MouseEventKind::ScrollDown => {
                    self.chats.wheel(false);
                    None
                }
                _ => None,
            }
        }

        /// Handle a response from the daemon.
        fn handle_response(&mut self, response: Response) -> Option<Effect> {
            let uuid: Option<Uuid> = Some(*response.uuid());
            let params = match response.params() {
                ResponseParams::SearchChats(_) if uuid != self.searching => return None,
                ResponseParams::SearchChats(params) => params,
                _ => {
                    #[cfg(feature = "logging")]
                    log::error!("Unexpected response parameters.");
                    return None;
                }
            };

            // The chats found by the words which were used come first, then the ones found by what
            // was meant, leaving out any which are already there.
            let mut entries: Vec<Entry> = params
                .chats()
                .iter()
                .map(|chat| Entry {
                    id: chat.id(),
                    title: chat.title().to_string(),
                })
                .collect();
            for hit in params.hits() {
                if entries.iter().any(|entry| entry.id == hit.chat().id()) {
                    continue;
                }
                entries.push(Entry {
                    id: hit.chat().id(),
                    title: hit.chat().title().to_string(),
                });
            }

            self.chats.show(entries);

            None
        }
    }
}
pub use searcher::Searcher;

/// Contains the [`Focus`] enum.
mod focus {
    /// Which part of the searcher has focus.
    #[derive(Default, Clone, Copy, Eq, PartialEq)]
    pub enum Focus {
        /// The box which what to search for is typed into.
        #[default]
        Phrase,
        /// The chats which were found.
        Chats,
    }
}
use focus::Focus;

/// Contains the [`Effect`] enum.
mod effect {
    use insh_api::Request;

    /// A searcher effect.
    pub enum Effect {
        /// Open a chat.
        Open {
            /// Which chat to open.
            id: i64,
        },
        /// Make a request.
        Request(Request),
        /// Go back to the past chats.
        Quit,
        /// Ring the bell.
        Bell,
    }
}
pub use effect::Effect;

#[cfg(test)]
mod tests {
    use super::Searcher;

    use rend::{Fabric, Size};
    use til::Component;

    use test_case::test_case;

    /// Whatever it is asked for, the searcher has to give back a fabric of exactly that size.
    #[test_case(24, 80; "a normal terminal")]
    #[test_case(24, 40; "a narrow terminal")]
    #[test_case(3, 80; "room for one chat")]
    #[test_case(2, 80; "only room for the box and the footer")]
    #[test_case(1, 80; "one row")]
    #[test_case(0, 80; "no rows")]
    #[test_case(24, 0; "no columns")]
    fn test_render_fills_the_size_it_is_given(rows: usize, columns: usize) {
        let size: Size = Size::new(rows, columns);

        let fabric: Fabric = Searcher::new(size).render(size);

        assert_eq!(fabric.size(), size);
    }
}

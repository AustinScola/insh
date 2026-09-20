//! Contains the [`Searcher`] component.

/// Contains the [`Searcher`] component.
mod searcher {
    use super::super::{Chats, Entry, Movement};
    use super::{Effect, Focus};
    use crate::components::common::{
        Footer, FooterProps, Phrase, PhraseEffect, PhraseEvent, PhraseProps,
    };

    use insh_api::{
        ChatSearchMode, Request, RequestParams, Response, ResponseParams, SearchChatsRequestParams,
    };
    use rend::{Fabric, Size};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::{Component, Event};

    /// The most chats to ask for at a time.
    const CHAT_LIMIT: usize = 100;

    /// The rows which the box what to search for is typed into and the footer take between them.
    const CHROME_ROWS: usize = 2;

    /// A search of the past chats.
    pub struct Searcher {
        /// The chats which were found.
        chats: Chats,
        /// The box which what to search for is typed into.
        phrase: Phrase,
        /// Which part of the searcher has focus.
        focus: Focus,
    }

    impl Component<Size, Event<Response>, Effect> for Searcher {
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
            }
        }

        fn name(&self) -> String {
            String::from("inshie history searcher")
        }

        fn handle(&mut self, event: Event<Response>) -> Option<Effect> {
            match event {
                Event::Response(response) => self.handle_response(response),
                Event::TermEvent(TermEvent::Resize(size)) => {
                    self.chats.resize(size.rows.saturating_sub(CHROME_ROWS));
                    None
                }
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
                    matches!(self.focus, Focus::Chats),
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

                    Some(Effect::Request(
                        Request::builder()
                            .params(RequestParams::SearchChats(
                                SearchChatsRequestParams::builder()
                                    .text(phrase)
                                    .mode(ChatSearchMode::Both)
                                    .limit(CHAT_LIMIT)
                                    .build(),
                            ))
                            .build(),
                    ))
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

        /// Handle a response from the daemon.
        fn handle_response(&mut self, response: Response) -> Option<Effect> {
            let params = match response.params() {
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

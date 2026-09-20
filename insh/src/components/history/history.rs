//! Contains the [`History`] component.

/// Contains the [`Entry`] struct.
mod entry {
    /// A chat in the history.
    #[derive(Clone)]
    pub struct Entry {
        /// Which chat it is.
        pub id: i64,
        /// What the chat is called.
        pub title: String,
    }
}
pub use entry::Entry;

/// Contains the [`Chats`] struct.
mod chats {
    use super::Entry;
    use crate::color::Color;
    use crate::components::common::FooterInfo;

    use rend::{Fabric, Size, Yarn};
    use term::{Key, KeyEvent, KeyMods};

    /// A list of chats which can be looked through.
    ///
    /// Both the history and the searcher of it show one of these, so what it takes to move around a
    /// list of chats is written once.
    #[derive(Default)]
    pub struct Chats {
        /// The chats which are shown.
        entries: Vec<Entry>,
        /// Which chat is selected.
        selected: Option<usize>,
        /// The first chat which is shown.
        offset: usize,
        /// How many rows the list has.
        rows: usize,
    }

    /// What moving around a list of chats did.
    pub enum Movement {
        /// The list moved.
        Moved,
        /// There was nowhere to move to.
        Stuck,
        /// The key was not one the list responds to.
        Ignored,
    }

    impl Chats {
        /// Show some chats, starting again from the first.
        pub fn show(&mut self, entries: Vec<Entry>) {
            self.selected = match entries.is_empty() {
                true => None,
                false => Some(0),
            };
            self.entries = entries;
            self.offset = 0;
        }

        /// Take note of how many rows the list has.
        pub fn resize(&mut self, rows: usize) {
            self.rows = rows;
            self.scroll_to_selected();
        }

        /// Return which chat is selected, if one is.
        pub fn selected(&self) -> Option<&Entry> {
            self.entries.get(self.selected?)
        }

        /// Move around the list, and say whether anything came of it.
        pub fn handle(&mut self, key_event: &KeyEvent) -> Movement {
            match (&key_event.key, &key_event.mods) {
                (Key::Char('j'), &KeyMods::NONE) | (Key::Down, &KeyMods::NONE) => self.down(),
                (Key::Char('k'), &KeyMods::NONE) | (Key::Up, &KeyMods::NONE) => self.up(),
                (Key::Char('J'), &KeyMods::SHIFT) | (Key::End, &KeyMods::NONE) => {
                    self.really_down()
                }
                (Key::Char('K'), &KeyMods::SHIFT) | (Key::Home, &KeyMods::NONE) => self.really_up(),
                (Key::Char('e'), &KeyMods::CONTROL) => self.scroll_down(),
                (Key::Char('y'), &KeyMods::CONTROL) => self.scroll_up(),
                _ => Movement::Ignored,
            }
        }

        /// Select the chat after the selected one.
        fn down(&mut self) -> Movement {
            let selected: usize = match self.selected {
                Some(selected) => selected,
                None => return Movement::Stuck,
            };
            if selected + 1 >= self.entries.len() {
                return Movement::Stuck;
            }

            self.selected = Some(selected + 1);
            self.scroll_to_selected();
            Movement::Moved
        }

        /// Select the chat before the selected one.
        fn up(&mut self) -> Movement {
            let selected: usize = match self.selected {
                Some(selected) => selected,
                None => return Movement::Stuck,
            };
            if selected == 0 {
                return Movement::Stuck;
            }

            self.selected = Some(selected - 1);
            self.scroll_to_selected();
            Movement::Moved
        }

        /// Select the last chat, bringing it into view.
        fn really_down(&mut self) -> Movement {
            if self.entries.is_empty() {
                return Movement::Stuck;
            }
            if self.selected == Some(self.entries.len() - 1) {
                return Movement::Stuck;
            }

            self.selected = Some(self.entries.len() - 1);
            self.scroll_to_selected();
            Movement::Moved
        }

        /// Select the first chat, bringing it into view.
        fn really_up(&mut self) -> Movement {
            if self.entries.is_empty() || self.selected == Some(0) {
                return Movement::Stuck;
            }

            self.selected = Some(0);
            self.scroll_to_selected();
            Movement::Moved
        }

        /// Show the chats after the ones which are shown.
        fn scroll_down(&mut self) -> Movement {
            if self.rows == 0 || self.offset + self.rows >= self.entries.len() {
                return Movement::Stuck;
            }

            self.offset += 1;
            // What is selected goes with the view rather than out of it.
            if let Some(selected) = self.selected {
                if selected < self.offset {
                    self.selected = Some(self.offset);
                }
            }
            Movement::Moved
        }

        /// Show the chats before the ones which are shown.
        fn scroll_up(&mut self) -> Movement {
            if self.offset == 0 || self.rows == 0 {
                return Movement::Stuck;
            }

            self.offset -= 1;
            if let Some(selected) = self.selected {
                if selected >= self.offset + self.rows {
                    self.selected = Some(self.offset + self.rows - 1);
                }
            }
            Movement::Moved
        }

        /// Bring the selected chat into view.
        fn scroll_to_selected(&mut self) {
            let selected: usize = match self.selected {
                Some(selected) => selected,
                None => return,
            };

            if selected < self.offset {
                self.offset = selected;
            } else if self.rows > 0 && selected >= self.offset + self.rows {
                self.offset = selected + 1 - self.rows;
            }
        }

        /// Return the chats, as they are shown.
        ///
        /// The selected one is only picked out in the color for what has focus while the list is
        /// what has it, since that is what the color says.
        pub fn render(&self, size: Size, focused: bool) -> Fabric {
            // Nothing is said when there are no chats. An empty list says that by itself.

            let mut yarns: Vec<Yarn> = Vec::with_capacity(size.rows);
            for (index, entry) in self
                .entries
                .iter()
                .enumerate()
                .skip(self.offset)
                .take(size.rows)
            {
                let mut yarn: Yarn = Yarn::from(entry.title.as_str());
                yarn.resize(size.columns);

                if self.selected == Some(index) {
                    yarn.color(Color::InvertedText.into());
                    yarn.background(Color::highlight(focused).into());
                }

                yarns.push(yarn);
            }

            let mut fabric: Fabric = Fabric::from(yarns);
            fabric.pad_bottom(size.rows);
            fabric
        }
    }

    impl FooterInfo for Chats {
        fn position(&self) -> String {
            if self.entries.is_empty() {
                return String::from("-/0");
            }

            let selected: String = match self.selected {
                Some(selected) => (selected + 1).to_string(),
                None => String::from("-"),
            };
            format!("{}/{}", selected, self.entries.len())
        }
    }
}
pub use chats::{Chats, Movement};

/// Contains the [`History`] component.
mod history {
    use super::{Chats, Effect, Entry, Movement};
    use crate::components::common::{Footer, FooterProps};

    use insh_api::{
        DeleteChatRequestParams, ListChatsRequestParams, Request, RequestParams, Response,
        ResponseParams,
    };
    use rend::{Fabric, Size};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::{Component, Event};

    /// The most chats to ask for at a time.
    const CHAT_LIMIT: usize = 100;

    /// The rows which the footer takes.
    const FOOTER_ROWS: usize = 1;

    /// The past chats.
    #[derive(Default)]
    pub struct History {
        /// The chats.
        chats: Chats,
        /// Whether a `d` has been pressed and is waiting for the second one which deletes a chat.
        deleting: bool,
    }

    impl Component<Size, Event<Response>, Effect> for History {
        fn new(size: Size) -> Self {
            let mut history = Self::default();
            // How much of the list is on the screen is what keeps the selected chat in view, and
            // it is only ever told that again when the terminal changes size.
            history.chats.resize(size.rows.saturating_sub(FOOTER_ROWS));
            history
        }

        fn name(&self) -> String {
            String::from("inshie history")
        }

        fn handle(&mut self, event: Event<Response>) -> Option<Effect> {
            match event {
                Event::Response(response) => self.handle_response(response),
                Event::TermEvent(TermEvent::Resize(size)) => {
                    self.chats.resize(size.rows.saturating_sub(FOOTER_ROWS));
                    None
                }
                Event::TermEvent(TermEvent::KeyEvent(key_event)) => self.handle_key(key_event),
                Event::TermEvent(_) => None,
            }
        }

        fn render(&self, size: Size) -> Fabric {
            if size.rows == 0 || size.columns == 0 {
                return Fabric::new(size);
            }

            let footer_props = FooterProps::builder()
                .name(self.name())
                .info(&self.chats)
                .build();
            let footer: Fabric = Footer::new(footer_props).render(Size::new(1, size.columns));

            if size.rows == FOOTER_ROWS {
                return footer;
            }

            let list: Fabric = self
                .chats
                .render(Size::new(size.rows - FOOTER_ROWS, size.columns), true);
            list.quilt_bottom(footer)
        }
    }

    impl History {
        /// Return the request to make when the history is opened.
        pub fn initial_request() -> Request {
            Request::builder()
                .params(RequestParams::ListChats(
                    ListChatsRequestParams::builder().limit(CHAT_LIMIT).build(),
                ))
                .build()
        }

        /// Handle something which was typed.
        fn handle_key(&mut self, key_event: KeyEvent) -> Option<Effect> {
            // Deleting takes two presses of `d`, and anything else in between calls it off.
            let deleting: bool = std::mem::take(&mut self.deleting);
            if let (Key::Char('d'), &KeyMods::NONE) = (&key_event.key, &key_event.mods) {
                if !deleting {
                    self.deleting = true;
                    return None;
                }

                let entry: &Entry = match self.chats.selected() {
                    Some(entry) => entry,
                    None => return Some(Effect::Bell),
                };

                return Some(Effect::Request(
                    Request::builder()
                        .params(RequestParams::DeleteChat(
                            DeleteChatRequestParams::builder().chat_id(entry.id).build(),
                        ))
                        .build(),
                ));
            }

            match self.chats.handle(&key_event) {
                Movement::Moved => return None,
                Movement::Stuck => return Some(Effect::Bell),
                Movement::Ignored => {}
            }

            match (key_event.key, key_event.mods) {
                (Key::Char('q'), KeyMods::CONTROL) => Some(Effect::Quit),
                (Key::Char('n'), KeyMods::NONE) => Some(Effect::New),
                (Key::Char('/'), KeyMods::NONE) => Some(Effect::Search),
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
                ResponseParams::ListChats(params) => params,
                // A chat which was deleted is still in the list until it is asked for again.
                ResponseParams::DeleteChat(params) => {
                    return match params.deleted() {
                        true => Some(Effect::Request(Self::initial_request())),
                        false => Some(Effect::Bell),
                    };
                }
                _ => {
                    #[cfg(feature = "logging")]
                    log::error!("Unexpected response parameters.");
                    return None;
                }
            };

            self.chats.show(
                params
                    .chats()
                    .iter()
                    .map(|chat| Entry {
                        id: chat.id(),
                        title: chat.title().to_string(),
                    })
                    .collect(),
            );

            None
        }
    }
}
pub use history::History;

/// Contains the [`Effect`] enum.
mod effect {
    use insh_api::Request;

    /// A history effect.
    pub enum Effect {
        /// Open a chat.
        Open {
            /// Which chat to open.
            id: i64,
        },
        /// Make a request.
        Request(Request),
        /// Start a new chat.
        New,
        /// Search the past chats.
        Search,
        /// Go back to the chat.
        Quit,
        /// Ring the bell.
        Bell,
    }
}
pub use effect::Effect;

#[cfg(test)]
mod tests {
    use super::History;
    use super::{Chats, Effect, Entry, Movement};

    use insh_api::{Chat, ListChatsResponseParams, RequestParams, Response, ResponseParams};
    use rend::{Fabric, Size};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::{Component, Event};

    use std::path::PathBuf;
    use uuid::Uuid;

    use test_case::test_case;

    /// Return a list of the given number of chats, with the given number of rows to show them in.
    fn chats(count: usize, rows: usize) -> Chats {
        let mut chats = Chats::default();
        chats.resize(rows);
        chats.show(
            (0..count)
                .map(|number| Entry {
                    id: number as i64,
                    title: number.to_string(),
                })
                .collect(),
        );
        chats
    }

    /// Move down the list the given number of times.
    fn down(chats: &mut Chats, times: usize) {
        let key_event = KeyEvent {
            key: Key::Char('j'),
            mods: KeyMods::NONE,
        };

        for _ in 0..times {
            if matches!(chats.handle(&key_event), Movement::Stuck) {
                break;
            }
        }
    }

    /// Return whether the effect is a request to delete a chat.
    fn deletes(effect: Option<Effect>) -> bool {
        matches!(effect, Some(Effect::Request(request))
            if matches!(request.params(), RequestParams::DeleteChat(_)))
    }

    /// Return a history showing one chat.
    fn history() -> History {
        let mut history = History::new(Size::new(10, 20));
        history.handle(Event::Response(
            Response::builder()
                .uuid(Uuid::new_v4())
                .params(ResponseParams::ListChats(
                    ListChatsResponseParams::builder()
                        .chats(vec![Chat::builder()
                            .id(7)
                            .title("one")
                            .dir(PathBuf::from("/tmp"))
                            .build()])
                        .build(),
                ))
                .build(),
        ));
        history
    }

    /// Return the effect of typing a key.
    fn press(history: &mut History, key: Key) -> Option<Effect> {
        history.handle(Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
            key,
            mods: KeyMods::NONE,
        })))
    }

    /// One `d` does nothing on its own, and the second one deletes the selected chat.
    #[test]
    fn test_two_ds_delete_a_chat() {
        let mut history = history();

        assert!(!deletes(press(&mut history, Key::Char('d'))));
        assert!(deletes(press(&mut history, Key::Char('d'))));
    }

    /// Anything typed between the two calls the deleting off.
    #[test]
    fn test_a_key_in_between_calls_the_delete_off() {
        let mut history = history();

        press(&mut history, Key::Char('d'));
        press(&mut history, Key::Char('k'));

        assert!(!deletes(press(&mut history, Key::Char('d'))));
    }

    /// Shift and j goes to the last chat, and shift and k back to the first, the same as the
    /// browser.
    #[test]
    fn test_shift_jumps_to_the_ends_of_the_list() {
        let rows: usize = 5;
        let mut chats = chats(20, rows);

        let jump = |key: char| KeyEvent {
            key: Key::Char(key),
            mods: KeyMods::SHIFT,
        };

        assert!(matches!(chats.handle(&jump('J')), Movement::Moved));
        assert_eq!(chats.selected().map(|entry| entry.id), Some(19));
        // There is nowhere further to go once it is there.
        assert!(matches!(chats.handle(&jump('J')), Movement::Stuck));

        assert!(matches!(chats.handle(&jump('K')), Movement::Moved));
        assert_eq!(chats.selected().map(|entry| entry.id), Some(0));
        assert!(matches!(chats.handle(&jump('K')), Movement::Stuck));
    }

    /// The selected chat stays on the screen however far down the list it is moved.
    ///
    /// Which rows are on the screen was only ever told to the list when the terminal changed size,
    /// so until that happened it thought it had none and let the selection run off the bottom.
    #[test]
    fn test_moving_down_keeps_the_selected_chat_in_view() {
        let rows: usize = 5;
        let mut chats = chats(20, rows);

        down(&mut chats, 19);

        let shown: Fabric = chats.render(Size::new(rows, 20), true);
        let last: String = shown.cells()[rows - 1]
            .iter()
            .map(ToString::to_string)
            .collect();

        // The last chat is the one selected, and it is on the last row of the screen.
        assert_eq!(last.trim(), "19");
    }

    /// Whatever it is asked for, the history has to give back a fabric of exactly that size.
    #[test_case(24, 80; "a normal terminal")]
    #[test_case(24, 40; "a narrow terminal")]
    #[test_case(2, 80; "room for one chat")]
    #[test_case(1, 80; "only room for the footer")]
    #[test_case(0, 80; "no rows")]
    #[test_case(24, 0; "no columns")]
    fn test_render_fills_the_size_it_is_given(rows: usize, columns: usize) {
        let size: Size = Size::new(rows, columns);

        let fabric: Fabric = History::new(size).render(size);

        assert_eq!(fabric.size(), size);
    }
}

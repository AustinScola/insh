mod props {
    use std::path::PathBuf;

    use rend::Size;
    use typed_builder::TypedBuilder;

    #[derive(TypedBuilder)]
    pub struct Props {
        pub dir: PathBuf,
        pub size: Size,
    }
}
pub use props::Props;

mod contents {
    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::stateful::Stateful;

    use rend::{Fabric, Size, Yarn};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::Component;

    use std::path::{Path, MAIN_SEPARATOR as PATH_SEPARATOR};

    pub struct Contents {
        state: State,
    }

    impl Component<Props, Event, Effect> for Contents {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Self { state }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Option<Action> = match event {
                Event::Find { phrase } => Some(Action::Find { phrase }),
                Event::TermEvent(term_event) => match term_event {
                    TermEvent::Resize(size) => Some(Action::Resize { size }),
                    TermEvent::KeyEvent(key_event) => match key_event {
                        KeyEvent {
                            key: Key::Char('q'),
                            mods: KeyMods::CONTROL,
                            ..
                        } => Some(Action::Unfocus),
                        KeyEvent {
                            key: Key::Char('j'),
                            mods: KeyMods::NONE,
                        } => Some(Action::Down),
                        KeyEvent {
                            key: Key::Char('J'),
                            mods: KeyMods::SHIFT,
                        } => Some(Action::ReallyDown),
                        KeyEvent {
                            key: Key::Char('k'),
                            mods: KeyMods::NONE,
                        } => Some(Action::Up),
                        KeyEvent {
                            key: Key::Char('K'),
                            mods: KeyMods::SHIFT,
                        } => Some(Action::ReallyUp),
                        KeyEvent {
                            key: Key::Char('r'),
                            mods: KeyMods::NONE,
                        } => Some(Action::Refresh),
                        KeyEvent {
                            key: Key::Char('l'),
                            ..
                        }
                        | KeyEvent {
                            key: Key::CarriageReturn,
                            ..
                        } => Some(Action::Edit),
                        KeyEvent {
                            key: Key::Char('g'),
                            mods: KeyMods::NONE,
                        } => Some(Action::Goto),
                        KeyEvent {
                            key: Key::Char('G'),
                            mods: KeyMods::SHIFT,
                        } => Some(Action::ReallyGoto),
                        KeyEvent {
                            key: Key::Char('y'),
                            mods: KeyMods::NONE,
                            ..
                        } => Some(Action::Yank),
                        KeyEvent {
                            key: Key::Char('Y'),
                            mods: KeyMods::SHIFT,
                            ..
                        } => Some(Action::ReallyYank),
                        _ => None,
                    },
                },
                Event::Response(response) => Some(Action::HandleResponse(response)),
            };

            if let Some(action) = action {
                self.state.perform(action)
            } else {
                Some(Effect::Bell)
            }
        }

        fn render(&self, size: Size) -> Fabric {
            match self.state.hits() {
                Some(true) => {
                    let dir: &str = &self.state.dir().to_string_lossy();

                    let mut yarns: Vec<Yarn> = Vec::new();
                    for (entry, row) in self.state.visible_entries().iter().zip(0..size.rows) {
                        let path: &Path = entry.path();
                        let mut string: &str = &path.to_string_lossy();
                        string = string.strip_prefix(dir).unwrap();
                        if string.starts_with(PATH_SEPARATOR) {
                            string = string.strip_prefix(PATH_SEPARATOR).unwrap();
                        }
                        let mut yarn: Yarn = Yarn::from(string);

                        let file_name_start: usize =
                            yarn.len() - entry.file_name().expect("Entry is not a file").len();

                        if self.state.focussed() && Some(row) == self.state.selected() {
                            yarn.color_before(Color::InvertedGrayedText.into(), file_name_start);
                            yarn.color_after(Color::InvertedText.into(), file_name_start);
                            yarn.background(Color::Highlight.into());
                        } else {
                            yarn.color_before(Color::GrayedText.into(), file_name_start);
                        }

                        yarn.resize(size.columns);

                        yarns.push(yarn);
                    }

                    let mut fabric = Fabric::from(yarns);

                    if fabric.size().rows < size.rows {
                        fabric.pad_bottom(size.rows);
                    }

                    fabric
                }
                Some(false) => Fabric::center("No matching files.", size),
                None => Fabric::new(size),
            }
        }
    }
}
pub use contents::Contents;

mod event {
    use insh_api::Response;
    use term::TermEvent;

    #[allow(clippy::enum_variant_names)]
    pub enum Event {
        Find { phrase: String },
        Response(Response),
        TermEvent(TermEvent),
    }
}
pub use event::Event;

mod state {
    use super::{Action, Effect, Props};
    use crate::clipboard::Clipboard;
    use crate::programs::{VimArgs, VimArgsBuilder};
    use crate::stateful::Stateful;

    use insh_api::{FindFilesResponseParams, Response, ResponseParams};
    use path_finder::Entry;
    use rend::Size;

    use std::cmp::{self, Ordering};
    use std::path::{Path, PathBuf, MAIN_SEPARATOR as PATH_SEPARATOR};

    use uuid::Uuid;

    pub struct State {
        size: Size,
        dir: PathBuf,
        phrase: Option<String>,
        focussed: bool,
        hits: Option<bool>,
        entries: Vec<Entry>,
        selected: Option<usize>,
        offset: usize,
        pending_request: Option<Uuid>,
        received_first_resp: bool,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                size: props.size,
                dir: props.dir,
                phrase: None,
                focussed: false,
                hits: None,
                entries: Vec::new(),
                selected: None,
                offset: 0,
                pending_request: None,
                received_first_resp: false,
            }
        }
    }

    impl State {
        pub fn dir(&self) -> &PathBuf {
            &self.dir
        }

        pub fn focussed(&self) -> bool {
            self.focussed
        }

        pub fn hits(&self) -> Option<bool> {
            self.hits
        }

        pub fn visible_entries(&self) -> &[Entry] {
            if self.entries.is_empty() {
                return &[];
            }
            let start = self.offset;
            let end = cmp::min(self.offset + self.size.rows, self.entries.len());
            &self.entries[start..end]
        }

        pub fn selected(&self) -> Option<usize> {
            self.selected
        }

        fn entry_number(&self) -> Option<usize> {
            self.selected.map(|selected| self.offset + selected)
        }

        fn entry_path(&self) -> Option<&Path> {
            match self.entry_number() {
                Some(entry_number) => Some(self.entries[entry_number].path()),
                None => None,
            }
        }

        fn resize(&mut self, new_size: Size) -> Option<Effect> {
            if let Some(selected) = self.selected {
                let rows_before = self.size.rows;
                let entry_count = self.entries.len();
                let mut visible_entries_count = cmp::min(rows_before, entry_count - self.offset);
                let selected_percent: f64 = selected as f64 / visible_entries_count as f64;

                let mut new_selected: usize = (new_size.rows as f64 * selected_percent) as usize;
                let mut new_offset: usize;
                let entry_number = self.offset + selected;
                match entry_number.cmp(&new_selected) {
                    Ordering::Less | Ordering::Equal => {
                        new_offset = 0;
                        new_selected = entry_number;
                    }
                    Ordering::Greater => {
                        new_offset = entry_number - new_selected;
                        visible_entries_count = entry_count - new_offset;
                        if visible_entries_count < new_size.rows {
                            let bottom_pinned_offset = entry_count.saturating_sub(new_size.rows);
                            let difference = new_offset - bottom_pinned_offset;
                            new_selected += difference;
                            new_offset = bottom_pinned_offset;
                        }
                    }
                }

                self.offset = new_offset;
                self.selected = Some(new_selected);
            }

            self.size = new_size;
            None
        }

        fn focus(&mut self) {
            self.focussed = true;
        }

        fn unfocus(&mut self) -> Option<Effect> {
            self.focussed = false;
            Some(Effect::Unfocus)
        }

        fn find(&mut self, phrase: &str) -> Option<Effect> {
            self.focus();
            self.phrase = Some(phrase.to_string());
            let uuid: Uuid = Uuid::new_v4();
            self.pending_request = Some(uuid);
            self.received_first_resp = false;
            Some(Effect::SendFindFilesRequest {
                uuid,
                dir: self.dir.clone(),
                pattern: phrase.to_string(),
            })
        }

        fn down(&mut self) -> Option<Effect> {
            if self.entries.is_empty() {
                return None;
            }

            let entry_number = self.entry_number().unwrap();
            if entry_number >= self.entries.len() - 1 {
                return None;
            }
            let selected = self.selected.unwrap();
            if selected < self.size.rows - 1 {
                self.selected = Some(selected + 1);
            } else {
                self.offset += 1;
            }
            None
        }

        /// Select the last hit and adjust the scroll position if necessary.
        fn really_down(&mut self) -> Option<Effect> {
            if self.entries.is_empty() {
                return None;
            }

            if self.entries.len() > self.size.rows {
                self.offset = self.entries.len() - self.size.rows;
                self.selected = Some(self.size.rows - 1);
            } else {
                self.selected = Some(self.entries.len() - 1);
            }

            None
        }

        fn up(&mut self) -> Option<Effect> {
            if let Some(selected) = self.selected {
                if selected > 0 {
                    self.selected = Some(selected.saturating_sub(1))
                } else {
                    self.offset = self.offset.saturating_sub(1);
                }
            }
            None
        }

        /// Select the first hit and adjust the scroll position if necessary.
        fn really_up(&mut self) -> Option<Effect> {
            self.offset = 0;
            self.selected = Some(0);
            None
        }

        /// Refresh the hits by finding the phrase again.
        fn refresh(&mut self) -> Option<Effect> {
            if let Some(phrase) = self.phrase.clone() {
                return self.find(&phrase);
            }
            None
        }

        fn edit(&mut self) -> Option<Effect> {
            match self.entry_path() {
                Some(path) => {
                    let vim_args: VimArgs = VimArgsBuilder::new().path(path).build();
                    Some(Effect::OpenVim(vim_args))
                }
                None => None,
            }
        }

        fn goto(&mut self) -> Option<Effect> {
            self._goto(false)
        }

        fn really_goto(&mut self) -> Option<Effect> {
            self._goto(true)
        }

        fn _goto(&mut self, really: bool) -> Option<Effect> {
            match self.entry_path() {
                Some(entry) => {
                    let dir = entry.parent().unwrap().to_path_buf();
                    let file: Option<PathBuf> = if really {
                        Some(entry.to_path_buf())
                    } else {
                        None
                    };

                    Some(Effect::Goto { dir, file })
                }
                None => None,
            }
        }

        /// Copy the file path to the system clipboard.
        fn yank(&mut self) -> Option<Effect> {
            self._yank(false)
        }

        /// Copy the absolute file path to the system clipboard.
        fn really_yank(&mut self) -> Option<Effect> {
            self._yank(true)
        }

        fn _yank(&mut self, really: bool) -> Option<Effect> {
            if let Some(entry) = self.entry_path() {
                let mut path: String = entry.to_path_buf().to_string_lossy().to_string();
                if !really {
                    let dir_string: String = self.dir().to_string_lossy().to_string();
                    path = path.strip_prefix(&dir_string).unwrap().to_string();
                    if path.starts_with(PATH_SEPARATOR) {
                        path = path.strip_prefix(PATH_SEPARATOR).unwrap().to_string();
                    }
                }
                let mut clipboard = Clipboard::new();
                clipboard.copy(path);
            }
            None
        }

        fn handle_response(&mut self, response: Response) -> Option<Effect> {
            #[cfg(feature = "logging")]
            log::debug!("Handling response...");

            let pending_request: Uuid = match self.pending_request {
                Some(pending_request) => pending_request,
                None => {
                    #[cfg(feature = "logging")]
                    log::debug!("There is no pending request.");
                    return None;
                }
            };

            if !self.received_first_resp {
                self.hits = None;
                self.entries.clear();
                self.selected = None;
                self.offset = 0;
            }
            self.received_first_resp = true;

            if response.uuid() != &pending_request {
                #[cfg(feature = "logging")]
                log::debug!("The response is not for the pending request.");
                return None;
            }

            let params: &FindFilesResponseParams = match response.params() {
                ResponseParams::FindFiles(params) => params,
                _ => {
                    #[cfg(feature = "logging")]
                    log::error!("Unexpected response parameters.");
                    return None;
                }
            };

            self.entries.extend_from_slice(params.entries());

            if self.entries.is_empty() && response.last() {
                self.hits = Some(false);
                self.selected = None;
                return Some(Effect::Unfocus);
            }

            self.hits = Some(true);
            if self.selected.is_none() {
                self.selected = Some(0);
            }

            if response.last() {
                self.pending_request = None;
            }

            None
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Unfocus => self.unfocus(),
                Action::Find { phrase } => self.find(&phrase),
                Action::Resize { size } => self.resize(size),
                Action::Down => self.down(),
                Action::ReallyDown => self.really_down(),
                Action::Up => self.up(),
                Action::ReallyUp => self.really_up(),
                Action::Refresh => self.refresh(),
                Action::Edit => self.edit(),
                Action::Goto => self.goto(),
                Action::ReallyGoto => self.really_goto(),
                Action::Yank => self.yank(),
                Action::ReallyYank => self.really_yank(),
                Action::HandleResponse(response) => self.handle_response(response),
            }
        }
    }
}
use state::State;

mod action {
    use insh_api::Response;
    use rend::Size;

    pub enum Action {
        Unfocus,
        Find { phrase: String },
        Resize { size: Size },
        Down,
        ReallyDown,
        Up,
        ReallyUp,
        Refresh,
        Edit,
        Goto,
        ReallyGoto,
        Yank,
        ReallyYank,
        HandleResponse(Response),
    }
}
use action::Action;

mod effect {
    use crate::programs::VimArgs;

    use std::path::PathBuf;

    use uuid::Uuid;

    pub enum Effect {
        Unfocus,
        SendFindFilesRequest {
            uuid: Uuid,
            dir: PathBuf,
            pattern: String,
        },
        Goto {
            dir: PathBuf,
            file: Option<PathBuf>,
        },
        OpenVim(VimArgs),
        Bell,
    }
}
pub use effect::Effect;

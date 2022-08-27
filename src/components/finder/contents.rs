mod props {
    use crate::rendering::Size;
    use std::path::PathBuf;

    pub struct Props {
        pub directory: PathBuf,
        pub size: Size,
    }

    impl Props {
        pub fn new(directory: PathBuf, size: Size) -> Self {
            Self { directory, size }
        }
    }
}
pub use props::Props;

mod contents {
    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::component::Component;
    use crate::rendering::{Fabric, Size, Yarn};
    use crate::stateful::Stateful;

    use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

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
                Event::Resize { size } => Some(Action::Resize { size }),
                Event::Crossterm { event } => match event {
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }) => Some(Action::Unfocus),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        modifiers: KeyModifiers::NONE,
                    }) => Some(Action::Down),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('J'),
                        modifiers: KeyModifiers::SHIFT,
                    }) => Some(Action::ReallyDown),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        modifiers: KeyModifiers::NONE,
                    }) => Some(Action::Up),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('K'),
                        modifiers: KeyModifiers::SHIFT,
                    }) => Some(Action::ReallyUp),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('r'),
                        modifiers: KeyModifiers::NONE,
                    }) => Some(Action::Refresh),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('l'),
                        ..
                    })
                    | CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => Some(Action::Edit),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('g'),
                        modifiers: KeyModifiers::NONE,
                    }) => Some(Action::Goto),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('G'),
                        modifiers: KeyModifiers::SHIFT,
                    }) => Some(Action::ReallyGoto),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('y'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    }) => Some(Action::Yank),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('Y'),
                        modifiers: KeyModifiers::SHIFT,
                        ..
                    }) => Some(Action::ReallyYank),
                    _ => None,
                },
            };

            if let Some(action) = action {
                self.state.perform(action)
            } else {
                None
            }
        }

        fn render(&self, size: Size) -> Fabric {
            match self.state.hits() {
                Some(true) => {
                    let directory: &str = &self.state.directory().to_string_lossy();

                    let mut yarns: Vec<Yarn> = Vec::new();
                    for (entry, row) in self.state.visible_entries().iter().zip(0..size.rows) {
                        let path: &Path = entry.path();
                        let mut string: &str = &path.to_string_lossy();
                        string = string.strip_prefix(&directory).unwrap();
                        if string.starts_with(PATH_SEPARATOR) {
                            string = string.strip_prefix(PATH_SEPARATOR).unwrap();
                        }
                        let mut yarn: Yarn = Yarn::from(string);

                        let file_name_start: usize = yarn.len() - entry.file_name().len();

                        if self.state.focussed() && Some(row) == self.state.selected() {
                            yarn.color_before(Color::InvertedGrayyedText.into(), file_name_start);
                            yarn.color_after(Color::InvertedText.into(), file_name_start);
                            yarn.background(Color::Highlight.into());
                        } else {
                            yarn.color_before(Color::GrayyedText.into(), file_name_start);
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
    use crate::rendering::Size;
    use crossterm::event::Event as CrosstermEvent;

    pub enum Event {
        Find { phrase: String },
        Resize { size: Size },
        Crossterm { event: CrosstermEvent },
    }
}
pub use event::Event;

mod state {
    use super::{Action, Effect, Props};
    use crate::clipboard::Clipboard;
    use crate::path_finder::PathFinder;
    use crate::programs::{VimArgs, VimArgsBuilder};
    use crate::rendering::Size;
    use crate::stateful::Stateful;

    use std::cmp::{self, Ordering};
    use std::path::{Path, PathBuf, MAIN_SEPARATOR as PATH_SEPARATOR};

    use walkdir::DirEntry as Entry;

    pub struct State {
        size: Size,
        directory: PathBuf,
        phrase: Option<String>,
        focussed: bool,
        hits: Option<bool>,
        entries: Vec<Entry>,
        selected: Option<usize>,
        offset: usize,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                size: props.size,
                directory: props.directory,
                phrase: None,
                focussed: false,
                hits: None,
                entries: Vec::new(),
                selected: None,
                offset: 0,
            }
        }
    }

    impl State {
        pub fn directory(&self) -> &PathBuf {
            &self.directory
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

            // TODO: handle regex errors!
            let path_finder = PathFinder::new(&self.directory, phrase).unwrap();
            self.entries = path_finder.collect();
            self.offset = 0;

            if self.entries.is_empty() {
                self.selected = None;
                self.hits = Some(false);
                Some(Effect::Unfocus)
            } else {
                self.selected = Some(0);
                self.hits = Some(true);
                None
            }
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
                    let directory = entry.parent().unwrap().to_path_buf();
                    let file: Option<PathBuf> = if really {
                        Some(entry.to_path_buf())
                    } else {
                        None
                    };

                    Some(Effect::Goto { directory, file })
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
                    let directory_string: String = self.directory().to_string_lossy().to_string();
                    path = path.strip_prefix(&directory_string).unwrap().to_string();
                    if path.starts_with(PATH_SEPARATOR) {
                        path = path.strip_prefix(PATH_SEPARATOR).unwrap().to_string();
                    }
                }
                let mut clipboard = Clipboard::new();
                clipboard.copy(path);
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
            }
        }
    }
}
use state::State;

mod action {
    use crate::rendering::Size;

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
    }
}
use action::Action;

mod effect {
    use crate::programs::VimArgs;
    use std::path::PathBuf;

    pub enum Effect {
        Unfocus,
        Goto {
            directory: PathBuf,
            file: Option<PathBuf>,
        },
        OpenVim(VimArgs),
    }
}
pub use effect::Effect;

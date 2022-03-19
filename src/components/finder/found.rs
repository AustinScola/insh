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

mod found {
    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::component::Component;
    use crate::rendering::{Fabric, Size, Yarn};
    use crate::stateful::Stateful;

    use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

    pub struct Found {
        state: State,
    }

    impl Component<Props, Event, Effect> for Found {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Self { state }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Option<Action> = match event {
                Event::Find { phrase } => Some(Action::Find { phrase }),
                Event::Resize { size } => Some(Action::Resize { size }),
                Event::CrosstermEvent { event } => match event {
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }) => Some(Action::Unfocus),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    }) => Some(Action::Down),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    }) => Some(Action::Up),
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
                        ..
                    }) => Some(Action::Goto),
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
                    let mut yarns: Vec<Yarn> = Vec::new();
                    for (entry, row) in self.state.visible_entries().iter().zip(0..size.rows) {
                        let string: String = entry
                            .path()
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string();
                        let mut yarn = Yarn::from(string);

                        if self.state.focussed() && Some(row) == self.state.selected() {
                            yarn.color(Color::InvertedText.into());
                            yarn.background(Color::Highlight.into());
                        }
                        yarns.push(yarn);
                    }
                    Fabric::from(yarns)
                }
                Some(false) => {
                    let mut yarn = Yarn::from("No matching files.");
                    yarn.pad(size.columns);
                    let mut fabric = Fabric::from(vec![yarn]);
                    fabric.pad(size.rows);
                    fabric
                }
                None => Fabric::new(size),
            }
        }
    }
}
pub use found::Found;

mod event {
    use crate::rendering::Size;
    use crossterm::event::Event as CrosstermEvent;

    pub enum Event {
        Find { phrase: String },
        Resize { size: Size },
        CrosstermEvent { event: CrosstermEvent },
    }
}
pub use event::Event;

mod state {
    use super::{Action, Effect, Props};
    use crate::path_finder::PathFinder;
    use crate::rendering::Size;
    use crate::stateful::Stateful;

    use std::cmp::{self, Ordering};
    use std::fs::DirEntry;
    use std::path::PathBuf;

    pub struct State {
        size: Size,
        directory: PathBuf,
        focussed: bool,
        hits: Option<bool>,
        entries: Vec<DirEntry>,
        selected: Option<usize>,
        offset: usize,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                size: props.size,
                directory: props.directory,
                focussed: false,
                hits: None,
                entries: Vec::new(),
                selected: None,
                offset: 0,
            }
        }
    }

    impl State {
        pub fn focussed(&self) -> bool {
            self.focussed
        }

        pub fn hits(&self) -> Option<bool> {
            self.hits
        }

        pub fn entries(&self) -> &Vec<DirEntry> {
            &self.entries
        }

        pub fn visible_entries(&self) -> &[DirEntry] {
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

        fn entry_path(&self) -> Option<PathBuf> {
            match self.entry_number() {
                Some(entry_number) => Some(self.entries[entry_number].path()),
                None => None,
            }
        }

        fn resize(&mut self, new_size: Size) -> Option<Effect> {
            match self.selected {
                Some(selected) => {
                    let rows_before = self.size.rows;
                    let entry_count = self.entries.len();
                    let mut visible_entries_count =
                        cmp::min(rows_before, entry_count - self.offset);
                    let selected_percent: f64 = selected as f64 / visible_entries_count as f64;

                    let mut new_selected: usize =
                        (new_size.rows as f64 * selected_percent) as usize;
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
                                let bottom_pinned_offset =
                                    entry_count.saturating_sub(new_size.rows);
                                let difference = new_offset - bottom_pinned_offset;
                                new_selected += difference;
                                new_offset = bottom_pinned_offset;
                            }
                        }
                    }

                    self.offset = new_offset;
                    self.selected = Some(new_selected);
                }
                None => {}
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

        fn find(&mut self, phrase: String) -> Option<Effect> {
            self.focus();

            // TODO: handle regex errors!
            let path_finder = PathFinder::new(&self.directory, &phrase).unwrap();
            self.entries = path_finder.collect();

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

        fn edit(&mut self) -> Option<Effect> {
            match self.entry_path() {
                Some(path) => Some(Effect::OpenVim { file: path }),
                None => None,
            }
        }

        fn goto(&mut self) -> Option<Effect> {
            Some(Effect::Goto {
                directory: self.directory.clone(),
            })
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Unfocus => self.unfocus(),
                Action::Find { phrase } => self.find(phrase),
                Action::Resize { size } => self.resize(size),
                Action::Down => self.down(),
                Action::Up => self.up(),
                Action::Edit => self.edit(),
                Action::Goto => self.goto(),
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
        Up,
        Edit,
        Goto,
    }
}
use action::Action;

mod effect {
    use std::path::PathBuf;

    pub enum Effect {
        Unfocus,
        Goto { directory: PathBuf },
        OpenVim { file: PathBuf },
    }
}
pub use effect::Effect;

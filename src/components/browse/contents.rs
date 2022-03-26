use crate::color::Color;
use crate::component::Component;
use crate::rendering::{Fabric, Size, Yarn};
use crate::stateful::Stateful;

use std::cmp;
use std::cmp::Ordering;
use std::env;
use std::fs::{self, DirEntry};
use std::path::PathBuf;

use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent};

pub struct Props {
    directory: PathBuf,
    size: Size,
}

impl Props {
    pub fn new(directory: PathBuf, size: Size) -> Self {
        Self { directory, size }
    }
}

pub struct Contents {
    state: State,
}

impl Component<Props, Event, Effect> for Contents {
    fn new(props: Props) -> Self {
        let state = State::from(props);
        Self { state }
    }

    fn handle(&mut self, event: Event) -> Option<Effect> {
        if let Some(action) = self.map(event) {
            return self.state.perform(action);
        }
        None
    }

    fn render(&self, size: Size) -> Fabric {
        let mut yarns: Vec<Yarn> = Vec::new();
        for (entry, row) in self.state.visible_entries().iter().zip(0..size.rows) {
            let mut string: String;
            {
                let path = entry.path();
                string = path.file_name().unwrap().to_str().unwrap().to_string();
                if path.is_dir() {
                    string.push('/');
                }
            }
            let mut yarn = Yarn::from(string);

            if Some(row) == self.state.selected {
                yarn.color(Color::InvertedText.into());
                yarn.background(Color::Highlight.into());
            }
            yarns.push(yarn);
        }
        Fabric::from(yarns)
    }
}

impl Contents {
    fn map(&self, event: Event) -> Option<Action> {
        match event {
            Event::Resize { size } => Some(Action::Resize { size }),
            Event::Pop => Some(Action::Pop),
            Event::CrosstermEvent { event } => {
                if let CrosstermEvent::Key(key_event) = event {
                    match key_event {
                        KeyEvent {
                            code: KeyCode::Char('j'),
                            ..
                        } => Some(Action::Down),
                        KeyEvent {
                            code: KeyCode::Char('k'),
                            ..
                        } => Some(Action::Up),
                        KeyEvent {
                            code: KeyCode::Char('r'),
                            ..
                        } => Some(Action::Refresh),
                        KeyEvent {
                            code: KeyCode::Char('l'),
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Enter,
                            ..
                        } => Some(Action::Push),
                        KeyEvent {
                            code: KeyCode::Char('h'),
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Backspace,
                            ..
                        } => Some(Action::Pop),
                        KeyEvent {
                            code: KeyCode::Char('b'),
                            ..
                        } => Some(Action::RunBash),
                        KeyEvent {
                            code: KeyCode::Char('f'),
                            ..
                        } => Some(Action::OpenFinder),
                        _ => None,
                    }
                } else {
                    None
                }
            }
        }
    }
}

pub enum Event {
    Resize { size: Size },
    Pop,
    CrosstermEvent { event: CrosstermEvent },
}

struct State {
    size: Size,
    directory: PathBuf,
    entries: Vec<DirEntry>,
    selected: Option<usize>,
    offset: usize,
}

impl State {
    pub fn from(props: Props) -> Self {
        let size = props.size;
        let directory: PathBuf = props.directory;
        let entries = State::get_entries(&directory);
        let selected = if !entries.is_empty() { Some(0) } else { None };
        let offset = 0;
        State {
            size,
            directory,
            entries,
            selected,
            offset,
        }
    }

    fn get_entries(directory: &PathBuf) -> Vec<DirEntry> {
        let entries_iter = fs::read_dir(directory.as_path()).unwrap();
        let mut entries = Vec::from_iter(entries_iter.map(|entry| entry.unwrap()));
        entries.sort_unstable_by_key(|a| a.file_name());
        entries
    }

    fn visible_entries(&self) -> &[DirEntry] {
        if self.entries.is_empty() {
            return &[];
        }
        let start = self.offset;
        let end = cmp::min(self.offset + self.size.rows, self.entries.len());
        &self.entries[start..end]
    }

    fn entry_number(&self) -> Option<usize> {
        self.selected.map(|selected| self.offset + selected)
    }

    fn entry(&self) -> Option<&DirEntry> {
        match self.entry_number() {
            Some(entry_number) => Some(&self.entries[entry_number]),
            None => None,
        }
    }

    fn set_directory(&mut self, directory: &PathBuf) {
        self.directory = directory.to_path_buf();
        self.reset_entries();
    }

    fn reset_entries(&mut self) {
        self.entries = State::get_entries(&self.directory);
        self.selected = if !self.entries.is_empty() {
            Some(0)
        } else {
            None
        };
        self.offset = 0;
    }

    fn resize(&mut self, new_size: Size) {
        match self.selected {
            Some(selected) => {
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
            None => {}
        }

        self.size = new_size;
    }

    fn down(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        let entry_number = self.entry_number().unwrap();
        if entry_number >= self.entries.len() - 1 {
            return;
        }
        let selected = self.selected.unwrap();
        if selected < self.size.rows - 1 {
            self.selected = Some(selected + 1);
        } else {
            self.offset += 1;
        }
    }

    fn up(&mut self) {
        if let Some(selected) = self.selected {
            if selected > 0 {
                self.selected = Some(selected.saturating_sub(1))
            } else {
                self.offset = self.offset.saturating_sub(1);
            }
        }
    }

    /// Refresh the contents of the browser to reflect the current state of the file system.
    fn refresh(&mut self) {
        // TODO: Maintain the currently selected entry (if possible) and maintain the currently
        // selected scroll position (if possible).
        self.reset_entries();
    }

    fn push(&mut self) -> Option<Effect> {
        if let Some(entry) = self.entry() {
            let path = entry.path();
            if path.is_dir() {
                self.set_directory(&path);
                return Some(Effect::SetDirectory { directory: path });
            }

            if path.is_file() {
                return Some(Effect::OpenVim { file: path });
            }
        }
        None
    }

    fn pop(&mut self) -> Option<Effect> {
        let popped: bool = self.directory.pop();
        if popped {
            self.reset_entries();
            return Some(Effect::SetDirectory {
                directory: self.directory.clone(),
            });
        }
        None
    }

    fn open_finder(&self) -> Option<Effect> {
        Some(Effect::OpenFinder {
            directory: self.directory.clone(),
        })
    }

    fn run_bash(&self) -> Option<Effect> {
        Some(Effect::RunBash {
            directory: self.directory.clone(),
        })
    }
}

impl Stateful<Action, Effect> for State {
    fn perform(&mut self, action: Action) -> Option<Effect> {
        let mut effect: Option<Effect> = None;
        match action {
            Action::Resize { size } => {
                self.resize(size);
            }
            Action::Down => {
                self.down();
            }
            Action::Up => {
                self.up();
            }
            Action::Refresh => {
                self.refresh();
            }
            Action::Push => {
                effect = self.push();
            }
            Action::Pop => {
                effect = self.pop();
            }
            Action::OpenFinder => {
                effect = self.open_finder();
            }
            Action::RunBash => {
                effect = self.run_bash();
            }
        }
        effect
    }
}

enum Action {
    Resize { size: Size },
    Down,
    Up,
    Refresh,
    Push,
    Pop,
    OpenFinder,
    RunBash,
}

pub enum Effect {
    SetDirectory { directory: PathBuf },
    OpenFinder { directory: PathBuf },
    OpenVim { file: PathBuf },
    RunBash { directory: PathBuf },
}

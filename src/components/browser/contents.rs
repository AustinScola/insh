use crate::color::Color;
use crate::component::Component;
use crate::programs::{VimArgs, VimArgsBuilder};
use crate::rendering::{Fabric, Size, Yarn};
use crate::stateful::Stateful;

use std::cmp::{self, Ordering};
use std::fs::{self, DirEntry};
use std::io::ErrorKind as IOErrorKind;
use std::path::{Path, PathBuf};

use crate::clipboard::Clipboard;
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

pub struct Props {
    directory: PathBuf,
    size: Size,
    file: Option<PathBuf>,
}

impl Props {
    pub fn new(directory: PathBuf, size: Size, file: Option<PathBuf>) -> Self {
        Self {
            directory,
            size,
            file,
        }
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
        match self.map(event) {
            Some(action) => self.state.perform(action),
            None => Some(Effect::Bell),
        }
    }

    fn render(&self, size: Size) -> Fabric {
        match self.state.entries() {
            Ok(_) => {
                let visible_entries = self.state.visible_entries().unwrap();
                if visible_entries.is_empty() {
                    return Fabric::center("The directory is empty.", size);
                }

                let mut yarns: Vec<Yarn> = Vec::new();
                for (entry, row) in visible_entries.iter().zip(0..size.rows) {
                    let mut string: String;
                    {
                        let path = entry.path();
                        string = path.file_name().unwrap().to_str().unwrap().to_string();
                        if path.is_dir() {
                            string.push('/');
                        }
                    }

                    let hidden = string.starts_with('.');

                    let mut yarn = Yarn::from(string);

                    if Some(row) == self.state.selected {
                        yarn.color(Color::InvertedText.into());
                        yarn.background(Color::Highlight.into());
                    } else if hidden {
                        yarn.color(Color::LightGrayedText.into());
                    }
                    yarn.resize(size.columns);
                    yarns.push(yarn);
                }

                let mut fabric = Fabric::from(yarns);
                fabric.pad_bottom(size.rows);

                fabric
            }
            Err(error) => Fabric::center(&error.to_string(), size),
        }
    }
}

impl Contents {
    fn map(&self, event: Event) -> Option<Action> {
        match event {
            Event::Resize { size } => Some(Action::Resize { size }),
            Event::Crossterm { event } => {
                if let CrosstermEvent::Key(key_event) = event {
                    match key_event {
                        KeyEvent {
                            code: KeyCode::Char('j'),
                            modifiers: KeyModifiers::NONE,
                        } => Some(Action::Down),
                        KeyEvent {
                            code: KeyCode::Char('J'),
                            modifiers: KeyModifiers::SHIFT,
                        } => Some(Action::ReallyDown),
                        KeyEvent {
                            code: KeyCode::Char('k'),
                            modifiers: KeyModifiers::NONE,
                        } => Some(Action::Up),
                        KeyEvent {
                            code: KeyCode::Char('K'),
                            modifiers: KeyModifiers::SHIFT,
                        } => Some(Action::ReallyUp),
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
                            code: KeyCode::Char('y'),
                            modifiers: KeyModifiers::NONE,
                        } => Some(Action::Yank),
                        KeyEvent {
                            code: KeyCode::Char('Y'),
                            modifiers: KeyModifiers::SHIFT,
                        } => Some(Action::ReallyYank),
                        KeyEvent {
                            code: KeyCode::Char('b'),
                            ..
                        } => Some(Action::RunBash),
                        KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: KeyModifiers::NONE,
                        } => Some(Action::OpenFileCreator),
                        KeyEvent {
                            code: KeyCode::Char('f'),
                            ..
                        } => Some(Action::OpenFinder),
                        KeyEvent {
                            code: KeyCode::Char('s'),
                            ..
                        } => Some(Action::OpenSearcher),
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
    Crossterm { event: CrosstermEvent },
}

struct State {
    size: Size,
    directory: PathBuf,

    /// The directory entries (if they can be read).
    entries: EntriesResult,

    selected: Option<usize>,
    offset: usize,
}

impl From<Props> for State {
    fn from(props: Props) -> Self {
        let size = props.size;
        let directory: PathBuf = props.directory;
        let entries = State::get_entries(&directory);

        let selected;
        let offset;
        if let Ok(entries) = &entries {
            if entries.is_empty() {
                selected = None;
                offset = 0;
            } else if let Some(file) = props.file {
                let index = entries.iter().position(|entry| entry.path() == file);
                match index {
                    Some(index) => {
                        if index < size.rows {
                            selected = Some(index);
                            offset = 0;
                        } else {
                            selected = Some(0);
                            offset = index;
                        }
                    }
                    None => {
                        selected = Some(0);
                        offset = 0;
                    }
                }
            } else {
                selected = if !entries.is_empty() { Some(0) } else { None };
                offset = 0;
            }
        } else {
            selected = Some(0);
            offset = 0;
        }

        State {
            size,
            directory,
            entries,
            selected,
            offset,
        }
    }
}

impl State {
    fn get_entries(directory: &Path) -> EntriesResult {
        match fs::read_dir(directory) {
            Ok(entries_iter) => {
                let mut entries = Vec::from_iter(entries_iter.map(|entry| entry.unwrap()));
                entries.sort_unstable_by_key(|a| a.file_name());
                Ok(entries)
            }
            Err(error) => match error.kind() {
                IOErrorKind::NotFound => Err(GetEntriesError::DirectoryDoesNotExist),
                IOErrorKind::PermissionDenied => Err(GetEntriesError::PermissionDenied),
                _ => Err(GetEntriesError::OtherErrorReading),
            },
        }
    }

    /// Return the entries of the directory.
    pub fn entries(&self) -> &EntriesResult {
        &self.entries
    }

    fn visible_entries(&self) -> Option<&[DirEntry]> {
        let entries: &Vec<DirEntry> = match &self.entries {
            Ok(entries) => entries,
            Err(_) => {
                return None;
            }
        };

        if entries.is_empty() {
            return Some(&[]);
        }

        let start = self.offset;
        let end = cmp::min(self.offset + self.size.rows, entries.len());
        Some(&entries[start..end])
    }

    fn entry_number(&self) -> Option<usize> {
        self.selected.map(|selected| self.offset + selected)
    }

    fn entry(&self) -> Option<&DirEntry> {
        match self.entry_number() {
            Some(entry_number) => match &self.entries {
                Ok(entries) => Some(&entries[entry_number]),
                Err(_) => None,
            },
            None => None,
        }
    }

    fn set_directory(&mut self, directory: &Path) {
        self.directory = directory.to_path_buf();
        self.reset_entries();
    }

    fn reset_entries(&mut self) {
        self.entries = State::get_entries(&self.directory);

        self.selected = if let Ok(entries) = &self.entries {
            if !entries.is_empty() {
                Some(0)
            } else {
                None
            }
        } else {
            None
        };

        self.offset = 0;
    }

    fn resize(&mut self, new_size: Size) -> Option<Effect> {
        if let Some(selected) = self.selected {
            if let Ok(entries) = &self.entries {
                let rows_before = self.size.rows;
                let entry_count = entries.len();
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
        }

        self.size = new_size;

        None
    }

    fn down(&mut self) -> Option<Effect> {
        let entries: &Vec<DirEntry> = match &self.entries {
            Ok(entries) => entries,
            Err(_) => {
                return None;
            }
        };

        if entries.is_empty() {
            return None;
        }

        let entry_number = self.entry_number().unwrap();
        if entry_number >= entries.len() - 1 {
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

    /// Select the last entry and adjust the scroll position if necessary.
    fn really_down(&mut self) -> Option<Effect> {
        let entries: &Vec<DirEntry> = match &self.entries {
            Ok(entries) => entries,
            Err(_) => {
                return None;
            }
        };

        if entries.is_empty() {
            return None;
        }

        if entries.len() > self.size.rows {
            self.offset = entries.len() - self.size.rows;
            self.selected = Some(self.size.rows - 1);
        } else {
            self.selected = Some(entries.len() - 1);
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

    /// Select the first entry and adjust the scroll position if necessary.
    fn really_up(&mut self) -> Option<Effect> {
        self.offset = 0;
        self.selected = Some(0);
        None
    }

    /// Refresh the contents of the browser to reflect the current state of the file system.
    fn refresh(&mut self) -> Option<Effect> {
        // TODO: Maintain the currently selected entry (if possible) and maintain the currently
        // selected scroll position (if possible).
        self.reset_entries();
        None
    }

    fn push(&mut self) -> Option<Effect> {
        if let Some(entry) = self.entry() {
            let path = entry.path();
            if path.is_dir() {
                self.set_directory(&path);
                return Some(Effect::SetDirectory { directory: path });
            }

            if path.is_file() {
                let vim_args: VimArgs = VimArgsBuilder::new().path(&path).build();
                return Some(Effect::OpenVim(vim_args));
            }
        }
        None
    }

    fn pop(&mut self) -> Option<Effect> {
        let popped: bool = self.directory.pop();
        if popped {
            self.reset_entries();
            return Some(Effect::PopDirectory);
        }
        None
    }

    /// Copy the file name of the selected entry to the clipboard.
    ///
    /// If the entry is a directory, a trailing slash is added.
    fn yank(&self) -> Option<Effect> {
        let entry: &DirEntry = match self.entry() {
            Some(entry) => entry,
            None => {
                return None;
            }
        };

        let mut contents: String = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() {
            contents.push('/');
        }

        let mut clipboard = Clipboard::new();
        clipboard.copy(contents);

        None
    }

    /// Copy the path of the selected entry to the clipboard.
    ///
    /// If the entry is a directory, a trailing slash is added.
    fn really_yank(&self) -> Option<Effect> {
        let entry: &DirEntry = match self.entry() {
            Some(entry) => entry,
            None => {
                return None;
            }
        };

        let path: PathBuf = entry.path();
        let mut contents: String = path.to_string_lossy().to_string();
        if path.is_dir() {
            contents.push('/');
        }

        let mut clipboard = Clipboard::new();
        clipboard.copy(contents);

        None
    }

    fn open_file_creator(&self) -> Option<Effect> {
        Some(Effect::OpenFileCreator {
            directory: self.directory.clone(),
        })
    }

    fn open_finder(&self) -> Option<Effect> {
        Some(Effect::OpenFinder {
            directory: self.directory.clone(),
        })
    }

    fn open_searcher(&self) -> Option<Effect> {
        Some(Effect::OpenSearcher {
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
        match action {
            Action::Resize { size } => self.resize(size),
            Action::Down => self.down(),
            Action::ReallyDown => self.really_down(),
            Action::Up => self.up(),
            Action::ReallyUp => self.really_up(),
            Action::Refresh => self.refresh(),
            Action::Push => self.push(),
            Action::Pop => self.pop(),
            Action::Yank => self.yank(),
            Action::ReallyYank => self.really_yank(),
            Action::OpenFileCreator => self.open_file_creator(),
            Action::OpenFinder => self.open_finder(),
            Action::OpenSearcher => self.open_searcher(),
            Action::RunBash => self.run_bash(),
        }
    }
}

type EntriesResult = Result<Vec<DirEntry>, GetEntriesError>;

enum GetEntriesError {
    DirectoryDoesNotExist,
    PermissionDenied,
    OtherErrorReading,
}

impl ToString for GetEntriesError {
    fn to_string(&self) -> String {
        match self {
            Self::DirectoryDoesNotExist => String::from("The directory does not exist."),
            Self::PermissionDenied => String::from("Permission denied."),
            Self::OtherErrorReading => String::from("Failed to read the directory entries."),
        }
    }
}

enum Action {
    Resize { size: Size },
    Down,
    ReallyDown,
    Up,
    ReallyUp,
    Refresh,
    Push,
    Pop,
    Yank,
    ReallyYank,
    OpenFileCreator,
    OpenFinder,
    OpenSearcher,
    RunBash,
}

pub enum Effect {
    SetDirectory { directory: PathBuf },
    PopDirectory,
    OpenFileCreator { directory: PathBuf },
    OpenFinder { directory: PathBuf },
    OpenSearcher { directory: PathBuf },
    OpenVim(VimArgs),
    RunBash { directory: PathBuf },
    Bell,
}

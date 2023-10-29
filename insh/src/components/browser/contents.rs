use std::cmp::{self, Ordering};
use std::path::{Path, PathBuf};

use typed_builder::TypedBuilder;
use uuid::Uuid;

use file_info::FileInfo;
use file_type::FileType;
use insh_api::{
    GetFilesRequestParams, GetFilesResponseParams, GetFilesResult, Request, RequestParams,
    Response, ResponseParams,
};
use rend::{Fabric, Size, Yarn};
use term::{Key, KeyEvent, KeyMods, TermEvent};
use til::Component;

use crate::clipboard::Clipboard;
use crate::color::Color;
use crate::programs::{VimArgs, VimArgsBuilder};
use crate::stateful::Stateful;

#[derive(TypedBuilder)]
pub struct Props {
    dir: PathBuf,
    size: Size,
    file: Option<PathBuf>,
    pending_request: Option<Uuid>,
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
        match self.state.file_infos() {
            None => Fabric::new(size),
            Some(file_infos) => match file_infos {
                Ok(_) => {
                    let visible_file_infos = self.state.visible_file_infos().unwrap();
                    if visible_file_infos.is_empty() {
                        return Fabric::center("The directory is empty.", size);
                    }

                    let mut yarns: Vec<Yarn> = Vec::new();
                    for (entry, row) in visible_file_infos.iter().zip(0..size.rows) {
                        let mut string: String;
                        {
                            string = entry.name().unwrap().to_str().unwrap().to_string();
                            if let Ok(r#type) = entry.r#type() {
                                if r#type.is_dir() {
                                    string.push('/');
                                }
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
            },
        }
    }
}

impl Contents {
    fn map(&self, event: Event) -> Option<Action> {
        match event {
            Event::Response(response) => Some(Action::HandleResponse(response)),
            Event::Resize { size } => Some(Action::Resize { size }),
            Event::Term { event } => {
                if let TermEvent::KeyEvent(key_event) = event {
                    match key_event {
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
                            ..
                        } => Some(Action::Refresh),
                        KeyEvent {
                            key: Key::Char('l'),
                            ..
                        }
                        | KeyEvent {
                            key: Key::CarriageReturn,
                            ..
                        } => Some(Action::Push),
                        KeyEvent {
                            key: Key::Char('h'),
                            ..
                        }
                        | KeyEvent {
                            key: Key::Backspace,
                            ..
                        } => Some(Action::Pop),
                        KeyEvent {
                            key: Key::Char('y'),
                            mods: KeyMods::NONE,
                        } => Some(Action::Yank),
                        KeyEvent {
                            key: Key::Char('Y'),
                            mods: KeyMods::SHIFT,
                        } => Some(Action::ReallyYank),
                        KeyEvent {
                            key: Key::Char('b'),
                            ..
                        } => Some(Action::RunBash),
                        KeyEvent {
                            key: Key::Char('c'),
                            mods: KeyMods::NONE,
                        } => Some(Action::OpenFileCreator {
                            file_type: FileType::File,
                        }),
                        KeyEvent {
                            key: Key::Char('C'),
                            mods: KeyMods::SHIFT,
                        } => Some(Action::OpenFileCreator {
                            file_type: FileType::Dir,
                        }),
                        KeyEvent {
                            key: Key::Char('f'),
                            ..
                        } => Some(Action::OpenFinder),
                        KeyEvent {
                            key: Key::Char('s'),
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
    Response(Response),
    Resize { size: Size },
    Term { event: TermEvent },
}

struct State {
    size: Size,
    dir: PathBuf,

    starting_file: Option<PathBuf>,
    pending_request: Option<Uuid>,

    /// The dir entries (if they can be read).
    file_infos: Option<GetFilesResult>,

    selected: Option<usize>,
    offset: usize,
}

impl From<Props> for State {
    fn from(props: Props) -> Self {
        let size = props.size;
        let dir: PathBuf = props.dir;

        State {
            size,
            dir,
            starting_file: props.file,
            pending_request: props.pending_request,
            file_infos: None,
            selected: None,
            offset: 0,
        }
    }
}

impl State {
    /// Return the entries of the dir.
    pub fn file_infos(&self) -> &Option<GetFilesResult> {
        &self.file_infos
    }

    fn visible_file_infos(&self) -> Option<&[FileInfo]> {
        let file_infos: &GetFilesResult = match &self.file_infos {
            Some(file_infos) => file_infos,
            None => {
                return None;
            }
        };

        let file_infos: &Vec<FileInfo> = match file_infos {
            Ok(file_infos) => file_infos,
            Err(_) => {
                return None;
            }
        };

        if file_infos.is_empty() {
            return Some(&[]);
        }

        let start = self.offset;
        let end = cmp::min(self.offset + self.size.rows, file_infos.len());
        Some(&file_infos[start..end])
    }

    fn entry_number(&self) -> Option<usize> {
        self.selected.map(|selected| self.offset + selected)
    }

    fn entry(&self) -> Option<&FileInfo> {
        let file_infos: &GetFilesResult = match &self.file_infos {
            Some(file_infos) => file_infos,
            None => {
                return None;
            }
        };

        match self.entry_number() {
            Some(entry_number) => match file_infos {
                Ok(file_infos) => Some(&file_infos[entry_number]),
                Err(_) => None,
            },
            None => None,
        }
    }

    fn set_dir(&mut self, dir: &Path) -> Option<Effect> {
        self.dir = dir.to_path_buf();
        None
    }

    fn reset_file_infos(&mut self) {
        self.file_infos = None;
        self.selected = None;
        self.offset = 0;
    }

    fn resize(&mut self, new_size: Size) -> Option<Effect> {
        if let Some(selected) = self.selected {
            if let Some(Ok(file_infos)) = &self.file_infos {
                let rows_before = self.size.rows;
                let entry_count = file_infos.len();
                let mut visible_file_infos_count = cmp::min(rows_before, entry_count - self.offset);
                let selected_percent: f64 = selected as f64 / visible_file_infos_count as f64;

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
                        visible_file_infos_count = entry_count - new_offset;
                        if visible_file_infos_count < new_size.rows {
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
        let file_infos: &GetFilesResult = match &self.file_infos {
            Some(file_infos) => file_infos,
            None => {
                return None;
            }
        };

        let file_infos: &Vec<FileInfo> = match file_infos {
            Ok(file_infos) => file_infos,
            Err(_) => {
                return None;
            }
        };

        if file_infos.is_empty() {
            return None;
        }

        let entry_number = self.entry_number().unwrap();
        if entry_number >= file_infos.len() - 1 {
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
        let file_infos: &GetFilesResult = match &self.file_infos {
            Some(file_infos) => file_infos,
            None => {
                return None;
            }
        };

        let file_infos: &Vec<FileInfo> = match file_infos {
            Ok(file_infos) => file_infos,
            Err(_) => {
                return None;
            }
        };

        if file_infos.is_empty() {
            return None;
        }

        if file_infos.len() > self.size.rows {
            self.offset = file_infos.len() - self.size.rows;
            self.selected = Some(self.size.rows - 1);
        } else {
            self.selected = Some(file_infos.len() - 1);
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

        self.reset_file_infos();

        let request = Request::builder()
            .params(RequestParams::GetFiles(
                GetFilesRequestParams::builder()
                    .dir(self.dir.clone())
                    .build(),
            ))
            .build();
        self.pending_request = Some(*request.uuid());
        Some(Effect::Request(request))
    }

    fn push(&mut self) -> Option<Effect> {
        if let Some(entry) = self.entry() {
            let path: PathBuf = entry.path().to_path_buf();
            if path.is_dir() {
                self.set_dir(&path);

                let request = Request::builder()
                    .params(RequestParams::GetFiles(
                        GetFilesRequestParams::builder()
                            .dir(self.dir.clone())
                            .build(),
                    ))
                    .build();
                self.pending_request = Some(*request.uuid());

                return Some(Effect::SetDir {
                    dir: path.to_path_buf(),
                    get_files_request: request,
                });
            }

            if path.is_file() {
                let vim_args: VimArgs = VimArgsBuilder::new().path(&path).build();
                return Some(Effect::OpenVim(vim_args));
            }
        }
        None
    }

    fn pop(&mut self) -> Option<Effect> {
        let popped: bool = self.dir.pop();
        if popped {
            self.reset_file_infos();

            let request = Request::builder()
                .params(RequestParams::GetFiles(
                    GetFilesRequestParams::builder()
                        .dir(self.dir.clone())
                        .build(),
                ))
                .build();
            self.pending_request = Some(*request.uuid());

            return Some(Effect::PopDir {
                get_files_request: request,
            });
        }
        None
    }

    /// Copy the file name of the selected entry to the clipboard.
    ///
    /// If the entry is a directory, a trailing slash is added.
    fn yank(&self) -> Option<Effect> {
        let entry: &FileInfo = match self.entry() {
            Some(entry) => entry,
            None => {
                return None;
            }
        };

        let mut contents: String = entry.name().unwrap().to_string_lossy().to_string();
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
        let entry: &FileInfo = match self.entry() {
            Some(entry) => entry,
            None => {
                return None;
            }
        };

        let path: PathBuf = entry.path().to_path_buf();
        let mut contents: String = path.to_string_lossy().to_string();
        if path.is_dir() {
            contents.push('/');
        }

        let mut clipboard = Clipboard::new();
        clipboard.copy(contents);

        None
    }

    fn open_file_creator(&self, file_type: FileType) -> Option<Effect> {
        Some(Effect::OpenFileCreator {
            dir: self.dir.clone(),
            file_type,
        })
    }

    fn open_finder(&self) -> Option<Effect> {
        Some(Effect::OpenFinder {
            dir: self.dir.clone(),
        })
    }

    fn open_searcher(&self) -> Option<Effect> {
        Some(Effect::OpenSearcher {
            dir: self.dir.clone(),
        })
    }

    fn run_bash(&self) -> Option<Effect> {
        Some(Effect::RunBash {
            dir: self.dir.clone(),
        })
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

        if response.uuid() != &pending_request {
            #[cfg(feature = "logging")]
            log::debug!("The response is not for the pending request.");
            return None;
        }

        let params: &GetFilesResponseParams = match response.params() {
            ResponseParams::GetFiles(params) => params,
            _ => {
                #[cfg(feature = "logging")]
                log::error!("Unexpected response parameters.");
                return None;
            }
        };

        self.file_infos = Some(params.result().clone());

        // Adjust the selected entry and offset.
        let selected;
        let offset;
        if let Some(Ok(file_infos)) = &self.file_infos {
            if file_infos.is_empty() {
                selected = None;
                offset = 0;
            } else if let Some(file) = &self.starting_file {
                let index = file_infos.iter().position(|entry| entry.path() == file);
                match index {
                    Some(index) => {
                        if index < self.size.rows {
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
                selected = if !file_infos.is_empty() {
                    Some(0)
                } else {
                    None
                };
                offset = 0;
            }
        } else {
            selected = Some(0);
            offset = 0;
        }
        self.selected = selected;
        self.offset = offset;

        self.starting_file = None;

        None
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
            Action::OpenFileCreator { file_type } => self.open_file_creator(file_type),
            Action::OpenFinder => self.open_finder(),
            Action::OpenSearcher => self.open_searcher(),
            Action::RunBash => self.run_bash(),
            Action::HandleResponse(response) => self.handle_response(response),
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
    OpenFileCreator { file_type: FileType },
    OpenFinder,
    OpenSearcher,
    RunBash,
    HandleResponse(Response),
}

pub enum Effect {
    SetDir {
        dir: PathBuf,
        // NOTE: We only jam this in here for now because we can only emit a single effect right
        // now.
        get_files_request: Request,
    },
    PopDir {
        // NOTE: We only jam this in here for now because we can only emit a single effect right
        // now.
        get_files_request: Request,
    },
    OpenFileCreator {
        dir: PathBuf,
        file_type: FileType,
    },
    OpenFinder {
        dir: PathBuf,
    },
    OpenSearcher {
        dir: PathBuf,
    },
    OpenVim(VimArgs),
    RunBash {
        dir: PathBuf,
    },
    Bell,
    Request(Request),
}

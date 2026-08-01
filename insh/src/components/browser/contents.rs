use std::cmp::{self, Ordering};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use typed_builder::TypedBuilder;
use uuid::Uuid;

use file_info::FileInfo;
use file_type::FileType;
use insh_api::{GetFilesResponseParams, GetFilesResult, Request, Response, ResponseParams};
use rend::{Fabric, Size, Yarn};
use term::{Key, KeyEvent, KeyMods, TermEvent};
use til::Component;

use crate::clipboard::Clipboard;
use crate::color::Color;
use crate::config::Config;
use crate::programs::{VimArgs, VimArgsBuilder};
use crate::stateful::Stateful;
use crate::utils::get_files_request;

/// Contains functionality for rendering the entries of a directory.
mod entry {
    use file_info::{FileInfo, FileMetadata};
    use human_friendly::{
        human_friendly_file_mode, human_friendly_file_time, FILE_MODE_WIDTH, FILE_TIME_WIDTH,
    };

    /// The string used to separate the name of a symlink from the name of its target.
    const LINK_ARROW: &str = " -> ";

    /// The character used for metadata which is not known.
    const UNKNOWN: char = '?';

    /// Return the name of the entry (with a trailing slash if the entry is a directory).
    pub fn name(file_info: &FileInfo) -> String {
        let mut name: String = match file_info.name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => String::new(),
        };

        if let Ok(r#type) = file_info.r#type() {
            if r#type.is_dir() {
                name.push('/');
            }
        }

        name
    }

    /// Return the name of the entry along with the target of the entry if it is a symlink.
    pub fn name_and_link_target(file_info: &FileInfo) -> String {
        let mut string: String = name(file_info);

        if let Some(link_target) = file_info.metadata().and_then(FileMetadata::link_target) {
            string.push_str(LINK_ARROW);
            string.push_str(&link_target.to_string_lossy());
        }

        string
    }

    /// Return the name of the user which owns the entry (or the id of the user if the name is not
    /// known).
    pub fn user(metadata: &FileMetadata) -> String {
        match metadata.user() {
            Some(user) => user.to_string(),
            None => metadata.uid().to_string(),
        }
    }

    /// Return the name of the group which owns the entry (or the id of the group if the name is
    /// not known).
    pub fn group(metadata: &FileMetadata) -> String {
        match metadata.group() {
            Some(group) => group.to_string(),
            None => metadata.gid().to_string(),
        }
    }

    /// Return the number of columns which come before the name of an entry shown with its
    /// metadata.
    pub fn name_offset(
        hard_links_width: usize,
        user_width: usize,
        group_width: usize,
        size_width: usize,
    ) -> usize {
        FILE_MODE_WIDTH
            + 1
            + hard_links_width
            + 1
            + user_width
            + 1
            + group_width
            + 1
            + size_width
            + 1
            + FILE_TIME_WIDTH
            + 1
    }

    /// Return the number of columns needed to show the entries with their metadata without cutting
    /// anything off.
    pub fn width(
        hard_links_width: usize,
        user_width: usize,
        group_width: usize,
        size_width: usize,
        name_width: usize,
    ) -> usize {
        name_offset(hard_links_width, user_width, group_width, size_width) + name_width
    }

    /// Return the entry shown with its metadata the same way that `ls -l` shows it.
    ///
    /// The `now` is the current number of seconds since the Unix epoch.
    pub fn row(
        file_info: &FileInfo,
        now: i64,
        hard_links_width: usize,
        user_width: usize,
        group_width: usize,
        size_width: usize,
    ) -> String {
        let name: String = name_and_link_target(file_info);

        let (mode, hard_links, user, group, size, time) = match file_info.metadata() {
            Some(metadata) => (
                human_friendly_file_mode(metadata.mode()),
                metadata.hard_links().to_string(),
                user(metadata),
                group(metadata),
                metadata.size().to_string(),
                human_friendly_file_time(metadata.modified(), now),
            ),
            // The metadata could not be read, so show that it is not known the same way that `ls`
            // does.
            None => (
                unknown(FILE_MODE_WIDTH),
                unknown(hard_links_width),
                unknown(user_width),
                unknown(group_width),
                unknown(size_width),
                unknown(FILE_TIME_WIDTH),
            ),
        };

        format!(
            "{mode} {hard_links:>hard_links_width$} {user:<user_width$} \
             {group:<group_width$} {size:>size_width$} {time} {name}"
        )
    }

    /// Return a string of the given width indicating that metadata is not known.
    fn unknown(width: usize) -> String {
        UNKNOWN.to_string().repeat(width)
    }

    #[cfg(test)]
    mod tests {
        use std::path::PathBuf;

        use file_type::FileType;
        use test_case::test_case;

        use super::*;

        /// Return the number of seconds since the Unix epoch used as the current time by the
        /// tests.
        fn now() -> i64 {
            1_718_444_400
        }

        /// Return the time used by the tests rendered the way that it is shown.
        ///
        /// How times are rendered is tested by the `human-friendly` crate, so the tests here use
        /// this so that they do not depend on the time zone that they are run in.
        fn time() -> String {
            human_friendly_file_time(now(), now())
        }

        /// Return the metadata of an entry for testing.
        fn metadata(mode: u32, hard_links: u64, size: u64) -> FileMetadata {
            FileMetadata::builder()
                .mode(mode)
                .hard_links(hard_links)
                .uid(1000)
                .user(Some(String::from("austin")))
                .gid(1000)
                .group(Some(String::from("austin")))
                .size(size)
                .modified(now())
                .build()
        }

        /// Return the metadata of an entry which is owned by a user and group without names.
        fn metadata_without_names(mode: u32, hard_links: u64, size: u64) -> FileMetadata {
            FileMetadata::builder()
                .mode(mode)
                .hard_links(hard_links)
                .uid(1234)
                .gid(5678)
                .size(size)
                .modified(now())
                .build()
        }

        /// Return the metadata of a symlink to the target.
        fn metadata_of_link(size: u64, link_target: &str) -> FileMetadata {
            FileMetadata::builder()
                .mode(0o120777)
                .hard_links(1)
                .uid(1000)
                .user(Some(String::from("austin")))
                .gid(1000)
                .group(Some(String::from("austin")))
                .size(size)
                .modified(now())
                .link_target(Some(PathBuf::from(link_target)))
                .build()
        }

        /// Return an entry for testing.
        fn file_info(path: &str, r#type: FileType, metadata: Option<FileMetadata>) -> FileInfo {
            FileInfo::builder()
                .path(PathBuf::from(path))
                .r#type(Ok(r#type))
                .metadata(metadata)
                .build()
        }

        #[test_case(
            file_info("/tmp/big", FileType::File, Some(metadata(0o100664, 1, 123456))),
            (2, 6, 6, 6),
            "-rw-rw-r--  1 austin austin 123456 {time} big";
            "a file in columns which are wider than it needs"
        )]
        #[test_case(
            file_info("/tmp/sub", FileType::Dir, Some(metadata(0o040775, 12, 4096))),
            (2, 6, 6, 6),
            "drwxrwxr-x 12 austin austin   4096 {time} sub/";
            "a directory which has a trailing slash"
        )]
        #[test_case(
            file_info("/tmp/link", FileType::Symlink, Some(metadata_of_link(9, "../target"))),
            (1, 6, 6, 1),
            "lrwxrwxrwx 1 austin austin 9 {time} link -> ../target";
            "a symlink which shows its target"
        )]
        #[test_case(
            file_info("/tmp/file", FileType::File, Some(metadata_without_names(0o100644, 1, 0))),
            (1, 4, 4, 1),
            "-rw-r--r-- 1 1234 5678 0 {time} file";
            "a file owned by a user and group without names"
        )]
        #[test_case(
            file_info("/tmp/unknown", FileType::File, None),
            (1, 6, 6, 3),
            "?????????? ? ?????? ?????? ??? ???????????? unknown";
            "a file whose metadata is not known"
        )]
        fn test_row(
            file_info: FileInfo,
            widths: (usize, usize, usize, usize),
            expected_string: &str,
        ) {
            let (hard_links_width, user_width, group_width, size_width) = widths;

            let string: String = row(
                &file_info,
                now(),
                hard_links_width,
                user_width,
                group_width,
                size_width,
            );

            assert_eq!(string, expected_string.replace("{time}", &time()));
        }

        #[test_case(
            file_info("/tmp/sub", FileType::Dir, Some(metadata(0o040775, 12, 4096))),
            (2, 6, 6, 6),
            4;
            "a directory which has a trailing slash"
        )]
        #[test_case(
            file_info("/tmp/link", FileType::Symlink, Some(metadata_of_link(9, "../target"))),
            (1, 6, 6, 1),
            17;
            "a symlink which shows its target"
        )]
        fn test_width_is_the_width_of_the_row(
            file_info: FileInfo,
            widths: (usize, usize, usize, usize),
            name_width: usize,
        ) {
            let (hard_links_width, user_width, group_width, size_width) = widths;

            let string: String = row(
                &file_info,
                now(),
                hard_links_width,
                user_width,
                group_width,
                size_width,
            );

            assert_eq!(
                width(
                    hard_links_width,
                    user_width,
                    group_width,
                    size_width,
                    name_width
                ),
                string.chars().count()
            );
        }
    }
}
use entry::{name, name_and_link_target};

#[derive(TypedBuilder)]
pub struct Props {
    config: Config,
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
                Ok(file_infos) => {
                    let visible_file_infos = self.state.visible_file_infos().unwrap();
                    if visible_file_infos.is_empty() {
                        return Fabric::center("The directory is empty.", size);
                    }

                    // The columns are sized by *all* of the entries (not just the visible ones) so
                    // that they do not shift around when scrolling. The metadata is only shown if
                    // the whole listing fits without anything being cut off.
                    let mut hard_links_width: usize = 1;
                    let mut user_width: usize = 1;
                    let mut group_width: usize = 1;
                    let mut size_width: usize = 1;
                    let mut name_width: usize = 0;
                    if self.state.metadata {
                        for entry in file_infos {
                            name_width =
                                cmp::max(name_width, name_and_link_target(entry).chars().count());

                            let metadata = match entry.metadata() {
                                Some(metadata) => metadata,
                                None => {
                                    continue;
                                }
                            };

                            hard_links_width = cmp::max(
                                hard_links_width,
                                metadata.hard_links().to_string().chars().count(),
                            );
                            user_width =
                                cmp::max(user_width, entry::user(metadata).chars().count());
                            group_width =
                                cmp::max(group_width, entry::group(metadata).chars().count());
                            size_width =
                                cmp::max(size_width, metadata.size().to_string().chars().count());
                        }
                    }
                    let show_metadata: bool = self.state.metadata
                        && entry::width(
                            hard_links_width,
                            user_width,
                            group_width,
                            size_width,
                            name_width,
                        ) <= size.columns;

                    // Only the name of a hidden entry is grayed out (not its metadata).
                    let name_offset: usize = match show_metadata {
                        true => entry::name_offset(
                            hard_links_width,
                            user_width,
                            group_width,
                            size_width,
                        ),
                        false => 0,
                    };

                    let now: i64 = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|since_epoch| since_epoch.as_secs() as i64)
                        .unwrap_or(0);

                    let mut yarns: Vec<Yarn> = Vec::new();
                    for (entry, row) in visible_file_infos.iter().zip(0..size.rows) {
                        let name: String = name(entry);
                        let hidden = name.starts_with('.');

                        let string: String = match show_metadata {
                            true => entry::row(
                                entry,
                                now,
                                hard_links_width,
                                user_width,
                                group_width,
                                size_width,
                            ),
                            false => name,
                        };

                        let mut yarn = Yarn::from(string);

                        if Some(row) == self.state.selected {
                            yarn.color(Color::InvertedText.into());
                            yarn.background(Color::Highlight.into());
                        } else if hidden {
                            yarn.color_after(Color::LightGrayedText.into(), name_offset);
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
                        KeyEvent {
                            key: Key::Char('m'),
                            mods: KeyMods::NONE,
                        } => Some(Action::ToggleMetadata),
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
    config: Config,
    size: Size,
    dir: PathBuf,

    starting_file: Option<PathBuf>,
    /// The offset to return to once the starting file is found (if possible).
    starting_offset: Option<usize>,
    pending_request: Option<Uuid>,

    /// The dir entries (if they can be read).
    file_infos: Option<GetFilesResult>,

    /// Whether or not the entries are shown with their metadata.
    metadata: bool,

    selected: Option<usize>,
    offset: usize,
}

impl From<Props> for State {
    fn from(props: Props) -> Self {
        let size = props.size;
        let dir: PathBuf = props.dir;

        // NOTE: The request which is already pending (if there is one) asks for the metadata of
        // the entries if it is configured to be shown too.
        let metadata: bool = props.config.browser().metadata();

        State {
            config: props.config,
            size,
            dir,
            starting_file: props.file,
            starting_offset: None,
            pending_request: props.pending_request,
            file_infos: None,
            metadata,
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

    /// Return a request for getting the files of the current directory.
    fn get_files_request(&self) -> Request {
        get_files_request(self.dir.clone(), &self.config, self.metadata)
    }

    /// Return whether or not the entries which are known have their metadata.
    fn have_metadata(&self) -> bool {
        match &self.file_infos {
            Some(Ok(file_infos)) => file_infos
                .iter()
                .any(|file_info| file_info.metadata().is_some()),
            _ => false,
        }
    }

    /// Remember the selected entry (and the scroll position) so that they can be returned to once
    /// the entries are known again.
    ///
    /// If there is no selected entry to remember (because the entries are not known yet), then the
    /// position which is already remembered is kept. Otherwise requesting the entries again before
    /// a previous response is handled would forget where to return to.
    fn remember_position(&mut self) {
        let selected_entry: Option<PathBuf> = match self.entry() {
            Some(entry) => Some(entry.path().to_path_buf()),
            None => {
                return;
            }
        };

        self.starting_file = selected_entry;
        self.starting_offset = Some(self.offset);
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
        self.remember_position();
        self.reset_file_infos();

        let request = self.get_files_request();
        self.pending_request = Some(*request.uuid());
        Some(Effect::Request(request))
    }

    fn push(&mut self) -> Option<Effect> {
        if let Some(entry) = self.entry() {
            let path: PathBuf = entry.path().to_path_buf();
            if path.is_dir() {
                self.set_dir(&path);

                let request = self.get_files_request();
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

            let request = self.get_files_request();
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

    /// Toggle whether or not the entries are shown with their metadata.
    ///
    /// If the entries are now shown with their metadata but it is not known yet, then request the
    /// entries again. The entries which are already known are kept so that they remain shown
    /// (without their metadata) until the response is handled.
    fn toggle_metadata(&mut self) -> Option<Effect> {
        self.metadata = !self.metadata;

        if !self.metadata || self.have_metadata() {
            return None;
        }

        self.remember_position();

        let request = self.get_files_request();
        self.pending_request = Some(*request.uuid());
        Some(Effect::Request(request))
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
                        // Keep the current scroll position if the entry is still visible from it.
                        let starting_offset: Option<usize> =
                            self.starting_offset.filter(|starting_offset| {
                                (*starting_offset..starting_offset + self.size.rows)
                                    .contains(&index)
                            });

                        if let Some(starting_offset) = starting_offset {
                            selected = Some(index - starting_offset);
                            offset = starting_offset;
                        } else if index < self.size.rows {
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
        self.starting_offset = None;

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
            Action::ToggleMetadata => self.toggle_metadata(),
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
    ToggleMetadata,
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

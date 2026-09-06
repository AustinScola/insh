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
    use std::path::{Path, MAIN_SEPARATOR as PATH_SEPARATOR};

    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::components::common::FooterInfo;
    use crate::stateful::Stateful;

    use rend::{Cell, Fabric, Size, Yarn};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::{CommandParser, Component, KeyPattern, Parsed};

    impl Contents {
        /// Return a parser for the keys which the contents responds to.
        fn command_parser() -> CommandParser<Action> {
            CommandParser::new()
                .bind(
                    [KeyPattern::exact(Key::Char('q'), KeyMods::CONTROL)],
                    Action::Unfocus,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('j'), KeyMods::NONE)],
                    Action::Down,
                )
                .bind([KeyPattern::exact(Key::Down, KeyMods::NONE)], Action::Down)
                .bind(
                    [KeyPattern::exact(Key::Char('J'), KeyMods::SHIFT)],
                    Action::ReallyDown,
                )
                .bind(
                    [KeyPattern::exact(Key::End, KeyMods::NONE)],
                    Action::ReallyDown,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('k'), KeyMods::NONE)],
                    Action::Up,
                )
                .bind([KeyPattern::exact(Key::Up, KeyMods::NONE)], Action::Up)
                .bind(
                    [KeyPattern::exact(Key::Char('K'), KeyMods::SHIFT)],
                    Action::ReallyUp,
                )
                .bind(
                    [KeyPattern::exact(Key::Home, KeyMods::NONE)],
                    Action::ReallyUp,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('r'), KeyMods::NONE)],
                    Action::Refresh,
                )
                .bind([KeyPattern::any(Key::Char('l'))], Action::Edit)
                .bind([KeyPattern::any(Key::CarriageReturn)], Action::Edit)
                .bind([KeyPattern::exact(Key::Right, KeyMods::NONE)], Action::Edit)
                .bind(
                    [KeyPattern::exact(Key::Char('g'), KeyMods::NONE)],
                    Action::Goto,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('G'), KeyMods::SHIFT)],
                    Action::ReallyGoto,
                )
                .bind(
                    [
                        KeyPattern::exact(Key::Char('y'), KeyMods::NONE),
                        KeyPattern::exact(Key::Char('E'), KeyMods::SHIFT),
                    ],
                    Action::YankName,
                )
                .bind(
                    [
                        KeyPattern::exact(Key::Char('y'), KeyMods::NONE),
                        KeyPattern::exact(Key::Char('y'), KeyMods::NONE),
                    ],
                    Action::YankPath,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('Y'), KeyMods::SHIFT)],
                    Action::YankContents,
                )
        }
    }

    pub struct Contents {
        state: State,
        /// Parses the keys pressed into actions.
        command_parser: CommandParser<Action>,
    }

    impl Component<Props, Event, Effect> for Contents {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Self {
                state,
                command_parser: Self::command_parser(),
            }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Action = match event {
                Event::Find { phrase } => Action::Find { phrase },
                Event::TermEvent(term_event) => match term_event {
                    TermEvent::Resize(size) => Action::Resize { size },
                    // There is nothing to paste into a list of hits.
                    TermEvent::Paste(_) => {
                        return None;
                    }
                    TermEvent::KeyEvent(key_event) => match self.command_parser.parse(key_event) {
                        Parsed::Command(action) => action,
                        Parsed::Pending => {
                            return None;
                        }
                        Parsed::Unknown(keys) => Action::UnknownCommand {
                            keys: keys.iter().map(KeyEvent::to_string).collect(),
                        },
                    },
                },
                Event::Response(response) => Action::HandleResponse(response),
            };

            self.state.perform(action)
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

                        // NOTE: The file name has to be measured in columns like the yarn is.
                        // Measuring it in bytes makes the colours land in the wrong place for a
                        // name which is not all ASCII, and underflows when the file is directly in
                        // the directory which was searched.
                        let file_name: String = entry
                            .file_name()
                            .expect("Entry is not a file")
                            .to_string_lossy()
                            .to_string();
                        let file_name_start: usize = yarn.len() - Cell::columns(&file_name);

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

    impl FooterInfo for Contents {
        fn text(&self) -> String {
            let pending: String = self
                .command_parser
                .pending()
                .iter()
                .map(KeyEvent::to_string)
                .collect();
            if !pending.is_empty() {
                return pending;
            }

            match self.state.message() {
                Some(message) => message.text(),
                // The contents of a file are on their way, so showing how the last search went
                // again until they get here would only be a flash of what is already old news.
                None if self.state.yanking() => String::new(),
                None => self.state.progress(),
            }
        }

        fn position(&self) -> String {
            let hits: usize = self.state.entries().len();

            match self.state.entry_number() {
                Some(entry_number) if entry_number < hits => {
                    format!("{}/{}", entry_number + 1, hits)
                }
                _ => String::new(),
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
    use std::cmp::{self, Ordering};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use super::{Action, Effect, Props};
    use crate::clipboard::Clipboard;
    use crate::command_message::CommandMessage;
    use crate::programs::{VimArgs, VimArgsBuilder};
    use crate::request_builders::find_files_request;
    use crate::stateful::Stateful;

    use insh_api::{
        FindFilesResponseParams, GetFileContentsRequestParams, GetFileContentsResponseParams,
        Request, RequestParams, Response, ResponseParams,
    };
    use path_finder::Entry;
    use rend::Size;

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
        /// The request for the contents of a file which are to be copied to the clipboard.
        pending_yank_request: Option<Uuid>,
        /// What the last command had to say for itself (if anything).
        message: Option<CommandMessage>,
        received_first_resp: bool,
        /// The number of files which have been searched.
        files_searched: usize,
        /// The duration of the search so far.
        duration: Duration,
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
                pending_yank_request: None,
                message: None,
                received_first_resp: false,
                files_searched: 0,
                duration: Duration::ZERO,
            }
        }
    }

    impl State {
        pub fn dir(&self) -> &PathBuf {
            &self.dir
        }

        /// Return what the last command had to say for itself (if anything).
        pub fn message(&self) -> Option<&CommandMessage> {
            self.message.as_ref()
        }

        /// Return whether or not the contents of a file are being read to be copied to the
        /// clipboard.
        pub fn yanking(&self) -> bool {
            self.pending_yank_request.is_some()
        }

        /// Return how finding the files is going (or nothing if no files have been looked for).
        pub fn progress(&self) -> String {
            if self.phrase.is_none() {
                return String::new();
            }

            format!(
                "{} file{} searched ({:.2}s)",
                self.files_searched,
                match self.files_searched {
                    1 => "",
                    _ => "s",
                },
                self.duration.as_secs_f64(),
            )
        }

        pub fn entries(&self) -> &[Entry] {
            &self.entries
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

        pub fn entry_number(&self) -> Option<usize> {
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
            self.received_first_resp = false;
            self.files_searched = 0;
            self.duration = Duration::ZERO;

            let request: Request = find_files_request(self.dir.clone(), phrase.to_string());
            self.pending_request = Some(*request.uuid());

            Some(Effect::Request(request))
        }

        fn down(&mut self) -> Option<Effect> {
            // There is nowhere to move to if none of the hits are shown (which happens when the
            // terminal is too short for anything but the directory, the phrase, and the footer).
            if self.entries.is_empty() || self.size.rows == 0 {
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
            // There is nowhere to move to if none of the hits are shown (which happens when the
            // terminal is too short for anything but the directory, the phrase, and the footer).
            if self.entries.is_empty() || self.size.rows == 0 {
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

        /// Copy the file name to the system clipboard.
        fn yank_name(&mut self) -> Option<Effect> {
            let entry: PathBuf = match self.entry_path() {
                Some(entry) => entry.to_path_buf(),
                None => {
                    return None;
                }
            };

            let name: String = match entry.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => {
                    return None;
                }
            };

            let mut clipboard = Clipboard::new();
            clipboard.copy(name);

            self.message = Some(CommandMessage::Ran(String::from("yanked file name")));
            None
        }

        /// Copy the file path to the system clipboard.
        fn yank_path(&mut self) -> Option<Effect> {
            let path: String = match self.entry_path() {
                Some(entry) => entry.to_path_buf().to_string_lossy().to_string(),
                None => {
                    return None;
                }
            };

            let mut clipboard = Clipboard::new();
            clipboard.copy(path);

            self.message = Some(CommandMessage::Ran(String::from("yanked path")));
            None
        }

        /// Ask the daemon for the contents of the file so that they can be copied to the
        /// clipboard.
        fn yank_contents(&mut self) -> Option<Effect> {
            let path: PathBuf = match self.entry_path() {
                Some(entry) => entry.to_path_buf(),
                None => {
                    return None;
                }
            };

            let request: Request = Request::builder()
                .params(RequestParams::GetFileContents(
                    GetFileContentsRequestParams::builder().path(path).build(),
                ))
                .build();
            self.pending_yank_request = Some(*request.uuid());
            Some(Effect::Request(request))
        }

        /// Remember that the keys pressed do not form a command.
        fn unknown_command(&mut self, keys: String) -> Option<Effect> {
            self.message = Some(CommandMessage::UnknownCommand(keys));
            Some(Effect::Bell)
        }

        fn handle_response(&mut self, response: Response) -> Option<Effect> {
            #[cfg(feature = "logging")]
            log::debug!("Handling response...");

            if Some(*response.uuid()) == self.pending_yank_request {
                self.pending_yank_request = None;

                let params: &GetFileContentsResponseParams = match response.params() {
                    ResponseParams::GetFileContents(params) => params,
                    _ => {
                        #[cfg(feature = "logging")]
                        log::error!("Unexpected response parameters.");
                        return Some(Effect::Bell);
                    }
                };

                return match params.result() {
                    Ok(contents) => {
                        let mut clipboard = Clipboard::new();
                        clipboard.copy(contents.clone());

                        self.message =
                            Some(CommandMessage::Ran(String::from("yanked file contents")));
                        None
                    }
                    Err(error) => {
                        self.message = Some(CommandMessage::Failed(format!(
                            "failed to read the file contents: {}",
                            error
                        )));
                        Some(Effect::Bell)
                    }
                };
            }

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

            if !self.received_first_resp {
                self.hits = None;
                self.entries.clear();
                self.selected = None;
                self.offset = 0;
            }
            self.received_first_resp = true;

            let params: &FindFilesResponseParams = match response.params() {
                ResponseParams::FindFiles(params) => params,
                _ => {
                    #[cfg(feature = "logging")]
                    log::error!("Unexpected response parameters.");
                    return None;
                }
            };

            self.entries.extend_from_slice(params.entries());
            self.files_searched = params.searched();
            self.duration = params.duration();

            if response.last() {
                self.pending_request = None;
            }

            // A response which does not have any entries is only an update of how the finding of
            // the files is going, so nothing is shown yet unless there is nothing left to find.
            if self.entries.is_empty() {
                if response.last() {
                    self.hits = Some(false);
                    self.selected = None;
                    return Some(Effect::Unfocus);
                }

                return None;
            }

            self.hits = Some(true);
            if self.selected.is_none() {
                self.selected = Some(0);
            }

            None
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            // Running a command clears what the last one had to say. Responses are not commands,
            // so they leave it alone.
            if !matches!(action, Action::HandleResponse(_)) {
                self.message = None;
            }

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
                Action::YankName => self.yank_name(),
                Action::YankPath => self.yank_path(),
                Action::YankContents => self.yank_contents(),
                Action::UnknownCommand { keys } => self.unknown_command(keys),
                Action::HandleResponse(response) => self.handle_response(response),
            }
        }
    }
}
use state::State;

mod action {
    use insh_api::Response;
    use rend::Size;

    #[derive(Clone)]
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
        YankName,
        YankPath,
        YankContents,
        UnknownCommand { keys: String },
        HandleResponse(Response),
    }
}
use action::Action;

mod effect {
    use std::path::PathBuf;

    use crate::programs::VimArgs;

    use insh_api::Request;

    pub enum Effect {
        Unfocus,
        Request(Request),
        Goto { dir: PathBuf, file: Option<PathBuf> },
        OpenVim(VimArgs),
        Bell,
    }
}
pub use effect::Effect;

mod props {
    use std::path::PathBuf;

    use crate::config::Config;

    use rend::Size;

    use uuid::Uuid;

    pub struct Props {
        pub config: Config,
        pub dir: PathBuf,
        pub size: Size,
        pub phrase: Option<String>,
        pub pending_request: Option<Uuid>,
    }

    impl Props {
        pub fn new(
            config: Config,
            dir: PathBuf,
            size: Size,
            phrase: Option<String>,
            pending_request: Option<Uuid>,
        ) -> Self {
            Self {
                config,
                dir,
                size,
                phrase,
                pending_request,
            }
        }
    }
}
pub use props::Props;

mod contents {
    use std::path::MAIN_SEPARATOR as PATH_SEPARATOR;

    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::components::common::FooterInfo;
    use crate::string::DetabExt;
    use crate::Config;
    use crate::Stateful;

    use phrase_searcher::{FileHit, LineHit};
    use rend::{Fabric, Size, Yarn};
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
                .bind(
                    [KeyPattern::exact(Key::Char('J'), KeyMods::SHIFT)],
                    Action::ReallyDown,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('j'), KeyMods::CONTROL)],
                    Action::ScrollDown,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('k'), KeyMods::NONE)],
                    Action::Up,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('K'), KeyMods::SHIFT)],
                    Action::ReallyUp,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('k'), KeyMods::CONTROL)],
                    Action::ScrollUp,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('r'), KeyMods::NONE)],
                    Action::Refresh,
                )
                .bind([KeyPattern::any(Key::Char('l'))], Action::Edit)
                .bind([KeyPattern::any(Key::CarriageReturn)], Action::Edit)
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
                    Action::YankWord,
                )
                .bind(
                    [
                        KeyPattern::exact(Key::Char('y'), KeyMods::NONE),
                        KeyPattern::exact(Key::Char('y'), KeyMods::NONE),
                    ],
                    Action::YankLine,
                )
                .bind(
                    [KeyPattern::exact(Key::Char('Y'), KeyMods::SHIFT)],
                    Action::YankContents,
                )
        }
    }

    pub struct Contents {
        config: Config,
        state: State,
        /// Parses the keys pressed into actions.
        command_parser: CommandParser<Action>,
    }

    impl Component<Props, Event, Effect> for Contents {
        fn new(props: Props) -> Self {
            let state: State = State::from(&props);
            Self {
                config: props.config,
                state,
                command_parser: Self::command_parser(),
            }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Action = match event {
                Event::Search { phrase } => Action::Search { phrase },
                Event::Response(response) => Action::HandleResponse(response),
                Event::TermEvent(TermEvent::Resize(size)) => Action::Resize { size },
                Event::TermEvent(TermEvent::KeyEvent(key_event)) => {
                    match self.command_parser.parse(key_event) {
                        Parsed::Command(action) => action,
                        Parsed::Pending => {
                            return None;
                        }
                        Parsed::Unknown(keys) => Action::UnknownCommand {
                            keys: keys.iter().map(KeyEvent::to_string).collect(),
                        },
                    }
                }
            };

            self.state.perform(action)
        }

        fn render(&self, size: Size) -> Fabric {
            match self.state.searched() {
                false => Fabric::new(size),
                true => {
                    if self.state.hits().is_empty() {
                        if self.state.is_pending() {
                            Fabric::new(size)
                        } else {
                            Fabric::center("No matches.", size)
                        }
                    } else {
                        let file_hits: &Vec<FileHit> = self.state.hits();
                        let rows = size.rows;
                        let columns = size.columns;
                        let mut yarns: Vec<Yarn> = Vec::new();

                        let file_hits = file_hits.iter().enumerate().skip(self.state.file_offset());
                        for (file_hit_number, file_hit) in file_hits {
                            if yarns.len() == rows {
                                break;
                            }

                            let first_hit = file_hit_number == self.state.file_offset();
                            let file_hit_is_focused: bool =
                                self.state.hit_number().unwrap() == file_hit_number;

                            let draw_path = !(first_hit && self.state.line_offset().is_some());
                            if draw_path {
                                let mut path: String =
                                    file_hit.path().to_string_lossy().to_string();
                                let dir_string: String =
                                    self.state.dir().to_string_lossy().to_string();
                                path = path.strip_prefix(&dir_string).unwrap().to_string();
                                if path.starts_with(PATH_SEPARATOR) {
                                    path = path.strip_prefix(PATH_SEPARATOR).unwrap().to_string();
                                }

                                let mut yarn = Yarn::from(path);
                                yarn.resize(columns);

                                if self.state.focussed()
                                    && !self.state.is_line_selected()
                                    && file_hit_is_focused
                                {
                                    yarn.background(Color::Highlight.into());
                                    yarn.color(Color::InvertedText.into());
                                }

                                yarns.push(yarn);
                            }

                            let mut line_hits: Vec<(usize, &LineHit)> =
                                file_hit.line_hits().iter().enumerate().collect();
                            if first_hit {
                                if let Some(line_offset) = self.state.line_offset() {
                                    line_hits = line_hits.into_iter().skip(line_offset).collect();
                                }
                            }
                            for (line_hit_number, line_hit) in line_hits {
                                if yarns.len() == rows {
                                    break;
                                }

                                let mut string: String = line_hit.line_number().to_string();
                                string.push_str(": ");
                                string.push_str(
                                    &line_hit.line().detab(self.config.general().tab_width()),
                                );

                                let mut yarn = Yarn::from(string);
                                yarn.resize(columns);
                                if self.state.focussed()
                                    && file_hit_is_focused
                                    && self.state.is_line_selected()
                                    && self.state.line_hit_number().unwrap() == line_hit_number
                                {
                                    yarn.background(Color::Highlight.into());
                                    yarn.color(Color::InvertedText.into());
                                }
                                yarns.push(yarn);
                            }

                            if yarns.len() == rows {
                                break;
                            }
                            let yarn = Yarn::blank(columns);
                            yarns.push(yarn);
                        }

                        let mut fabric = Fabric::from(yarns);

                        if fabric.size().rows < size.rows {
                            fabric.pad_bottom(size.rows);
                        }

                        fabric
                    }
                }
            }
        }
    }

    /// The line hits are counted (and not the file hits) because the lines are what is moved
    /// between. A dash is shown in place of the position when a file is selected instead of one of
    /// its lines.
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
            let line_hits: usize = self.state.line_hits();
            if line_hits == 0 {
                return String::new();
            }

            match self.state.selected_line_hit() {
                Some(selected) if selected < line_hits => {
                    format!("{}/{}", selected + 1, line_hits)
                }
                Some(_) => String::new(),
                None => format!("-/{}", line_hits),
            }
        }
    }
}
pub use contents::Contents;

mod event {
    use insh_api::Response;
    use term::TermEvent;

    pub enum Event {
        TermEvent(TermEvent),
        Search { phrase: String },
        Response(Response),
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
    use crate::request_builders::search_phrase_request;
    use crate::Stateful;

    use insh_api::{
        GetFileContentsRequestParams, GetFileContentsResponseParams, Request, RequestParams,
        Response, ResponseParams, SearchPhraseResponseParams,
    };
    use phrase_searcher::{FileHit, LineHit};
    use rend::Size;

    use uuid::Uuid;

    #[derive(Debug, PartialEq, Eq, Default)]
    pub struct State {
        size: Size,
        dir: PathBuf,
        phrase: Option<String>,
        focussed: bool,
        searched: bool,
        hits: Vec<FileHit>,
        file_offset: usize,
        line_offset: Option<usize>,
        file_selected: usize,
        line_selected: Option<usize>,
        pending_request: Option<Uuid>,
        /// The request for the contents of a file which are to be copied to the clipboard.
        pending_yank_request: Option<Uuid>,
        /// What the last command had to say for itself (if anything).
        message: Option<CommandMessage>,
        /// The number of files which have been searched.
        files_searched: usize,
        /// The duration of the search so far.
        duration: Duration,
    }

    impl From<&Props> for State {
        fn from(props: &Props) -> Self {
            Self {
                size: props.size,
                // Would be nice to not have to clone this. Maybe use the builder pattern instead
                // of using From &Props?
                dir: props.dir.clone(),
                phrase: props.phrase.clone(),
                focussed: props.pending_request.is_some(),
                searched: props.pending_request.is_some(),
                hits: Vec::new(),
                file_offset: 0,
                line_offset: None,
                file_selected: 0,
                line_selected: None,
                pending_request: props.pending_request,
                pending_yank_request: None,
                message: None,
                files_searched: 0,
                duration: Duration::ZERO,
            }
        }
    }

    impl State {
        pub fn dir(&self) -> &Path {
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

        /// Return how searching the files is going (or nothing if the files were not searched).
        pub fn progress(&self) -> String {
            if !self.searched {
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

        /// Return the total number of lines which contain the phrase.
        pub fn line_hits(&self) -> usize {
            self.hits
                .iter()
                .map(|file_hit| file_hit.line_hits().len())
                .sum()
        }

        /// Return which of the lines containing the phrase is selected, or `None` if a file is
        /// selected instead of one of its lines.
        pub fn selected_line_hit(&self) -> Option<usize> {
            let hit_number: usize = self.hit_number()?;
            let line_hit_number: usize = self.line_hit_number()?;

            // The line hits of the hits before the selected one all come first.
            let before: usize = self.hits[..hit_number]
                .iter()
                .map(|file_hit| file_hit.line_hits().len())
                .sum();

            // The number of the selected line is clamped so that the position stays in range even
            // if the selection and the scroll are momentarily out of step.
            let line_hits: usize = self.hits[hit_number].line_hits().len();
            let line_hit_number: usize = cmp::min(line_hit_number, line_hits.saturating_sub(1));

            return Some(before + line_hit_number);
        }

        /// Return if the search contents are currently foccused on.
        pub fn focussed(&self) -> bool {
            self.focussed
        }

        pub fn searched(&self) -> bool {
            self.searched
        }

        /// Return if a search request is pending.
        pub fn is_pending(&self) -> bool {
            self.pending_request.is_some()
        }

        /// The number of the currently selected file hit.
        pub fn hit_number(&self) -> Option<usize> {
            let number: usize = self.file_offset + self.file_selected;
            if number < self.hits().len() {
                Some(number)
            } else {
                None
            }
        }

        pub fn file_offset(&self) -> usize {
            self.file_offset
        }

        pub fn line_offset(&self) -> Option<usize> {
            self.line_offset
        }

        pub fn line_hit_number(&self) -> Option<usize> {
            match self.line_selected {
                Some(line_selected) => match self.file_selected {
                    0 => match self.line_offset {
                        None => Some(line_selected),
                        Some(line_offset) => Some(line_offset + line_selected),
                    },
                    _ => Some(line_selected),
                },
                None => None,
            }
        }

        /// Return the currently selected file hit.
        pub fn hit(&self) -> Option<&FileHit> {
            match self.hit_number() {
                Some(hit_number) => Some(&self.hits[hit_number]),
                None => None,
            }
        }

        pub fn hits(&self) -> &Vec<FileHit> {
            &self.hits
        }

        /// Return if a line is selected or not.
        pub fn is_line_selected(&self) -> bool {
            self.line_selected.is_some()
        }

        /// Return the row number that is selected.
        fn selected_row_number(&self) -> usize {
            match self.file_selected {
                0 => match self.line_selected {
                    None => 0,
                    Some(line_selected) => match self.line_offset {
                        None => line_selected + 1,
                        Some(_) => line_selected,
                    },
                },
                _ => {
                    let mut selected_row_number = 0;

                    let first_hit = &self.hits[self.file_offset];
                    selected_row_number += (first_hit.line_hits().len() + 1)
                        - match self.line_offset {
                            None => 0,
                            Some(line_offset) => line_offset + 1,
                        };

                    for hit_number in
                        (self.file_offset + 1)..(self.file_offset + self.file_selected)
                    {
                        selected_row_number += self.hits[hit_number].line_hits().len() + 2;
                    }

                    selected_row_number += match self.line_selected {
                        None => 1,
                        Some(line_selected) => line_selected + 2,
                    };

                    selected_row_number
                }
            }
        }

        fn resize(&mut self, new_size: Size) -> Option<Effect> {
            let rows_before = self.size.rows;
            let selected_row_number = self.selected_row_number();
            let position_percent: f64 = selected_row_number as f64 / rows_before as f64;

            let new_selected_row_number = (new_size.rows as f64 * position_percent) as usize;

            self.size = new_size;

            match new_selected_row_number.cmp(&selected_row_number) {
                Ordering::Less => {
                    self.scroll_down(selected_row_number - new_selected_row_number);
                }
                Ordering::Greater => {
                    self.scroll_up(new_selected_row_number - selected_row_number);
                }
                _ => {}
            }

            None
        }

        fn focus(&mut self) {
            self.focussed = true;
        }

        fn unfocus(&mut self) -> Option<Effect> {
            self.focussed = false;
            Some(Effect::Unfocus)
        }

        fn search(&mut self, phrase: &str) -> Option<Effect> {
            self.focus();
            self.phrase = Some(phrase.to_string());
            self.searched = true;

            // Clear the previous search's hits immediately rather than leaving them displayed
            // until the new search's first response arrives.
            self.hits.clear();
            self.file_offset = 0;
            self.line_offset = None;
            self.file_selected = 0;
            self.line_selected = None;
            self.files_searched = 0;
            self.duration = Duration::ZERO;

            let request: Request = search_phrase_request(self.dir.clone(), phrase.to_string());
            self.pending_request = Some(*request.uuid());

            Some(Effect::Request(request))
        }

        fn down(&mut self) -> Option<Effect> {
            match self.line_selected {
                None => {
                    self.line_selected = Some(0);
                }
                Some(line_selected) => {
                    if self.line_hit_number().unwrap() < self.hit().unwrap().line_hits().len() - 1 {
                        self.line_selected = Some(line_selected + 1);
                    } else if self.hit_number().unwrap() < self.hits().len() - 1 {
                        self.line_selected = None;
                        self.file_selected += 1;
                    }
                }
            }

            let down_adjustment: usize =
                (self.selected_row_number() + 1).saturating_sub(self.size.rows);
            self.scroll_down(down_adjustment);

            None
        }

        /// Select the last file hit and adjust the scroll if necessary.
        fn really_down(&mut self) -> Option<Effect> {
            if self.hits.is_empty() {
                return None;
            }

            self.file_offset = self.hits.len() - 1;
            self.line_offset = None;
            self.file_selected = 0;
            self.line_selected = None;

            let up_adjustment: usize;
            {
                let last_file_hit: &FileHit = self.hits.last().unwrap();
                let number_of_line_hits: usize = last_file_hit.line_hits().len();
                up_adjustment = self.size.rows.saturating_sub(number_of_line_hits + 1);
            }
            // For now, scroll up one line at a time b/c there seems to be a bug w/ scrolling too
            // many lines at a time
            for _ in 0..up_adjustment {
                self.scroll_up(1);
            }

            None
        }

        fn scroll_down(&mut self, rows: usize) -> Option<Effect> {
            for _ in 0..rows {
                match self.line_offset {
                    None => {
                        self.line_offset = Some(0);
                    }
                    Some(line_offset) => {
                        let first_visible_hit = &self.hits[self.file_offset];
                        if line_offset < first_visible_hit.line_hits().len() {
                            self.line_offset = Some(line_offset + 1);
                            if self.file_selected == 0 {
                                if let Some(line_selected) = self.line_selected {
                                    self.line_selected = Some(line_selected.saturating_sub(1));
                                }
                            }
                        } else if self.file_offset < self.hits.len() - 1 {
                            self.file_offset += 1;
                            self.file_selected = self.file_selected.saturating_sub(1);
                            self.line_offset = None;
                        }
                    }
                }
            }
            None
        }

        fn up(&mut self) -> Option<Effect> {
            match self.line_selected {
                None => match self.file_selected {
                    0 => {
                        if self.file_offset > 0 {
                            self.file_offset -= 1;
                            self.line_offset = Some(self.hit().unwrap().line_hits().len() - 1);
                            self.line_selected = Some(0);
                        }
                    }
                    1 => {
                        self.file_selected = 0;
                        match self.line_offset {
                            None => {
                                self.line_selected =
                                    Some(self.hit().unwrap().line_hits().len() - 1);
                            }
                            Some(line_offset) => {
                                if line_offset == self.hit().unwrap().line_hits().len() {
                                    self.line_offset = Some(line_offset - 1);
                                    self.line_selected = Some(0);
                                } else {
                                    self.line_selected = Some(
                                        self.hit().unwrap().line_hits().len() - 1 - line_offset,
                                    );
                                }
                            }
                        }
                    }
                    _ => {
                        self.file_selected -= 1;
                        self.line_selected = Some(self.hit().unwrap().line_hits().len() - 1);
                    }
                },
                Some(0) => match self.file_selected.cmp(&0) {
                    Ordering::Equal => {
                        match self.line_offset {
                            None => {
                                self.line_offset = None;
                                self.line_selected = None;
                            }
                            Some(0) => {
                                self.line_offset = None;
                                self.line_selected = None;
                            }
                            Some(line_offset) => {
                                self.line_offset = Some(line_offset - 1);
                            }
                        };
                    }
                    Ordering::Greater => {
                        self.line_selected = None;
                    }
                    _ => {}
                },
                Some(line_selected) => {
                    self.line_selected = Some(line_selected - 1);
                }
            }

            None
        }

        /// Select the first file hit and adjust the scroll position if necessary.
        fn really_up(&mut self) -> Option<Effect> {
            if self.hits.is_empty() {
                return None;
            }

            self.file_offset = 0;
            self.line_offset = None;
            self.file_selected = 0;
            self.line_selected = None;

            None
        }

        fn scroll_up(&mut self, mut rows: usize) -> Option<Effect> {
            while rows > 0 {
                match self.line_offset {
                    Some(line_offset) => {
                        if rows <= line_offset {
                            self.line_offset = Some(line_offset - rows);
                            if self.file_selected == 0 {
                                if let Some(line_selected) = self.line_selected {
                                    self.line_selected = Some(line_selected + rows);
                                }
                            }
                            break;
                        }

                        if rows == line_offset + 1 {
                            self.line_offset = None;
                            if self.file_selected == 0 {
                                if let Some(line_selected) = self.line_selected {
                                    self.line_selected = Some(line_selected + rows + 1);
                                }
                            }
                            break;
                        }

                        if self.file_offset == 0 {
                            break;
                        }

                        if self.file_selected == 0 {
                            if let Some(line_selected) = self.line_selected {
                                self.line_selected = Some(line_selected + line_offset);
                            }
                        }
                        rows -= line_offset + 1;
                        self.file_offset -= 1;
                        self.line_offset = Some(self.hits[self.file_offset].line_hits().len());
                    }
                    None => {
                        if self.file_offset == 0 {
                            break;
                        }

                        rows -= 1;
                        self.file_offset -= 1;
                        self.file_selected += 1;
                        self.line_offset = Some(self.hits[self.file_offset].line_hits().len());
                    }
                }
            }
            None
        }

        /// Refresh the hits by searching for the phrase again.
        fn refresh(&mut self) -> Option<Effect> {
            if let Some(phrase) = self.phrase.clone() {
                return self.search(&phrase);
            }
            None
        }

        fn edit(&mut self) -> Option<Effect> {
            let file_hit: &FileHit = self.hit().unwrap();
            let path: &Path = file_hit.path();

            let mut vim_args_builder = VimArgsBuilder::new().path(path);

            if let Some(line_hit_number) = self.line_hit_number() {
                let line_hit: &LineHit = &file_hit.line_hits()[line_hit_number];
                let line_number = line_hit.line_number();
                vim_args_builder = vim_args_builder.line(line_number);
            }
            let vim_args: VimArgs = vim_args_builder.build();

            Some(Effect::OpenVim(vim_args))
        }

        fn goto(&mut self) -> Option<Effect> {
            self._goto(false)
        }

        fn really_goto(&mut self) -> Option<Effect> {
            self._goto(true)
        }

        fn _goto(&mut self, really: bool) -> Option<Effect> {
            if let Some(file_hit) = self.hit() {
                let path: &Path = file_hit.path();
                let dir = path.parent().unwrap().to_path_buf();
                let file: Option<PathBuf> = if really {
                    Some(path.to_path_buf())
                } else {
                    None
                };

                return Some(Effect::Goto { dir, file });
            }
            None
        }

        /// Copy the word which the cursor is on to the system clipboard.
        ///
        /// The cursor is on the name of the file when a line of a file is not selected, and on the
        /// first word of the line when one is.
        fn yank_word(&mut self) -> Option<Effect> {
            let file_hit: &FileHit = match self.hit() {
                Some(file_hit) => file_hit,
                None => {
                    return None;
                }
            };

            let (what, word): (&str, String) = match self.line_hit_number() {
                Some(line_hit_number) => {
                    let line_hit: &LineHit = &file_hit.line_hits()[line_hit_number];
                    match line_hit.line().split_whitespace().next() {
                        Some(word) => ("yanked word", word.to_string()),
                        None => {
                            return None;
                        }
                    }
                }
                None => match file_hit.path().file_name() {
                    Some(name) => ("yanked file name", name.to_string_lossy().to_string()),
                    None => {
                        return None;
                    }
                },
            };

            let mut clipboard = Clipboard::new();
            clipboard.copy(word);

            self.message = Some(CommandMessage::Ran(String::from(what)));
            None
        }

        /// Copy the line which is selected to the system clipboard.
        ///
        /// The path of a file is the line which is shown for it, so that is what is copied when a
        /// line of a file is not selected.
        fn yank_line(&mut self) -> Option<Effect> {
            let file_hit: &FileHit = match self.hit() {
                Some(file_hit) => file_hit,
                None => {
                    return None;
                }
            };

            let (what, contents): (&str, String) = match self.line_hit_number() {
                Some(line_hit_number) => {
                    let line_hit: &LineHit = &file_hit.line_hits()[line_hit_number];
                    ("yanked line", line_hit.line().to_string())
                }
                None => (
                    "yanked path",
                    file_hit.path().to_path_buf().to_string_lossy().to_string(),
                ),
            };

            let mut clipboard = Clipboard::new();
            clipboard.copy(contents);

            self.message = Some(CommandMessage::Ran(String::from(what)));
            None
        }

        /// Ask the daemon for the contents of the file so that they can be copied to the
        /// clipboard.
        fn yank_contents(&mut self) -> Option<Effect> {
            let path: PathBuf = match self.hit() {
                Some(file_hit) => file_hit.path().to_path_buf(),
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

            let params: &SearchPhraseResponseParams = match response.params() {
                ResponseParams::SearchPhrase(params) => params,
                _ => {
                    #[cfg(feature = "logging")]
                    log::error!("Unexpected response parameters.");
                    return None;
                }
            };

            self.hits.extend_from_slice(params.hits());
            self.files_searched = params.searched();
            self.duration = params.duration();

            if response.last() {
                self.pending_request = None;
            }

            if self.hits.is_empty() && !self.is_pending() {
                return Some(Effect::Unfocus);
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
                Action::Resize { size } => self.resize(size),
                Action::Unfocus => self.unfocus(),
                Action::Search { phrase } => self.search(&phrase),
                Action::Down => self.down(),
                Action::ReallyDown => self.really_down(),
                Action::ScrollDown => self.scroll_down(1),
                Action::Up => self.up(),
                Action::ReallyUp => self.really_up(),
                Action::ScrollUp => self.scroll_up(1),
                Action::Refresh => self.refresh(),
                Action::Edit => self.edit(),
                Action::Goto => self.goto(),
                Action::ReallyGoto => self.really_goto(),
                Action::YankWord => self.yank_word(),
                Action::YankLine => self.yank_line(),
                Action::YankContents => self.yank_contents(),
                Action::UnknownCommand { keys } => self.unknown_command(keys),
                Action::HandleResponse(response) => self.handle_response(response),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use test_case::test_case;

        #[test_case(&mut State::default(), 0, State::default(); "scrolling a default state by zero does nothing")]
        #[test_case(
            &mut State{
                size: Size{rows: 1, columns: 2},
                hits: vec![FileHit::new(Path::new(""), vec![LineHit::new(0, "")])],
                ..Default::default()
            },
            1,
            State{
                size: Size{rows: 1, columns: 2},
                hits: vec![FileHit::new(Path::new(""), vec![LineHit::new(0, "")])],
                ..Default::default()
            }
        )]
        #[test_case(
            &mut State{
                size: Size{rows: 2, columns: 5},
                hits: vec![
                    FileHit::new(Path::new(""), vec![LineHit::new(0, ""), LineHit::new(1, "")]),
                    FileHit::new(Path::new(""), vec![LineHit::new(0, "")]),
                ],
                file_offset: 1,
                line_offset: None,
                file_selected: 0,
                line_selected: None,
                ..Default::default()
            },
            1,
            State{
                size: Size{rows: 2, columns: 5},
                hits: vec![
                    FileHit::new(Path::new(""), vec![LineHit::new(0, ""), LineHit::new(1, "")]),
                    FileHit::new(Path::new(""), vec![LineHit::new(0, "")]),
                ],
                file_offset: 0,
                line_offset: Some(2),
                file_selected: 1,
                line_selected: None,
                ..Default::default()
            }
        )]
        fn test_scroll_up(state: &mut State, rows: usize, expected_state: State) {
            state.scroll_up(rows);

            assert_eq!(*state, expected_state);
        }
    }
}
use state::State;

mod action {
    use insh_api::Response;
    use rend::Size;

    #[derive(Clone)]
    pub enum Action {
        Resize { size: Size },
        Unfocus,
        Search { phrase: String },
        Down,
        ReallyDown,
        ScrollDown,
        Up,
        ReallyUp,
        ScrollUp,
        Refresh,
        Edit,
        Goto,
        ReallyGoto,
        YankWord,
        YankLine,
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
        Goto { dir: PathBuf, file: Option<PathBuf> },
        OpenVim(VimArgs),
        Bell,
        Request(Request),
    }
}
pub use effect::Effect;

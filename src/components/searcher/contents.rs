mod props {
    use crate::config::Config;
    use crate::rendering::Size;

    use std::path::PathBuf;

    pub struct Props {
        pub config: Config,
        pub directory: PathBuf,
        pub size: Size,
    }

    impl Props {
        pub fn new(config: Config, directory: PathBuf, size: Size) -> Self {
            Self {
                config,
                directory,
                size,
            }
        }
    }
}
pub use props::Props;

mod contents {
    use super::{Action, Effect, Event, Props, State};
    use crate::color::Color;
    use crate::phrase_searcher::{FileHit, LineHit};
    use crate::rendering::{Fabric, Size, Yarn};
    use crate::{Component, Stateful};

    use std::path::MAIN_SEPARATOR as PATH_SEPARATOR;

    use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

    pub struct Contents {
        state: State,
    }

    impl Component<Props, Event, Effect> for Contents {
        fn new(props: Props) -> Self {
            let state: State = State::from(props);
            Self { state }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Option<Action> = match event {
                Event::Search { phrase } => Some(Action::Search { phrase }),
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
                        code: KeyCode::Char('j'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => Some(Action::ScrollDown),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        modifiers: KeyModifiers::NONE,
                    }) => Some(Action::Up),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('K'),
                        modifiers: KeyModifiers::SHIFT,
                    }) => Some(Action::ReallyUp),
                    CrosstermEvent::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => Some(Action::ScrollUp),
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
            match self.state.searched() {
                false => Fabric::new(size),
                true => {
                    let file_hits: &Vec<FileHit> = self.state.hits();
                    if self.state.hits().is_empty() {
                        Fabric::center("No matches.", size)
                    } else {
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
                                let directory_string: String =
                                    self.state.directory().to_string_lossy().to_string();
                                path = path.strip_prefix(&directory_string).unwrap().to_string();
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
                                string.push_str(line_hit.line());

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
}
pub use contents::Contents;

mod event {
    use crate::rendering::Size;
    use crossterm::event::Event as CrosstermEvent;

    pub enum Event {
        Crossterm { event: CrosstermEvent },
        Search { phrase: String },
        Resize { size: Size },
    }
}
pub use event::Event;

mod state {
    use super::{Action, Effect, Props};
    use crate::clipboard::Clipboard;
    use crate::config::Config;
    use crate::data::Data;
    use crate::phrase_searcher::{FileHit, LineHit, PhraseSearcher};
    use crate::programs::{VimArgs, VimArgsBuilder};
    use crate::rendering::Size;
    use crate::Stateful;

    use std::cmp::Ordering;
    use std::path::{Path, PathBuf, MAIN_SEPARATOR as PATH_SEPARATOR};

    #[derive(Debug, PartialEq, Eq, Default)]
    pub struct State {
        config: Config,
        size: Size,
        directory: PathBuf,
        phrase: Option<String>,
        focussed: bool,
        searched: bool,
        hits: Vec<FileHit>,
        file_offset: usize,
        line_offset: Option<usize>,
        file_selected: usize,
        line_selected: Option<usize>,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                config: props.config,
                size: props.size,
                directory: props.directory,
                phrase: None,
                focussed: false,
                searched: false,
                hits: Vec::new(),
                file_offset: 0,
                line_offset: None,
                file_selected: 0,
                line_selected: None,
            }
        }
    }

    impl State {
        pub fn directory(&self) -> &Path {
            &self.directory
        }

        /// Return if the search contents are currently foccused on.
        pub fn focussed(&self) -> bool {
            self.focussed
        }

        pub fn searched(&self) -> bool {
            self.searched
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

            let phrase_searcher = PhraseSearcher::new(&self.directory, phrase);
            self.hits = phrase_searcher.collect();
            self.searched = true;

            self.add_to_history(phrase);

            self.file_offset = 0;
            self.line_offset = None;
            self.file_selected = 0;
            self.line_selected = None;

            if self.hits.is_empty() {
                Some(Effect::Unfocus)
            } else {
                None
            }
        }

        fn add_to_history(&self, phrase: &str) {
            let mut data: Data = Data::read();
            let max_length: usize = self.config.searcher.history.length;
            data.searcher.add_to_history(phrase, max_length);
            data.write();
            data.release();
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
                up_adjustment = self.size.rows - (number_of_line_hits + 1);
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
                let directory = path.parent().unwrap().to_path_buf();
                let file: Option<PathBuf> = if really {
                    Some(path.to_path_buf())
                } else {
                    None
                };

                return Some(Effect::Goto { directory, file });
            }
            None
        }

        /// If a file path is selected, copy it to the system clipboard. Else if the line of a file is selected, then copy it.
        fn yank(&mut self) -> Option<Effect> {
            self._yank(false)
        }

        /// If a file path is selected, copy the absolute file path to the system clipboard. Else if the line of a file is selected, then copy it.
        fn really_yank(&mut self) -> Option<Effect> {
            self._yank(true)
        }

        fn _yank(&mut self, really: bool) -> Option<Effect> {
            if let Some(file_hit) = self.hit() {
                let contents: String = match self.line_hit_number() {
                    Some(line_hit_number) => {
                        let line_hit: &LineHit = &file_hit.line_hits()[line_hit_number];
                        line_hit.line().to_string()
                    }
                    None => {
                        let mut path: String =
                            file_hit.path().to_path_buf().to_string_lossy().to_string();
                        if !really {
                            let directory_string: String =
                                self.directory().to_string_lossy().to_string();
                            path = path.strip_prefix(&directory_string).unwrap().to_string();
                            if path.starts_with(PATH_SEPARATOR) {
                                path = path.strip_prefix(PATH_SEPARATOR).unwrap().to_string();
                            }
                        }
                        path
                    }
                };
                let mut clipboard = Clipboard::new();
                clipboard.copy(contents);
            }
            None
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
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
                Action::Yank => self.yank(),
                Action::ReallyYank => self.really_yank(),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use test_case::test_case;

        #[test_case(&mut State::default(), 0, State::default();)]
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
            };
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
            };
        )]
        fn test_scroll_up(state: &mut State, rows: usize, expected_state: State) {
            state.scroll_up(rows);

            assert_eq!(*state, expected_state);
        }
    }
}
use state::State;

mod action {
    use crate::rendering::Size;

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

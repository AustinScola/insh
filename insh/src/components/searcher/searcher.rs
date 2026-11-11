//! Contains the [`Searcher`] component.

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use crate::config::Config;

    use rend::Size;

    use uuid::Uuid;

    /// The properties of the searcher.
    pub struct Props {
        /// The configuration.
        pub config: Config,
        /// The directory to search in.
        pub dir: PathBuf,
        /// The size of the searcher.
        pub size: Size,
        /// The phrase to start with.
        pub phrase: Option<String>,
        /// The pending request for the hits.
        pub pending_request: Option<Uuid>,
        /// Whether the phrase is being replaced rather than only looked for.
        pub replacing: bool,
    }

    impl Props {
        /// Return new properties.
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
                replacing: false,
            }
        }

        /// Return the same, but replacing the phrase rather than only looking for it.
        pub fn replacing(mut self) -> Self {
            self.replacing = true;
            self
        }
    }
}
pub use props::Props;

/// Contains the [`Searcher`] component.
mod searcher {
    use super::super::{ContentsEffect, ContentsEvent};
    use super::{Action, Effect, Focus, Props, State};
    use crate::components::common::{
        DirEffect, DirEvent, Footer, FooterProps, Hover, PaneContentsEvent, Phrase, PhraseEffect,
        PhraseEvent,
    };
    use crate::Stateful;

    use insh_api::{
        Request, RequestParams, Response, ResponseParams, SuggestDirRequestParams,
        SuggestSearchPhraseRequestParams, VisitDirRequestParams,
    };
    use rend::{Fabric, Location, Size};
    use term::{Key, KeyEvent, KeyMods, MouseButton, MouseEvent, MouseEventKind, TermEvent};
    use til::{Component, Event};

    use regex::Regex;

    /// A file contents searcher.
    pub struct Searcher {
        /// The state of the searcher.
        state: State,
    }

    impl Component<Props, PaneContentsEvent, Effect> for Searcher {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Self { state }
        }

        fn name(&self) -> String {
            match self.state.replacement {
                Some(_) => String::from("replacer"),
                None => String::from("searcher"),
            }
        }

        fn render(&self, size: Size) -> Fabric {
            match size.rows {
                0 => Fabric::new(size),
                1 => self.state.phrase().render(size),
                2 => {
                    let columns = size.columns;
                    let phrase_fabric = self.state.phrase().render(Size::new(1, columns));
                    let dir_fabric = self.state.dir.render(Size::new(1, columns));
                    dir_fabric.quilt_bottom(phrase_fabric)
                }
                rows => {
                    let columns = size.columns;

                    // NOTE: What the phrase is replaced with and what the names of the files have
                    // to match go right below the phrase, once there are any, with as many of them
                    // as fit above the footer.
                    let mut fabric: Fabric = Fabric::new(Size::new(0, columns));
                    let mut above: usize = 0;
                    for bar in self.state.bars().into_iter().take(rows - 1) {
                        let bar_fabric: Fabric = match bar {
                            Focus::Dir => self.state.dir.render(Size::new(1, columns)),
                            Focus::Phrase => self.state.phrase().render(Size::new(1, columns)),
                            Focus::Replacement => match &self.state.replacement {
                                Some(replacement) => replacement.render(Size::new(1, columns)),
                                None => continue,
                            },
                            Focus::Files => match &self.state.files {
                                Some(files) => files.render(Size::new(1, columns)),
                                None => continue,
                            },
                            Focus::Contents => continue,
                        };
                        fabric = fabric.quilt_bottom(bar_fabric);
                        above += 1;
                    }

                    if rows > above + 1 {
                        let contents_fabric = self
                            .state
                            .contents()
                            .render(Size::new(rows - above - 1, columns));
                        fabric = fabric.quilt_bottom(contents_fabric);
                    }

                    let footer_props = FooterProps::builder()
                        .name(self.name())
                        .info(self.state.contents())
                        .build();
                    let footer_fabric = Footer::new(footer_props).render(Size::new(1, columns));

                    fabric.quilt_bottom(footer_fabric)
                }
            }
        }

        fn handle(&mut self, event: PaneContentsEvent) -> Option<Effect> {
            match event {
                PaneContentsEvent::SetFocus { focused } => {
                    self.state
                        .contents
                        .handle(ContentsEvent::SetPaneFocus { focused });
                    self.state
                        .phrase
                        .handle(PhraseEvent::SetPaneFocus { focused });
                    self.state.dir.handle(DirEvent::SetPaneFocus { focused });
                    if let Some(files) = self.state.files.as_mut() {
                        files.handle(PhraseEvent::SetPaneFocus { focused });
                    }
                    if let Some(replacement) = self.state.replacement.as_mut() {
                        replacement.handle(PhraseEvent::SetPaneFocus { focused });
                    }
                    None
                }
                PaneContentsEvent::Response(response) => {
                    self.handle_event(Event::Response(response))
                }
                PaneContentsEvent::TermEvent(term_event) => {
                    self.handle_event(Event::TermEvent(term_event))
                }
            }
        }
    }

    impl Searcher {
        /// Return where in it looking back through it starts, which is the end of what is being
        /// typed or the start of the selected row.
        pub fn copy_start(&self) -> Option<Location> {
            let phrase: (usize, usize) = (1, self.state.phrase.end());
            let (row, column): (usize, usize) = match self.state.focus() {
                Focus::Dir => (0, self.state.dir.end()),
                Focus::Phrase => phrase,
                Focus::Files => (
                    self.state.row_of(Focus::Files),
                    self.state.files.as_ref().map_or(0, Phrase::end),
                ),
                Focus::Replacement => (
                    self.state.row_of(Focus::Replacement),
                    self.state.replacement.as_ref().map_or(0, Phrase::end),
                ),
                // The directory bar and what is being looked for are drawn above what was found,
                // which may be nothing yet.
                Focus::Contents => match self.state.contents.selected_row() {
                    Some(row) => (row + self.state.above(), 0),
                    None => phrase,
                },
            };

            Some(Location::builder().row(row).column(column).build())
        }

        /// Return whether the row is shown as being under the mouse when it is, which is only when
        /// pressing on it would do something.
        ///
        /// The directory bar is the top row and the phrase the one below it, followed by what the
        /// names of the files have to match once that has been asked for, and the hits are below
        /// those, above the footer.
        pub fn hovers(&self, row: usize) -> Option<Hover> {
            match self.state.focus_at(row)? {
                Focus::Contents => self
                    .state
                    .contents
                    .hovers(row - self.state.above())
                    .then_some(Hover::Text),
                under => (under != *self.state.focus()).then_some(Hover::Row),
            }
        }

        /// Do what was done with the mouse.
        ///
        /// Pressing on the directory bar, the phrase, what the names of the files have to match,
        /// or a hit makes it what the keys go to, and selects the hit, or opens it when it is
        /// selected already. The mouse wheel only scrolls the hits while they are what the keys go
        /// to.
        fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Option<Effect> {
            let above: usize = self.state.above();
            let in_contents = MouseEvent {
                row: mouse_event.row.saturating_sub(above),
                ..mouse_event.clone()
            };

            match (mouse_event.kind, mouse_event.row) {
                (MouseEventKind::Press(MouseButton::Left), row) if row < above => {
                    let bar: Focus = self.state.focus_at(row)?;
                    self.state.perform(Action::TakeFocus(bar))
                }
                // NOTE: There is nothing to go to before anything has been found.
                // NOTE: The hits are told before they become what the keys go to, since pressing on
                // the one which is selected and focused already opens it. Pressing between two
                // files does nothing at all.
                (MouseEventKind::Press(MouseButton::Left), row) => {
                    self.state.contents.selected_row()?;
                    if !self.state.contents.shows(row - above) {
                        return None;
                    }
                    let contents_effect: Option<ContentsEffect> = self
                        .state
                        .contents
                        .handle(ContentsEvent::TermEvent(TermEvent::Mouse(in_contents)));
                    self.state.perform(Action::TakeFocus(Focus::Contents));
                    self.handle_contents_effect(contents_effect)
                }
                (MouseEventKind::ScrollUp | MouseEventKind::ScrollDown, row)
                    if row >= above && *self.state.focus() == Focus::Contents =>
                {
                    self.state
                        .contents
                        .handle(ContentsEvent::TermEvent(TermEvent::Mouse(in_contents)));
                    None
                }
                _ => None,
            }
        }

        /// Do what the terminal event or the response says.
        fn handle_event(&mut self, event: Event<Response>) -> Option<Effect> {
            // NOTE: Whatever could not be replaced is rung about, since the line it was in is
            // marked as replaced all the same.
            if let Event::Response(response) = &event {
                if let ResponseParams::ReplacePhrase(params) = response.params() {
                    return match params.failures().is_empty() {
                        true => None,
                        false => Some(Effect::Bell),
                    };
                }
            }

            match event {
                Event::TermEvent(TermEvent::Mouse(mouse_event)) => self.handle_mouse(mouse_event),
                Event::TermEvent(TermEvent::Resize(size)) => {
                    self.state.resize(size);
                    None
                }
                // A completion for the directory bar can arrive after the bar has stopped being
                // typed in, so it is routed by what it is rather than by what is focused on.
                Event::Response(response)
                    if matches!(response.params(), ResponseParams::SuggestDir(_)) =>
                {
                    let completion = match response.params() {
                        ResponseParams::SuggestDir(params) => params.suggestion().clone(),
                        _ => unreachable!(),
                    };
                    let dir_event = DirEvent::Completion {
                        uuid: *response.uuid(),
                        completion,
                    };
                    let dir_effect = self.state.dir.handle(dir_event);
                    self.handle_dir_effect(dir_effect)
                }
                // The key is not bound in the directory bar itself, so that pressing it again does
                // not throw away what has been typed in.
                Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
                    key: Key::Char('d'),
                    mods: KeyMods::CONTROL,
                })) if !matches!(self.state.focus(), Focus::Dir) => {
                    self.state.perform(Action::FocusDir)
                }
                Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
                    key: Key::Char('f'),
                    mods: KeyMods::NONE,
                })) if matches!(self.state.focus(), Focus::Contents) => {
                    self.state.perform(Action::FocusFiles)
                }
                // NOTE: Lower case decides about the selected line and goes on to the next one,
                // and upper case decides the same about every line which is left.
                Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
                    key: Key::Char(key @ ('y' | 'n' | 'Y' | 'N')),
                    mods: KeyMods::NONE | KeyMods::SHIFT,
                })) if matches!(self.state.focus(), Focus::Contents)
                    && self.state.replacement.is_some() =>
                {
                    let decide = ContentsEvent::Decide {
                        replace: key.eq_ignore_ascii_case(&'y'),
                        rest: key.is_ascii_uppercase(),
                    };
                    match self.state.contents.handle(decide) {
                        Some(ContentsEffect::Request(request)) => Some(Effect::Request(request)),
                        _ => None,
                    }
                }
                _ => match self.state.focus() {
                    Focus::Dir => {
                        let term_event = match event {
                            Event::TermEvent(term_event) => term_event,
                            // A response for a search which is still streaming arrives here when
                            // the directory bar is focused before the search is done.
                            Event::Response(_) => {
                                #[cfg(feature = "logging")]
                                log::debug!(
                                    "Ignoring a response for a search which is not focused."
                                );
                                return None;
                            }
                        };
                        let dir_effect = self.state.dir.handle(DirEvent::TermEvent(term_event));
                        self.handle_dir_effect(dir_effect)
                    }
                    Focus::Phrase => {
                        let phrase_event = match event {
                            Event::TermEvent(term_event) => PhraseEvent::TermEvent(term_event),
                            Event::Response(response) => {
                                let suggestion = match response.params() {
                                    ResponseParams::SuggestSearchPhrase(params) => {
                                        params.suggestion().clone()
                                    }
                                    _ => {
                                        #[cfg(feature = "logging")]
                                        log::error!("Unexpected response parameters.");
                                        return None;
                                    }
                                };
                                PhraseEvent::Completion {
                                    uuid: *response.uuid(),
                                    completion: suggestion,
                                }
                            }
                        };
                        let phrase_effect = self.state.phrase.handle(phrase_event);
                        match phrase_effect {
                            // What the phrase is replaced with is typed next, when it is.
                            Some(PhraseEffect::Enter { phrase }) => match self.state.replacement {
                                Some(_) => {
                                    self.state.perform(Action::TakeFocus(Focus::Replacement))
                                }
                                None => self.search(phrase),
                            },
                            Some(PhraseEffect::Bell) => Some(Effect::Bell),
                            Some(PhraseEffect::RequestCompletion { uuid, partial }) => {
                                let params = RequestParams::SuggestSearchPhrase(
                                    SuggestSearchPhraseRequestParams::builder()
                                        .partial(partial)
                                        .build(),
                                );
                                let request = Request::builder().uuid(uuid).params(params).build();
                                Some(Effect::Request(request))
                            }
                            Some(PhraseEffect::Quit) => self.state.perform(Action::Quit),
                            None => None,
                        }
                    }
                    Focus::Replacement => {
                        let term_event = match event {
                            Event::TermEvent(term_event) => term_event,
                            Event::Response(response) => {
                                self.state
                                    .contents
                                    .handle(ContentsEvent::Response(response));
                                return None;
                            }
                        };
                        let replacement = self.state.replacement.as_mut()?;
                        match replacement.handle(PhraseEvent::TermEvent(term_event)) {
                            Some(PhraseEffect::Enter { .. }) => {
                                let phrase: String = self.state.phrase.value().to_string();
                                self.search(phrase)
                            }
                            Some(PhraseEffect::Bell) => Some(Effect::Bell),
                            Some(PhraseEffect::Quit) => self.state.perform(Action::Quit),
                            // What the phrase is replaced with is never completed.
                            Some(PhraseEffect::RequestCompletion { .. }) | None => None,
                        }
                    }
                    Focus::Files => {
                        let term_event = match event {
                            Event::TermEvent(term_event) => term_event,
                            // A response for a search which is still streaming arrives here when
                            // the files are focused before the search is done.
                            Event::Response(response) => {
                                self.state
                                    .contents
                                    .handle(ContentsEvent::Response(response));
                                return None;
                            }
                        };
                        let files = self.state.files.as_mut()?;
                        match files.handle(PhraseEvent::TermEvent(term_event)) {
                            Some(PhraseEffect::Enter { phrase: files }) => {
                                // NOTE: A pattern which is not a regex is complained about here,
                                // where it can be typed again, rather than searching every file.
                                if Regex::new(&files).is_err() {
                                    return Some(Effect::Bell);
                                }
                                let phrase: String = self.state.phrase.value().to_string();
                                self.search(phrase)
                            }
                            Some(PhraseEffect::Bell) => Some(Effect::Bell),
                            Some(PhraseEffect::Quit) => self.state.perform(Action::Quit),
                            // What the names of the files have to match is never completed.
                            Some(PhraseEffect::RequestCompletion { .. }) | None => None,
                        }
                    }
                    Focus::Contents => {
                        let contents_event = match event {
                            Event::Response(response) => ContentsEvent::Response(response),
                            Event::TermEvent(term_event) => ContentsEvent::TermEvent(term_event),
                        };
                        let contents_effect = self.state.contents.handle(contents_event);
                        self.handle_contents_effect(contents_effect)
                    }
                },
            }
        }

        /// Return the effect of what the hits did.
        fn handle_contents_effect(
            &mut self,
            contents_effect: Option<ContentsEffect>,
        ) -> Option<Effect> {
            let action: Option<Action> = match contents_effect {
                Some(ContentsEffect::Unfocus) => {
                    self.state.phrase.handle(PhraseEvent::Focus);
                    Some(Action::FocusPhrase)
                }
                Some(ContentsEffect::Request(request)) => {
                    return Some(Effect::Request(request));
                }
                Some(ContentsEffect::Goto { dir, file }) => Some(Action::Goto { dir, file }),
                Some(ContentsEffect::OpenVim(vim_args)) => Some(Action::OpenVim(vim_args)),
                Some(ContentsEffect::Bell) => {
                    return Some(Effect::Bell);
                }
                None => None,
            };

            if let Some(action) = action {
                self.state.perform(action)
            } else {
                None
            }
        }

        /// Search for the phrase in the files whose names match what has been typed for them.
        fn search(&mut self, phrase: String) -> Option<Effect> {
            // NOTE: Nothing is looked for in order to replace it with something, since nothing is
            // in every line of every file, between every character of it.
            if self.state.replacement.is_some() && phrase.is_empty() {
                return Some(Effect::Bell);
            }

            let files: Option<String> = self.state.files_pattern();
            let replacement: Option<String> = self
                .state
                .replacement
                .as_ref()
                .map(|replacement| replacement.value().to_string());
            self.state.perform(Action::FocusContents);
            match self.state.contents.handle(ContentsEvent::Search {
                phrase,
                files,
                replacement,
            }) {
                Some(ContentsEffect::Request(request)) => Some(Effect::Request(request)),
                _ => None,
            }
        }

        /// Return the effect of what the directory bar did.
        fn handle_dir_effect(&mut self, dir_effect: Option<DirEffect>) -> Option<Effect> {
            match dir_effect {
                Some(DirEffect::RequestCompletion { uuid, partial }) => {
                    let params = RequestParams::SuggestDir(
                        SuggestDirRequestParams::builder().partial(partial).build(),
                    );
                    let request = Request::builder().uuid(uuid).params(params).build();
                    Some(Effect::Request(request))
                }
                Some(DirEffect::Enter { dir }) => {
                    let request = Request::builder()
                        .params(RequestParams::VisitDir(
                            VisitDirRequestParams::builder().dir(dir.clone()).build(),
                        ))
                        .build();
                    self.state.contents.handle(ContentsEvent::SetDir { dir });
                    self.state.phrase.handle(PhraseEvent::Focus);
                    self.state.perform(Action::FocusPhrase);
                    Some(Effect::Request(request))
                }
                Some(DirEffect::Unfocus) => {
                    self.state.phrase.handle(PhraseEvent::Focus);
                    self.state.perform(Action::FocusPhrase)
                }
                Some(DirEffect::Bell) => Some(Effect::Bell),
                None => None,
            }
        }
    }
}
pub use searcher::Searcher;

/// Contains the [`Effect`] enum.
mod effect {
    use std::path::PathBuf;

    use crate::programs::VimArgs;

    use insh_api::Request;

    /// A searcher effect.
    pub enum Effect {
        /// Browse a directory.
        Goto {
            /// The directory to browse.
            dir: PathBuf,
            /// The file to select.
            file: Option<PathBuf>,
        },
        /// Edit a file.
        OpenVim(VimArgs),
        /// Ring the bell.
        Bell,
        /// Send a request.
        Request(Request),
        /// Quit.
        Quit {
            /// The directory which was being searched in.
            dir: PathBuf,
        },
    }
}
pub use effect::Effect;

/// Contains the [`State`] struct.
mod state {
    use std::path::PathBuf;

    use super::super::{Contents, ContentsEvent, ContentsProps};
    use super::{Action, Effect, Props};
    use crate::components::common::{Dir, DirEvent, DirProps, Phrase, PhraseEvent, PhraseProps};
    use crate::programs::VimArgs;
    use crate::Stateful;

    use rend::Size;
    use term::TermEvent;
    use til::Component;

    /// The state of the searcher.
    pub struct State {
        /// What is focused on.
        focus: Focus,
        /// The directory bar.
        pub dir: Dir,
        /// The phrase typed in.
        pub phrase: Phrase,
        /// What the names of the files to search have to match, once it has been asked for.
        pub files: Option<Phrase>,
        /// What the phrase is replaced with, when it is being replaced.
        pub replacement: Option<Phrase>,
        /// The hits found.
        pub contents: Contents,
        /// The size of the searcher.
        size: Size,
    }

    impl State {
        /// Return what is focused on.
        pub fn focus(&self) -> &Focus {
            &self.focus
        }
        /// Return the phrase typed in.
        pub fn phrase(&self) -> &Phrase {
            &self.phrase
        }

        /// Return the hits found.
        pub fn contents(&self) -> &Contents {
            &self.contents
        }

        /// Return the bars drawn above the hits, from the top down.
        pub fn bars(&self) -> Vec<Focus> {
            let mut bars: Vec<Focus> = vec![Focus::Dir, Focus::Phrase];
            if self.replacement.is_some() {
                bars.push(Focus::Replacement);
            }
            if self.files.is_some() {
                bars.push(Focus::Files);
            }
            bars
        }

        /// Return how many rows are drawn above the hits.
        pub fn above(&self) -> usize {
            self.bars().len()
        }

        /// Return which row the bar is drawn in.
        pub fn row_of(&self, bar: Focus) -> usize {
            self.bars()
                .iter()
                .position(|drawn| *drawn == bar)
                .unwrap_or_default()
        }

        /// Return what is drawn in the row, which is nothing in particular for the footer.
        pub fn focus_at(&self, row: usize) -> Option<Focus> {
            let bars: Vec<Focus> = self.bars();
            match bars.get(row) {
                Some(bar) => Some(*bar),
                None if row < bars.len() + self.contents.rows() => Some(Focus::Contents),
                None => None,
            }
        }

        /// Return what the names of the files to search have to match, which is nothing when
        /// nothing has been typed for it.
        pub fn files_pattern(&self) -> Option<String> {
            self.files
                .as_ref()
                .map(|files| files.value().to_string())
                .filter(|files| !files.is_empty())
        }

        /// Take note of how big the searcher is, and tell the hits how big they are.
        pub fn resize(&mut self, size: Size) {
            self.size = size;
            let contents_size = Size::new(size.rows.saturating_sub(self.above() + 1), size.columns);
            self.contents
                .handle(ContentsEvent::TermEvent(TermEvent::Resize(contents_size)));
        }

        /// Stop sending the events to what the names of the files have to match, and to what the
        /// phrase is replaced with.
        fn unfocus_files(&mut self) {
            if let Some(files) = self.files.as_mut() {
                files.handle(PhraseEvent::Unfocus);
            }
            if let Some(replacement) = self.replacement.as_mut() {
                replacement.handle(PhraseEvent::Unfocus);
            }
        }

        /// Send the events to the phrase.
        fn focus_phrase(&mut self) -> Option<Effect> {
            self.unfocus_files();
            self.focus = Focus::Phrase;
            None
        }

        /// Send the events to the hits.
        fn focus_contents(&mut self) -> Option<Effect> {
            self.unfocus_files();
            self.focus = Focus::Contents;
            None
        }

        /// Send the events to what the names of the files have to match, which is shown below the
        /// phrase from the first time it is.
        fn focus_files(&mut self) -> Option<Effect> {
            if self.files.is_none() {
                let props = PhraseProps::builder()
                    .label(String::from("Files: "))
                    .build();
                self.files = Some(Phrase::new(props));
                self.resize(self.size);
            }

            self.dir.handle(DirEvent::Unfocus);
            self.phrase.handle(PhraseEvent::Unfocus);
            self.contents.handle(ContentsEvent::Unfocus);
            if let Some(replacement) = self.replacement.as_mut() {
                replacement.handle(PhraseEvent::Unfocus);
            }
            if let Some(files) = self.files.as_mut() {
                files.handle(PhraseEvent::Focus);
            }
            self.focus = Focus::Files;
            None
        }

        /// Send the events to the directory bar.
        fn focus_dir(&mut self) -> Option<Effect> {
            // Otherwise the phrase or the selected hit goes on showing itself as focused alongside
            // the directory. The contents say that they are unfocused, which is what is being done
            // here anyway.
            self.phrase.handle(PhraseEvent::Unfocus);
            self.contents.handle(ContentsEvent::Unfocus);
            self.unfocus_files();
            self.dir.handle(DirEvent::Focus);
            self.focus = Focus::Dir;
            None
        }

        /// Browse a directory.
        fn goto(&mut self, dir: PathBuf, file: Option<PathBuf>) -> Option<Effect> {
            Some(Effect::Goto { dir, file })
        }

        /// Edit a file.
        fn open_vim(&mut self, vim_args: VimArgs) -> Option<Effect> {
            Some(Effect::OpenVim(vim_args))
        }

        /// Send the events to whatever was pressed on, giving up on what was being typed in the
        /// directory bar if that is what they went to before.
        fn take_focus(&mut self, focus: Focus) -> Option<Effect> {
            if focus == self.focus {
                return None;
            }
            if focus == Focus::Files {
                return self.focus_files();
            }

            match self.focus {
                Focus::Dir => self.dir.handle(DirEvent::Unfocus),
                Focus::Phrase => self.phrase.handle(PhraseEvent::Unfocus).and(None),
                Focus::Files | Focus::Replacement => {
                    self.unfocus_files();
                    None
                }
                Focus::Contents => self.contents.handle(ContentsEvent::Unfocus).and(None),
            };
            match focus {
                Focus::Dir => self.dir.handle(DirEvent::Focus),
                Focus::Phrase => self.phrase.handle(PhraseEvent::Focus).and(None),
                Focus::Replacement => self
                    .replacement
                    .as_mut()
                    .and_then(|replacement| replacement.handle(PhraseEvent::Focus))
                    .and(None),
                Focus::Contents => self.contents.handle(ContentsEvent::Focus).and(None),
                Focus::Files => None,
            };
            self.focus = focus;

            None
        }

        /// Quit.
        fn quit(&mut self) -> Option<Effect> {
            Some(Effect::Quit {
                dir: self.dir.dir().to_path_buf(),
            })
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::FocusPhrase => self.focus_phrase(),
                Action::FocusContents => self.focus_contents(),
                Action::FocusDir => self.focus_dir(),
                Action::FocusFiles => self.focus_files(),
                Action::TakeFocus(focus) => self.take_focus(focus),
                Action::Goto { dir, file } => self.goto(dir, file),
                Action::OpenVim(vim_args) => self.open_vim(vim_args),
                Action::Quit => self.quit(),
            }
        }
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let dir_props = DirProps::builder().dir(props.dir.clone()).build();
            let dir = Dir::new(dir_props);

            // NOTE: What is looked for is said to be what is found when it is being replaced, so
            // that the two bars are told apart.
            let label: &str = match props.replacing {
                true => "Find: ",
                false => "Search: ",
            };
            let phrase_props = PhraseProps::builder()
                .completable(true)
                .value(props.phrase.clone())
                .label(String::from(label))
                .build();
            let mut phrase = Phrase::new(phrase_props);
            let replacement: Option<Phrase> = props.replacing.then(|| {
                let mut replacement = Phrase::new(
                    PhraseProps::builder()
                        .label(String::from("Replace: "))
                        .build(),
                );
                replacement.handle(PhraseEvent::Unfocus);
                replacement
            });

            // The directory bar, the phrase, what replaces it, and the footer are what is not the
            // hits.
            let not_hits: usize = 3 + usize::from(props.replacing);
            let contents_size =
                Size::new(props.size.rows.saturating_sub(not_hits), props.size.columns);
            let contents_props = ContentsProps::new(
                props.config,
                props.dir,
                contents_size,
                props.phrase.clone(),
                props.pending_request,
            );
            let contents = Contents::new(contents_props);

            let focus = if props.phrase.is_some() {
                phrase.handle(PhraseEvent::Unfocus);
                Focus::Contents
            } else {
                Focus::default()
            };

            Self {
                focus,
                dir,
                phrase,
                files: None,
                replacement,
                contents,
                size: props.size,
            }
        }
    }

    /// What the searcher is focused on.
    #[derive(Default, Clone, Copy, PartialEq, Eq)]
    pub enum Focus {
        /// The phrase typed in.
        #[default]
        Phrase,
        /// The hits found.
        Contents,
        /// The directory bar.
        Dir,
        /// What the names of the files to search have to match.
        Files,
        /// What the phrase is replaced with.
        Replacement,
    }
}
use state::{Focus, State};

/// Contains the [`Action`] enum.
mod action {
    use std::path::PathBuf;

    use super::Focus;
    use crate::programs::VimArgs;

    /// A searcher action.
    pub enum Action {
        /// Send the events to whatever was pressed on.
        TakeFocus(Focus),
        /// Send the events to the phrase.
        FocusPhrase,
        /// Send the events to the hits.
        FocusContents,
        /// Send the events to the directory bar.
        FocusDir,
        /// Send the events to what the names of the files to search have to match.
        FocusFiles,
        /// Browse a directory.
        Goto {
            /// The directory to browse.
            dir: PathBuf,
            /// The file to select.
            file: Option<PathBuf>,
        },
        /// Edit a file.
        OpenVim(VimArgs),
        /// Quit.
        Quit,
    }
}
pub use action::Action;

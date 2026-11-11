//! Contains the [`FileCreator`] component.

/// Contains the [`Purpose`] enum.
mod purpose {
    use file_type::FileType;

    /// What a file is being named for.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Purpose {
        /// Making a new file of the given type.
        Create(FileType),
        /// Editing the file, whether or not there is one by that name yet.
        Edit,
    }
}
pub use purpose::Purpose;

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use super::Purpose;

    use typed_builder::TypedBuilder;

    /// The properties of the file creator.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// The directory the file is in.
        dir: PathBuf,
        /// What the file is being named for.
        purpose: Purpose,
    }

    impl Props {
        /// Return the directory the file is in.
        pub fn dir(&self) -> &PathBuf {
            &self.dir
        }

        /// Return what the file is being named for.
        pub fn purpose(&self) -> Purpose {
            self.purpose
        }
    }
}
pub use props::Props;

/// Contains the [`FileCreator`] component.
mod file_creator {
    use std::path::Path;

    use super::Event;
    use super::{Action, Effect, Focus, Props, State};
    use crate::components::common::{
        DirEffect, DirEvent, Footer, FooterProps, Hover, PhraseEffect, PhraseEvent,
    };
    use crate::Stateful;

    use insh_api::{
        Request, RequestParams, ResponseParams, SuggestDirRequestParams, SuggestFileRequestParams,
        VisitDirRequestParams,
    };
    use rend::{Fabric, Location, Size};
    use term::{Key, KeyEvent, KeyMods, MouseButton, MouseEventKind, TermEvent};
    use til::Component;

    /// A file creator.
    pub struct FileCreator {
        /// The state of the file creator.
        state: State,
    }

    impl Component<Props, Event, Effect> for FileCreator {
        fn new(props: Props) -> Self {
            Self {
                state: State::from(props),
            }
        }

        fn name(&self) -> String {
            String::from(self.state.name())
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            // A completion for the directory bar can arrive after the bar has stopped being typed
            // in, so it is routed by what it is rather than by what is focused on.
            if let Event::Response(response) = &event {
                if let ResponseParams::SuggestDir(params) = response.params() {
                    let dir_event = DirEvent::Completion {
                        uuid: *response.uuid(),
                        completion: params.suggestion().clone(),
                    };
                    let dir_effect = self.state.dir_component.handle(dir_event);
                    return self.handle_dir_effect(dir_effect);
                }
                if let ResponseParams::SuggestFile(params) = response.params() {
                    let phrase_event = PhraseEvent::Completion {
                        uuid: *response.uuid(),
                        completion: params.suggestion().clone(),
                    };
                    self.state.phrase.handle(phrase_event);
                    return None;
                }
            }

            // The key is not bound in the directory bar itself, so that pressing it again does not
            // throw away what has been typed in.
            if !matches!(self.state.focus(), Focus::Dir) {
                if let Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
                    key: Key::Char('d'),
                    mods: KeyMods::CONTROL,
                })) = event
                {
                    return self.state.perform(Action::FocusDir);
                }
            }

            // NOTE: Pressing on the directory bar or the name makes it what the keys go to, giving up
            // on whatever was being typed in the directory bar.
            if let Event::TermEvent(TermEvent::Mouse(mouse_event)) = &event {
                let pressed: bool = mouse_event.kind == MouseEventKind::Press(MouseButton::Left);
                return match (pressed, mouse_event.row, self.state.focus()) {
                    (true, 0, Focus::Phrase) => {
                        self.state.phrase.handle(PhraseEvent::Unfocus);
                        self.state.perform(Action::FocusDir)
                    }
                    (true, 1, Focus::Dir) => {
                        self.state.dir_component.handle(DirEvent::Unfocus);
                        self.state.phrase.handle(PhraseEvent::Focus);
                        self.state.perform(Action::FocusPhrase)
                    }
                    _ => None,
                };
            }

            let mut action: Option<Action> = None;

            match event {
                Event::SetFocus { focused } => {
                    self.state
                        .phrase
                        .handle(PhraseEvent::SetPaneFocus { focused });
                    self.state
                        .dir_component
                        .handle(DirEvent::SetPaneFocus { focused });
                }
                Event::TermEvent(term_event) => match self.state.focus() {
                    Focus::Dir => {
                        let dir_effect = self
                            .state
                            .dir_component
                            .handle(DirEvent::TermEvent(term_event));
                        return self.handle_dir_effect(dir_effect);
                    }
                    Focus::Phrase => {
                        let phrase_event = PhraseEvent::TermEvent(term_event);
                        let phrase_effect = self.state.phrase.handle(phrase_event);
                        match phrase_effect {
                            Some(PhraseEffect::Enter { phrase }) => {
                                action = Some(Action::CreateFile { filename: phrase });
                            }
                            Some(PhraseEffect::Bell) => {
                                action = Some(Action::Bell);
                            }
                            // NOTE: Only the name of a file to edit is completed, since one which is
                            // being made does not exist yet.
                            Some(PhraseEffect::RequestCompletion { uuid, partial }) => {
                                let params = RequestParams::SuggestFile(
                                    SuggestFileRequestParams::builder()
                                        .dir(self.state.dir().to_path_buf())
                                        .partial(partial)
                                        .build(),
                                );
                                let request = Request::builder().uuid(uuid).params(params).build();
                                return Some(Effect::Request(request));
                            }
                            Some(PhraseEffect::Quit) => {
                                action = Some(Action::Quit);
                            }
                            None => {}
                        }
                    }
                },
                Event::Response(response) => {
                    action = Some(Action::HandleResponse(response));
                }
            }

            if let Some(action) = action {
                self.state.perform(action)
            } else {
                None
            }
        }

        fn render(&self, size: Size) -> Fabric {
            match size.rows {
                0 => Fabric::new(size),
                1 => self.state.phrase.render(size),
                2 => {
                    let columns = size.columns;
                    let phrase_fabric = self.state.phrase.render(Size::new(1, columns));
                    let dir_fabric = self.state.dir_component.render(Size::new(1, columns));
                    dir_fabric.quilt_bottom(phrase_fabric)
                }
                rows => {
                    let columns = size.columns;
                    let dir_fabric = self.state.dir_component.render(Size::new(1, columns));
                    let mut fabric: Fabric = dir_fabric;

                    let phrase_fabric = self.state.phrase.render(Size::new(1, columns));
                    fabric = fabric.quilt_bottom(phrase_fabric);

                    if rows > 3 {
                        let error_size = Size::new(rows - 3, columns);
                        let error_fabric = match self.state.error() {
                            Some(error) => Fabric::center(error, error_size),
                            None => Fabric::new(error_size),
                        };
                        fabric = fabric.quilt_bottom(error_fabric);
                    }

                    let footer_props = FooterProps::builder()
                        .name(self.name())
                        .info(&self.state)
                        .build();
                    let footer_fabric = Footer::new(footer_props).render(Size::new(1, columns));

                    fabric.quilt_bottom(footer_fabric)
                }
            }
        }
    }

    impl FileCreator {
        /// Return whether the row is shown as being under the mouse when it is, which is only when
        /// it is not what the keys go to already.
        ///
        /// The directory bar is the top row and the name is the one below it.
        pub fn hovers(&self, row: usize) -> Option<Hover> {
            let hovers: bool = match row {
                0 => !matches!(self.state.focus(), Focus::Dir),
                1 => !matches!(self.state.focus(), Focus::Phrase),
                _ => false,
            };
            hovers.then_some(Hover::Row)
        }

        /// Return the directory the file is in.
        pub fn dir(&self) -> &Path {
            self.state.dir()
        }

        /// Return where in it looking back through it starts, which is the end of what is being
        /// typed.
        pub fn copy_start(&self) -> Option<Location> {
            let (row, column): (usize, usize) = match self.state.focus() {
                Focus::Dir => (0, self.state.dir_component.end()),
                Focus::Phrase => (1, self.state.phrase.end()),
            };

            Some(Location::builder().row(row).column(column).build())
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
                    self.state.perform(Action::SetDir { dir });
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
pub use file_creator::FileCreator;

/// Contains the [`Event`] enum.
mod event {
    use insh_api::Response;
    use term::TermEvent;

    /// A file creator event.
    pub enum Event {
        /// Whether what is typed goes to the pane it is in has changed.
        SetFocus {
            /// Whether it does.
            focused: bool,
        },
        /// A response.
        Response(Response),
        /// A terminal event.
        TermEvent(TermEvent),
    }
}
pub use event::Event;

/// Contains the [`State`] struct.
mod state {
    use std::path::{Path, PathBuf, MAIN_SEPARATOR as PATH_SEPARATOR};

    use super::{Action, Effect, Focus, Props, Purpose};
    use crate::args::FileLineColumn;
    use crate::components::common::{
        Dir, DirEvent, DirProps, FooterInfo, Phrase, PhraseEvent, PhraseProps,
    };
    use crate::programs::{VimArgs, VimArgsBuilder};
    use crate::Stateful;

    use file_type::FileType;
    use insh_api::{
        CreateFileRequestParams, CreateFileResponseParams, Request, RequestParams, Response,
        ResponseParams,
    };
    use til::Component;

    use uuid::Uuid;

    /// The state of the file creator.
    pub struct State {
        /// The directory to make the file in.
        dir: PathBuf,
        /// The directory bar.
        pub dir_component: Dir,
        /// The name typed in.
        pub phrase: Phrase,
        /// What the file is being named for.
        purpose: Purpose,
        /// What is focused on.
        focus: Focus,

        /// The pending request.
        pending_request: Option<Uuid>,
        /// The path of the file being made.
        pending_file: Option<PathBuf>,

        /// Why the file could not be made, if it could not be.
        error: Option<String>,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let dir_component_props = DirProps::builder().dir(props.dir().clone()).build();
            let dir_component = Dir::new(dir_component_props);

            Self {
                dir: props.dir().to_path_buf(),
                dir_component,
                phrase: Phrase::new(
                    PhraseProps::builder()
                        .completable(matches!(props.purpose(), Purpose::Edit))
                        .part_separator(PATH_SEPARATOR)
                        .build(),
                ),
                purpose: props.purpose(),
                focus: Focus::default(),
                pending_request: None,
                pending_file: None,
                error: None,
            }
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::CreateFile { filename } => self.create_file(&filename),
                Action::SetDir { dir } => self.set_dir(dir),
                Action::FocusDir => self.focus_dir(),
                Action::FocusPhrase => self.focus_phrase(),
                Action::HandleResponse(response) => self.handle_response(response),
                Action::Bell => self.bell(),
                Action::Quit => self.quit(),
            }
        }
    }

    /// The footer only names what is being created, since there is nothing else to show.
    impl FooterInfo for State {}

    impl State {
        /// Return what is focused on.
        pub fn focus(&self) -> &Focus {
            &self.focus
        }

        /// Return the directory the file is in.
        pub fn dir(&self) -> &Path {
            &self.dir
        }

        /// Make the file in a different directory.
        fn set_dir(&mut self, dir: PathBuf) -> Option<Effect> {
            self.dir = dir;
            None
        }

        /// Send the events to the directory bar.
        fn focus_dir(&mut self) -> Option<Effect> {
            // Otherwise the name goes on showing itself as focused alongside the directory.
            self.phrase.handle(PhraseEvent::Unfocus);
            self.dir_component.handle(DirEvent::Focus);
            self.focus = Focus::Dir;
            None
        }

        /// Send the events to the name.
        fn focus_phrase(&mut self) -> Option<Effect> {
            self.focus = Focus::Phrase;
            None
        }

        /// Return why the file could not be made, if it could not be.
        pub fn error(&self) -> &Option<String> {
            &self.error
        }

        /// Return the name of what is being created shown in the footer.
        pub fn name(&self) -> &str {
            match self.purpose {
                Purpose::Create(FileType::Dir) => "directory creator",
                Purpose::Create(_) => "file creator",
                Purpose::Edit => "file editor",
            }
        }

        /// Ask inshd to make the file, or edit it.
        fn create_file(&mut self, filename: &str) -> Option<Effect> {
            let mut path = self.dir.clone();
            path.push(filename);

            let file_type: FileType = match self.purpose {
                Purpose::Create(file_type) => file_type,
                // NOTE: Nothing is made for a file which is to be edited, since vim makes it once
                // it is written and leaving without writing it should leave nothing behind.
                Purpose::Edit => return Some(Effect::OpenVim(self.vim_args(filename))),
            };

            let request = Request::builder()
                .params(RequestParams::CreateFile(
                    CreateFileRequestParams::builder()
                        .path(path.clone())
                        .file_type(file_type)
                        .build(),
                ))
                .build();
            self.pending_request = Some(*request.uuid());
            self.pending_file = Some(path);

            Some(Effect::Request(request))
        }

        /// Return what vim is run with to edit the file, which may be followed by the line and
        /// column to start at the way it can be on the command line.
        fn vim_args(&self, filename: &str) -> VimArgs {
            let file_line_column: FileLineColumn = match filename.parse() {
                Ok(file_line_column) => file_line_column,
                Err(_) => FileLineColumn::new(Some(PathBuf::from(filename)), None, None),
            };

            let mut vim_args_builder = VimArgsBuilder::new();
            if let Some(file) = file_line_column.file() {
                vim_args_builder = vim_args_builder.path(&self.dir.join(file));
            }
            if let Some(line) = file_line_column.line() {
                vim_args_builder = vim_args_builder.line(line);
            }
            if let Some(column) = file_line_column.column() {
                vim_args_builder = vim_args_builder.column(column);
            }
            vim_args_builder.build()
        }

        /// Browse the new file, or show why it could not be made.
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

            let params: &CreateFileResponseParams = match response.params() {
                ResponseParams::CreateFile(params) => params,
                _ => {
                    #[cfg(feature = "logging")]
                    log::error!("Unexpected response parameters.");
                    return None;
                }
            };

            if let Err(error) = params.result() {
                self.error = Some(error.to_string());
                self.phrase.handle(PhraseEvent::Focus);
                return None;
            }

            Some(Effect::Browse {
                dir: self.dir.clone(),
                file: Some(self.pending_file.clone().unwrap()),
            })
        }

        /// Ring the bell.
        fn bell(&mut self) -> Option<Effect> {
            Some(Effect::Bell)
        }

        /// Quit.
        fn quit(&mut self) -> Option<Effect> {
            Some(Effect::Quit)
        }
    }
}
use state::State;

/// Contains the [`Focus`] enum.
mod focus {
    /// What the file creator is focused on.
    #[derive(Default)]
    pub enum Focus {
        /// The name typed in.
        #[default]
        Phrase,
        /// The directory bar.
        Dir,
    }
}
use focus::Focus;

/// Contains the [`Effect`] enum.
mod effect {
    use std::path::PathBuf;

    use crate::programs::VimArgs;

    use insh_api::Request;

    /// A file creator effect.
    pub enum Effect {
        /// Send a request.
        Request(Request),
        /// Browse a directory.
        Browse {
            /// The directory to browse.
            dir: PathBuf,
            /// The file to select.
            file: Option<PathBuf>,
        },
        /// Edit a file.
        OpenVim(VimArgs),
        /// Ring the bell.
        Bell,
        /// Quit.
        Quit,
    }
}
pub use effect::Effect;

/// Contains the [`Action`] enum.
mod action {
    use std::path::PathBuf;

    use insh_api::Response;

    /// A file creator action.
    pub enum Action {
        /// Make the file.
        CreateFile {
            /// The name to give it.
            filename: String,
        },
        /// Make the file in a different directory.
        SetDir {
            /// The directory to make it in.
            dir: PathBuf,
        },
        /// Send the events to the directory bar.
        FocusDir,
        /// Send the events to the name.
        FocusPhrase,
        /// Handle a response from inshd.
        HandleResponse(Response),
        /// Ring the bell.
        Bell,
        /// Quit.
        Quit,
    }
}
use action::Action;

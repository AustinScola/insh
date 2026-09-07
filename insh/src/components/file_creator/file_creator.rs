//! Contains the [`FileCreator`] component.

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use file_type::FileType;

    use typed_builder::TypedBuilder;

    /// The properties of the file creator.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// The directory to make the file in.
        dir: PathBuf,
        /// The type of file to make.
        file_type: FileType,
    }

    impl Props {
        /// Return the directory to make the file in.
        pub fn dir(&self) -> &PathBuf {
            &self.dir
        }

        /// Return the type of file to make.
        pub fn file_type(&self) -> FileType {
            self.file_type
        }
    }
}
pub use props::Props;

/// Contains the [`FileCreator`] component.
mod file_creator {
    use super::Event;
    use super::{Action, Effect, Props, State};
    use crate::components::common::{Footer, FooterProps, PhraseEffect, PhraseEvent};
    use crate::Stateful;

    use rend::{Fabric, Size};
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
            let mut action: Option<Action> = None;

            match event {
                Event::TermEvent(term_event) => {
                    let phrase_event = PhraseEvent::TermEvent(term_event);
                    let phrase_effect = self.state.phrase.handle(phrase_event);
                    match phrase_effect {
                        Some(PhraseEffect::Enter { phrase }) => {
                            action = Some(Action::CreateFile { filename: phrase });
                        }
                        Some(PhraseEffect::Bell) => {
                            action = Some(Action::Bell);
                        }
                        // The phrase here is never completable, so this is never emitted.
                        Some(PhraseEffect::RequestCompletion { .. }) => {
                            #[cfg(feature = "logging")]
                            log::warn!(
                                "The phrase is not completable but a completion was requested."
                            );
                        }
                        Some(PhraseEffect::Quit) => {
                            action = Some(Action::Quit);
                        }
                        None => {}
                    }
                }
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
                    let dir_fabric = self.state.dir_component().render(Size::new(1, columns));
                    dir_fabric.quilt_bottom(phrase_fabric)
                }
                rows => {
                    let columns = size.columns;
                    let dir_fabric = self.state.dir_component().render(Size::new(1, columns));
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
}
pub use file_creator::FileCreator;

/// Contains the [`Event`] enum.
mod event {
    use insh_api::Response;
    use term::TermEvent;

    /// A file creator event.
    pub enum Event {
        /// A response.
        Response(Response),
        /// A terminal event.
        TermEvent(TermEvent),
    }
}
pub use event::Event;

/// Contains the [`State`] struct.
mod state {
    use std::path::PathBuf;

    use super::{Action, Effect, Props};
    use crate::components::common::{Dir, DirProps, FooterInfo, Phrase, PhraseEvent};
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
        dir_component: Dir,
        /// The name typed in.
        pub phrase: Phrase,
        /// The type of file to make.
        file_type: FileType,

        /// The pending request.
        pending_request: Option<Uuid>,
        /// The path of the file being made.
        pending_file: Option<PathBuf>,

        /// Why the file could not be made, if it could not be.
        error: Option<String>,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let dir_component_props = DirProps::new(props.dir().clone());
            let dir_component = Dir::new(dir_component_props);

            Self {
                dir: props.dir().to_path_buf(),
                dir_component,
                phrase: Phrase::default(),
                file_type: props.file_type(),
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
                Action::HandleResponse(response) => self.handle_response(response),
                Action::Bell => self.bell(),
                Action::Quit => self.quit(),
            }
        }
    }

    /// The footer only names what is being created, since there is nothing else to show.
    impl FooterInfo for State {}

    impl State {
        /// Return the directory bar.
        pub fn dir_component(&self) -> &Dir {
            &self.dir_component
        }

        /// Return why the file could not be made, if it could not be.
        pub fn error(&self) -> &Option<String> {
            &self.error
        }

        /// Return the name of what is being created shown in the footer.
        pub fn name(&self) -> &str {
            match self.file_type {
                FileType::Dir => "directory creator",
                _ => "file creator",
            }
        }

        /// Ask inshd to make the file.
        fn create_file(&mut self, filename: &str) -> Option<Effect> {
            let mut path = self.dir.clone();
            path.push(filename);

            let request = Request::builder()
                .params(RequestParams::CreateFile(
                    CreateFileRequestParams::builder()
                        .path(path.clone())
                        .file_type(self.file_type)
                        .build(),
                ))
                .build();
            self.pending_request = Some(*request.uuid());
            self.pending_file = Some(path);

            Some(Effect::Request(request))
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

/// Contains the [`Effect`] enum.
mod effect {
    use std::path::PathBuf;

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
        /// Ring the bell.
        Bell,
        /// Quit.
        Quit,
    }
}
pub use effect::Effect;

/// Contains the [`Action`] enum.
mod action {
    use insh_api::Response;

    /// A file creator action.
    pub enum Action {
        /// Make the file.
        CreateFile {
            /// The name to give it.
            filename: String,
        },
        /// Handle a response from inshd.
        HandleResponse(Response),
        /// Ring the bell.
        Bell,
        /// Quit.
        Quit,
    }
}
use action::Action;

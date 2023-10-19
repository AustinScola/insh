mod props {
    use std::path::PathBuf;

    use typed_builder::TypedBuilder;

    use file_type::FileType;

    #[derive(TypedBuilder)]
    pub struct Props {
        directory: PathBuf,
        file_type: FileType,
    }

    impl Props {
        pub fn directory(&self) -> &PathBuf {
            &self.directory
        }

        pub fn file_type(&self) -> FileType {
            self.file_type
        }
    }
}
pub use props::Props;

mod file_creator {
    use rend::{Fabric, Size};
    use til::Component;

    use super::Event;
    use super::{Action, Effect, Props, State};
    use crate::components::common::{PhraseEffect, PhraseEvent};
    use crate::Stateful;

    pub struct FileCreator {
        state: State,
    }

    impl Component<Props, Event, Effect> for FileCreator {
        fn new(props: Props) -> Self {
            Self {
                state: State::from(props),
            }
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
                    let directory_fabric = self
                        .state
                        .directory_component()
                        .render(Size::new(1, columns));
                    directory_fabric.quilt_bottom(phrase_fabric)
                }
                rows => {
                    let columns = size.columns;
                    let directory_fabric = self
                        .state
                        .directory_component()
                        .render(Size::new(1, columns));
                    let mut fabric: Fabric = directory_fabric;

                    let phrase_fabric = self.state.phrase.render(Size::new(1, columns));
                    fabric = fabric.quilt_bottom(phrase_fabric);

                    match self.state.error() {
                        Some(error) => {
                            let error_fabric = Fabric::center(error, Size::new(rows - 2, columns));
                            fabric = fabric.quilt_bottom(error_fabric);
                        }
                        None => {
                            fabric.pad_bottom(rows);
                        }
                    }

                    fabric
                }
            }
        }
    }
}
pub use file_creator::FileCreator;

mod event {
    use insh_api::Response;
    use term::TermEvent;

    pub enum Event {
        Response(Response),
        TermEvent(TermEvent),
    }
}
pub use event::Event;

mod state {
    use std::path::PathBuf;

    use uuid::Uuid;

    use file_type::FileType;
    use insh_api::{
        CreateFileRequestParams, CreateFileResponseParams, Request, RequestParams, Response,
        ResponseParams,
    };
    use til::Component;

    use super::{Action, Effect, Props};
    use crate::components::common::PhraseEvent;
    use crate::components::common::{Directory, DirectoryProps, Phrase};
    use crate::Stateful;

    pub struct State {
        directory: PathBuf,
        directory_component: Directory,
        pub phrase: Phrase,
        file_type: FileType,

        pending_request: Option<Uuid>,
        pending_file: Option<PathBuf>,

        error: Option<String>,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let directory_component_props = DirectoryProps::new(props.directory().clone());
            let directory_component = Directory::new(directory_component_props);

            Self {
                directory: props.directory().to_path_buf(),
                directory_component,
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

    impl State {
        pub fn directory_component(&self) -> &Directory {
            &self.directory_component
        }

        pub fn error(&self) -> &Option<String> {
            &self.error
        }

        fn create_file(&mut self, filename: &str) -> Option<Effect> {
            let mut path = self.directory.clone();
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
                directory: self.directory.clone(),
                file: Some(self.pending_file.clone().unwrap()),
            })
        }

        fn bell(&mut self) -> Option<Effect> {
            Some(Effect::Bell)
        }

        fn quit(&mut self) -> Option<Effect> {
            Some(Effect::Quit)
        }
    }
}
use state::State;

mod effect {
    use std::path::PathBuf;

    use insh_api::Request;

    pub enum Effect {
        Request(Request),
        Browse {
            directory: PathBuf,
            file: Option<PathBuf>,
        },
        Bell,
        Quit,
    }
}
pub use effect::Effect;

mod action {
    use insh_api::Response;

    pub enum Action {
        CreateFile { filename: String },
        HandleResponse(Response),
        Bell,
        Quit,
    }
}
use action::Action;

//! Contains the [`Browser`] component.

use std::path::PathBuf;

use super::{Contents, ContentsEffect, ContentsEvent, ContentsProps};
use crate::components::common::{Dir, DirEffect, DirEvent, DirProps, Footer, FooterProps};
use crate::config::Config;
use crate::programs::VimArgs;
use crate::stateful::Stateful;

use file_type::FileType;
use insh_api::{
    Request, RequestParams, Response, ResponseParams, SuggestDirRequestParams,
    VisitDirRequestParams,
};
use rend::{Fabric, Size};
use term::{Key, KeyEvent, KeyMods, TermEvent};
use til::Component;

use typed_builder::TypedBuilder;
use uuid::Uuid;

/// The properties of the browser.
#[derive(TypedBuilder)]
pub struct Props {
    /// The configuration.
    config: Config,
    /// The directory to browse.
    dir: PathBuf,
    /// The size of the browser.
    size: Size,
    /// The file to select.
    #[builder(default)]
    file: Option<PathBuf>,
    /// The pending request for the files.
    #[builder(default)]
    pending_request: Option<Uuid>,
}

/// A file browser.
pub struct Browser {
    /// The state of the browser.
    state: State,
}

impl Component<Props, Event, Effect> for Browser {
    fn new(props: Props) -> Self {
        let state = State::from(props);
        Self { state }
    }

    fn name(&self) -> String {
        String::from("browser")
    }

    fn handle(&mut self, event: Event) -> Option<Effect> {
        match event {
            Event::Response(response) => {
                // A completion for the directory bar can arrive after the bar has stopped being
                // typed in, so responses are routed by what they are rather than by what is
                // focused on.
                if let ResponseParams::SuggestDir(params) = response.params() {
                    let dir_event = DirEvent::Completion {
                        uuid: *response.uuid(),
                        completion: params.suggestion().clone(),
                    };
                    let dir_effect: Option<DirEffect> = self.state.dir.handle(dir_event);
                    return self.handle_dir_effect(dir_effect);
                }

                let contents_effect: Option<ContentsEffect> = self
                    .state
                    .contents
                    .handle(ContentsEvent::Response(response));
                self.handle_contents_effect(contents_effect)
            }
            Event::TermEvent(TermEvent::Resize(size)) => {
                let size = Size::new(size.rows.saturating_sub(2), size.columns);
                self.state.contents.handle(ContentsEvent::Resize { size });
                None
            }
            // The key is not bound in the directory bar itself, so that pressing it again does not
            // throw away what has been typed in.
            Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('d'),
                mods: KeyMods::CONTROL,
            })) if !matches!(self.state.focus, Focus::Dir) => self.state.perform(Action::FocusDir),
            Event::TermEvent(term_event) => match self.state.focus {
                Focus::Dir => {
                    let dir_effect: Option<DirEffect> =
                        self.state.dir.handle(DirEvent::TermEvent(term_event));
                    self.handle_dir_effect(dir_effect)
                }
                Focus::Contents => {
                    let contents_effect: Option<ContentsEffect> = self
                        .state
                        .contents
                        .handle(ContentsEvent::Term { event: term_event });
                    self.handle_contents_effect(contents_effect)
                }
            },
        }
    }

    fn render(&self, size: Size) -> Fabric {
        match size.rows {
            0 => Fabric::new(size),
            1 => self.state.dir.render(size),
            rows => {
                let columns = size.columns;
                let mut fabric: Fabric = self.state.dir.render(Size::new(1, columns));

                if rows > 2 {
                    let contents_fabric: Fabric =
                        self.state.contents.render(Size::new(rows - 2, columns));
                    fabric = fabric.quilt_bottom(contents_fabric);
                }

                let footer_props = FooterProps::builder()
                    .name(self.name())
                    .info(&self.state.contents)
                    .build();
                let footer_fabric: Fabric = Footer::new(footer_props).render(Size::new(1, columns));

                fabric.quilt_bottom(footer_fabric)
            }
        }
    }
}

impl Browser {
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
                self.state.perform(Action::UnfocusDir);
                let visit_request = Request::builder()
                    .params(RequestParams::VisitDir(
                        VisitDirRequestParams::builder().dir(dir.clone()).build(),
                    ))
                    .build();
                let contents_effect: Option<ContentsEffect> =
                    self.state.contents.handle(ContentsEvent::SetDir { dir });
                match self.handle_contents_effect(contents_effect) {
                    Some(Effect::Request(request)) => {
                        Some(Effect::Requests(vec![visit_request, request]))
                    }
                    effect => effect,
                }
            }
            Some(DirEffect::Unfocus) => self.state.perform(Action::UnfocusDir),
            Some(DirEffect::Bell) => Some(Effect::Bell),
            None => None,
        }
    }

    /// Return the effect of what the contents did.
    fn handle_contents_effect(
        &mut self,
        contents_effect: Option<ContentsEffect>,
    ) -> Option<Effect> {
        match contents_effect {
            Some(ContentsEffect::SetDir {
                dir,
                get_files_request,
            }) => {
                let visit_request = Request::builder()
                    .params(RequestParams::VisitDir(
                        VisitDirRequestParams::builder().dir(dir.clone()).build(),
                    ))
                    .build();
                self.state.dir.handle(DirEvent::SetDir { dir });
                Some(Effect::Requests(vec![visit_request, get_files_request]))
            }
            Some(ContentsEffect::PopDir {
                dir,
                get_files_request,
            }) => {
                let visit_request = Request::builder()
                    .params(RequestParams::VisitDir(
                        VisitDirRequestParams::builder().dir(dir).build(),
                    ))
                    .build();
                self.state.dir.handle(DirEvent::PopDir);
                Some(Effect::Requests(vec![visit_request, get_files_request]))
            }
            Some(ContentsEffect::OpenFileCreator { dir, file_type }) => {
                Some(Effect::OpenFileCreator { dir, file_type })
            }
            Some(ContentsEffect::OpenFinder { dir }) => Some(Effect::OpenFinder { dir }),
            Some(ContentsEffect::OpenSearcher { dir }) => Some(Effect::OpenSearcher { dir }),
            Some(ContentsEffect::OpenVim(vim_args)) => Some(Effect::OpenVim(vim_args)),
            Some(ContentsEffect::RunBash { dir }) => Some(Effect::RunBash { dir }),
            Some(ContentsEffect::Bell) => Some(Effect::Bell),
            Some(ContentsEffect::Request(request)) => Some(Effect::Request(request)),
            None => None,
        }
    }
}

/// The state of the browser.
struct State {
    /// The directory bar.
    dir: Dir,
    /// The files in the directory.
    contents: Contents,
    /// What is focused on.
    focus: Focus,
}

impl From<Props> for State {
    fn from(props: Props) -> Self {
        let dir_props = DirProps::builder().dir(props.dir.clone()).build();
        let dir = Dir::new(dir_props);

        let contents_size = Size::new(props.size.rows.saturating_sub(2), props.size.columns);
        let contents_props = ContentsProps::builder()
            .config(props.config)
            .dir(props.dir)
            .size(contents_size)
            .file(props.file)
            .pending_request(props.pending_request)
            .build();
        let contents = Contents::new(contents_props);

        let focus = Focus::default();

        State {
            dir,
            contents,
            focus,
        }
    }
}

impl Stateful<Action, Effect> for State {
    fn perform(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::FocusDir => {
                // Otherwise the selected file goes on showing itself as focused alongside the
                // directory.
                self.contents.handle(ContentsEvent::Unfocus);
                self.dir.handle(DirEvent::Focus);
                self.focus = Focus::Dir;
            }
            Action::UnfocusDir => {
                self.contents.handle(ContentsEvent::Focus);
                self.focus = Focus::Contents;
            }
        }
        None
    }
}

/// What the browser is focused on.
#[derive(Default)]
enum Focus {
    /// The directory bar.
    Dir,
    /// The files in the directory.
    #[default]
    Contents,
}

/// A browser event.
pub enum Event {
    /// A response.
    Response(Response),
    /// A terminal event.
    TermEvent(TermEvent),
}

/// A browser action.
enum Action {
    /// Start typing in the directory bar.
    FocusDir,
    /// Stop typing in the directory bar.
    UnfocusDir,
}

/// A browser effect.
pub enum Effect {
    /// Make a new file.
    OpenFileCreator {
        /// The directory to make it in.
        dir: PathBuf,
        /// The type of file to make.
        file_type: FileType,
    },
    /// Find files by name.
    OpenFinder {
        /// The directory to look in.
        dir: PathBuf,
    },
    /// Search files for a phrase.
    OpenSearcher {
        /// The directory to search in.
        dir: PathBuf,
    },
    /// Edit a file.
    OpenVim(VimArgs),
    /// Run a shell.
    RunBash {
        /// The directory to run it in.
        dir: PathBuf,
    },
    /// Ring the bell.
    Bell,
    /// Send a request.
    Request(Request),
    /// Send more than one request, in the order they are in.
    Requests(Vec<Request>),
}

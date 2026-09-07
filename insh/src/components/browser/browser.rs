//! Contains the [`Browser`] component.

use std::path::PathBuf;

use super::{Contents, ContentsEffect, ContentsEvent, ContentsProps};
use crate::components::common::{Dir, DirEvent, DirProps, Footer, FooterProps};
use crate::config::Config;
use crate::programs::VimArgs;
use crate::stateful::Stateful;

use file_type::FileType;
use insh_api::Request;
use insh_api::Response;
use rend::{Fabric, Size};
use term::TermEvent;
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
        let mut effect: Option<Effect> = None;
        match event {
            Event::Response(response) => match self.state.focus {
                Focus::Contents => {
                    let contents_event: ContentsEvent = ContentsEvent::Response(response);
                    self.state.contents.handle(contents_event);
                }
            },
            Event::TermEvent(term_event) => {
                match term_event {
                    TermEvent::Resize(size) => {
                        let size = Size::new(size.rows.saturating_sub(2), size.columns);
                        self.state.contents.handle(ContentsEvent::Resize { size });
                    }
                    _ => {
                        match self.state.focus {
                            Focus::Contents => {
                                let contents_event: ContentsEvent =
                                    ContentsEvent::Term { event: term_event };
                                let contents_effect: Option<ContentsEffect> =
                                    self.state.contents.handle(contents_event);

                                match contents_effect {
                                    Some(ContentsEffect::SetDir {
                                        dir,
                                        get_files_request,
                                    }) => {
                                        let dir_event = DirEvent::SetDir { dir };
                                        self.state.dir.handle(dir_event);
                                        // TODO: What if the directory returns an effect here? Do we need to loop?
                                        effect = Some(Effect::Request(get_files_request));
                                    }
                                    Some(ContentsEffect::PopDir { get_files_request }) => {
                                        let dir_event = DirEvent::PopDir;
                                        self.state.dir.handle(dir_event);
                                        effect = Some(Effect::Request(get_files_request));
                                    }
                                    Some(ContentsEffect::OpenFileCreator { dir, file_type }) => {
                                        effect = Some(Effect::OpenFileCreator { dir, file_type });
                                    }
                                    Some(ContentsEffect::OpenFinder { dir }) => {
                                        effect = Some(Effect::OpenFinder { dir });
                                    }
                                    Some(ContentsEffect::OpenSearcher { dir }) => {
                                        effect = Some(Effect::OpenSearcher { dir });
                                    }
                                    Some(ContentsEffect::OpenVim(vim_args)) => {
                                        effect = Some(Effect::OpenVim(vim_args));
                                    }
                                    Some(ContentsEffect::RunBash { dir }) => {
                                        effect = Some(Effect::RunBash { dir });
                                    }
                                    Some(ContentsEffect::Bell) => {
                                        effect = Some(Effect::Bell);
                                    }
                                    Some(ContentsEffect::Request(request)) => {
                                        effect = Some(Effect::Request(request))
                                    }
                                    None => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        effect
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
        let dir_props = DirProps::new(props.dir.clone());
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
    fn perform(&mut self, _action: Action) -> Option<Effect> {
        None
    }
}

/// What the browser is focused on.
#[derive(Default)]
enum Focus {
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
enum Action {}

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
}

use std::path::PathBuf;

use typed_builder::TypedBuilder;
use uuid::Uuid;

use file_type::FileType;
use insh_api::Request;
use insh_api::Response;
use rend::{Fabric, Size};
use term::TermEvent;
use til::Component;

use super::{Contents, ContentsEffect, ContentsEvent, ContentsProps};
use crate::components::common::{Dir, DirEvent, DirProps};
use crate::programs::VimArgs;
use crate::stateful::Stateful;

#[derive(TypedBuilder)]
pub struct Props {
    dir: PathBuf,
    size: Size,
    #[builder(default)]
    file: Option<PathBuf>,
    #[builder(default)]
    pending_request: Option<Uuid>,
}

pub struct Browser {
    state: State,
}

impl Component<Props, Event, Effect> for Browser {
    fn new(props: Props) -> Self {
        let state = State::from(props);
        Self { state }
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
                        let size = Size::new(size.rows - 1, size.columns);
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
                let fabric: Fabric = self.state.dir.render(Size::new(1, columns));
                let contents_fabric: Fabric =
                    self.state.contents.render(Size::new(rows - 1, columns));
                fabric.quilt_bottom(contents_fabric)
            }
        }
    }
}

struct State {
    dir: Dir,
    contents: Contents,
    focus: Focus,
}

impl From<Props> for State {
    fn from(props: Props) -> Self {
        let dir_props = DirProps::new(props.dir.clone());
        let dir = Dir::new(dir_props);

        let contents_size = Size::new(props.size.rows - 1, props.size.columns);
        let contents_props = ContentsProps::builder()
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

#[derive(Default)]
enum Focus {
    #[default]
    Contents,
}

pub enum Event {
    Response(Response),
    TermEvent(TermEvent),
}

enum Action {}

pub enum Effect {
    OpenFileCreator { dir: PathBuf, file_type: FileType },
    OpenFinder { dir: PathBuf },
    OpenSearcher { dir: PathBuf },
    OpenVim(VimArgs),
    RunBash { dir: PathBuf },
    Bell,
    Request(Request),
}

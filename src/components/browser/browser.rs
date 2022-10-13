use super::{Contents, ContentsEffect, ContentsEvent, ContentsProps};
use crate::component::Component;
use crate::components::common::{Directory, DirectoryEvent, DirectoryProps};
use crate::programs::VimArgs;
use crate::rendering::{Fabric, Size};
use crate::stateful::Stateful;

use std::path::PathBuf;

pub use crossterm::event::Event;

pub struct Props {
    directory: PathBuf,
    size: Size,
    file: Option<PathBuf>,
}

impl Props {
    pub fn new(directory: PathBuf, size: Size, file: Option<PathBuf>) -> Self {
        Self {
            directory,
            size,
            file,
        }
    }
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
            Event::Resize(columns, rows) => {
                let rows: usize = rows.into();
                let columns: usize = columns.into();
                let size = Size::new(rows - 1, columns);
                self.state.contents.handle(ContentsEvent::Resize { size });
            }
            _ => {
                match self.state.focus {
                    Focus::Contents => {
                        let contents_event: ContentsEvent = ContentsEvent::Crossterm { event };
                        let contents_effect: Option<ContentsEffect> =
                            self.state.contents.handle(contents_event);

                        match contents_effect {
                            Some(ContentsEffect::SetDirectory { directory }) => {
                                let directory_event = DirectoryEvent::SetDirectory { directory };
                                self.state.directory.handle(directory_event);
                                // TODO: What if the directory returns an effect here? Do we need to loop?
                            }
                            Some(ContentsEffect::PopDirectory) => {
                                let directory_event = DirectoryEvent::PopDirectory;
                                self.state.directory.handle(directory_event);
                            }
                            Some(ContentsEffect::OpenFinder { directory }) => {
                                effect = Some(Effect::OpenFinder { directory });
                            }
                            Some(ContentsEffect::OpenSearcher { directory }) => {
                                effect = Some(Effect::OpenSearcher { directory });
                            }
                            Some(ContentsEffect::OpenVim(vim_args)) => {
                                effect = Some(Effect::OpenVim(vim_args));
                            }
                            Some(ContentsEffect::RunBash { directory }) => {
                                effect = Some(Effect::RunBash { directory });
                            }
                            Some(ContentsEffect::Bell) => {
                                effect = Some(Effect::Bell);
                            }
                            None => {}
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
            1 => self.state.directory.render(size),
            rows => {
                let columns = size.columns;
                let fabric: Fabric = self.state.directory.render(Size::new(1, columns));
                let contents_fabric: Fabric =
                    self.state.contents.render(Size::new(rows - 1, columns));
                fabric.quilt_bottom(contents_fabric)
            }
        }
    }
}

struct State {
    directory: Directory,
    contents: Contents,
    focus: Focus,
}

impl From<Props> for State {
    fn from(props: Props) -> Self {
        let directory_props = DirectoryProps::new(props.directory.clone());
        let directory = Directory::new(directory_props);

        let contents_size = Size::new(props.size.rows - 1, props.size.columns);
        let contents_props = ContentsProps::new(props.directory, contents_size, props.file);
        let contents = Contents::new(contents_props);

        let focus = Focus::default();

        State {
            directory,
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

enum Focus {
    Contents,
}

impl Default for Focus {
    fn default() -> Self {
        Focus::Contents
    }
}

enum Action {}

pub enum Effect {
    OpenFinder { directory: PathBuf },
    OpenSearcher { directory: PathBuf },
    OpenVim(VimArgs),
    RunBash { directory: PathBuf },
    Bell,
}

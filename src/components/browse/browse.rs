use super::{ContentsComponent, ContentsEffect, ContentsEvent, ContentsProps};
use crate::component::Component;
use crate::components::common::{Directory, DirectoryEffect, DirectoryEvent};
use crate::rendering::{Fabric, Size};
use crate::stateful::Stateful;

use std::path::PathBuf;

pub use crossterm::event::Event;

pub struct Props {
    directory: PathBuf,
    size: Size,
}

impl Props {
    pub fn new(directory: PathBuf, size: Size) -> Self {
        Self { directory, size }
    }
}

pub struct Browse {
    state: State,
}

impl Component<Props, Event, Effect> for Browse {
    fn new(props: Props) -> Self {
        let state = State::new(props);
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
                    Focus::Directory => {
                        let directory_event: DirectoryEvent =
                            DirectoryEvent::CrosstermEvent { event };
                        let _directory_effect: Option<DirectoryEffect> =
                            self.state.directory.handle(directory_event);
                        // TODO: handle the dir effect!
                    }
                    Focus::Contents => {
                        let contents_event: ContentsEvent = ContentsEvent::CrosstermEvent { event };
                        let contents_effect: Option<ContentsEffect> =
                            self.state.contents.handle(contents_event);

                        match contents_effect {
                            Some(ContentsEffect::SetDirectory { directory }) => {
                                let directory_event = DirectoryEvent::SetDirectory { directory };
                                self.state.directory.handle(directory_event);
                                // TODO: What if the directory returns an effect here? Do we need to loop?
                            }
                            Some(ContentsEffect::OpenFinder { directory }) => {
                                effect = Some(Effect::OpenFinder { directory });
                            }
                            Some(ContentsEffect::OpenVim { file }) => {
                                effect = Some(Effect::OpenVim { file });
                            }
                            Some(ContentsEffect::RunBash { directory }) => {
                                effect = Some(Effect::RunBash { directory });
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
                let directory_fabric: Fabric = self.state.directory.render(Size::new(1, columns));
                let contents_fabric: Fabric =
                    self.state.contents.render(Size::new(rows - 1, columns));
                directory_fabric.quilt_bottom(contents_fabric)
            }
        }
    }
}

struct State {
    directory: Directory,
    contents: ContentsComponent,
    focus: Focus,
}

impl State {
    fn new(props: Props) -> Self {
        let directory = Directory::default();

        let contents_size = Size::new(props.size.rows - 1, props.size.columns);
        let contents = ContentsComponent::new(ContentsProps::new(contents_size));

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
    Directory,
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
    OpenVim { file: PathBuf },
    RunBash { directory: PathBuf },
}

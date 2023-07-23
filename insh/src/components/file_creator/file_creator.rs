mod props {
    use std::path::PathBuf;

    use typed_builder::TypedBuilder;

    #[derive(TypedBuilder)]
    pub struct Props {
        directory: PathBuf,
    }

    impl Props {
        pub fn directory(&self) -> &PathBuf {
            &self.directory
        }
    }
}
pub use props::Props;

mod file_creator {
    use super::{Action, Effect, Props, State};
    use crate::components::common::{PhraseEffect, PhraseEvent};
    use crate::Stateful;

    use rend::{Fabric, Size};
    use til::Component;

    use term::TermEvent as Event;

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

            let phrase_event = PhraseEvent::TermEvent(event);
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

mod state {
    use super::{Action, Effect, Props};
    use crate::components::common::{Directory, DirectoryProps, Phrase, PhraseEvent};
    use crate::Stateful;

    use til::Component;

    use std::fs::File;
    use std::path::PathBuf;

    pub struct State {
        directory: PathBuf,
        directory_component: Directory,
        pub phrase: Phrase,
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
                error: None,
            }
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::CreateFile { filename } => self.create_file(&filename),
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
            let mut filepath = self.directory.clone();
            filepath.push(filename);

            if filepath.exists() {
                self.error = Some(format!("The file {} already exists.", filename));
                self.phrase.handle(PhraseEvent::Focus);
                return None;
            }

            if let Err(error) = File::create(&filepath) {
                self.error = Some(error.to_string());
                self.phrase.handle(PhraseEvent::Focus);
                return None;
            }

            Some(Effect::Browse {
                directory: self.directory.clone(),
                file: Some(filepath),
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

    pub enum Effect {
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
    pub enum Action {
        CreateFile { filename: String },
        Bell,
        Quit,
    }
}
use action::Action;

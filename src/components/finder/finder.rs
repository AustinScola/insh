mod props {
    use crate::rendering::Size;
    use std::path::PathBuf;

    pub struct Props {
        pub directory: PathBuf,
        pub size: Size,
    }

    impl Props {
        pub fn new(directory: PathBuf, size: Size) -> Self {
            Props { directory, size }
        }
    }
}
pub use props::Props;

mod finder {
    use super::super::{FoundEffect, FoundEvent};
    use super::{Action, Effect, Focus, Props, State};
    use crate::components::common::{PhraseEffect, PhraseEvent};
    use crate::rendering::{Fabric, Size};
    use crate::stateful::Stateful;
    use crate::Component;

    use crossterm::event::Event as CrosstermEvent;

    pub struct Finder {
        state: State,
    }

    impl Component<Props, CrosstermEvent, Effect> for Finder {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Finder { state }
        }

        fn handle(&mut self, event: CrosstermEvent) -> Option<Effect> {
            match event {
                CrosstermEvent::Resize(columns, rows) => {
                    let rows: usize = rows.into();
                    let columns: usize = columns.into();
                    let size = Size::new(rows.saturating_sub(2), columns);
                    self.state.found.handle(FoundEvent::Resize { size });
                    None
                }
                _ => match self.state.focus() {
                    Focus::Directory => None,
                    Focus::Phrase => {
                        let mut action: Option<Action> = None;

                        let phrase_event = PhraseEvent::CrosstermEvent { event };
                        let phrase_effect = self.state.phrase.handle(phrase_event);
                        match phrase_effect {
                            Some(PhraseEffect::Enter { phrase }) => {
                                let found_event = FoundEvent::Find {
                                    phrase: phrase.clone(),
                                };
                                let found_effect = self.state.found.handle(found_event);
                                if let Some(FoundEffect::Unfocus) = found_effect {
                                    self.state.perform(Action::FocusPhrase);
                                    self.state.phrase.handle(PhraseEvent::Focus);
                                } else {
                                    action = Some(Action::Find { phrase });
                                }
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
                    Focus::Found => {
                        let found_event = FoundEvent::CrosstermEvent { event };
                        let found_effect = self.state.found.handle(found_event);
                        match found_effect {
                            Some(FoundEffect::Unfocus) => {
                                self.state.perform(Action::FocusPhrase);
                                self.state.phrase.handle(PhraseEvent::Focus);
                                None
                            }
                            Some(FoundEffect::Goto { directory }) => {
                                Some(Effect::Browse { directory })
                            }
                            Some(FoundEffect::OpenVim(vim_args)) => Some(Effect::OpenVim(vim_args)),
                            None => None,
                        }
                    }
                },
            }
        }

        fn render(&self, size: Size) -> Fabric {
            match size.rows {
                0 => Fabric::new(size),
                1 => self.state.phrase.render(size),
                2 => {
                    let columns = size.columns;
                    let phrase_fabric = self.state.phrase.render(Size::new(1, columns));
                    let directory_fabric = self.state.directory().render(Size::new(1, columns));
                    directory_fabric.quilt_bottom(phrase_fabric)
                }
                rows => {
                    let columns = size.columns;
                    let mut fabric: Fabric;

                    let directory_fabric = self.state.directory().render(Size::new(1, columns));
                    fabric = directory_fabric;

                    let phrase_fabric = self.state.phrase.render(Size::new(1, columns));
                    fabric = fabric.quilt_bottom(phrase_fabric);

                    let found_fabric = self.state.found().render(Size::new(rows - 2, columns));
                    fabric.quilt_bottom(found_fabric)
                }
            }
        }
    }
}
pub use finder::Finder;

mod state {
    use super::super::{Found, FoundProps};
    use super::{Action, Effect, Focus, Props};
    use crate::component::Component;
    use crate::components::common::{Directory, DirectoryProps, Phrase};
    use crate::rendering::Size;
    use crate::stateful::Stateful;

    pub struct State {
        directory: Directory,
        pub phrase: Phrase,
        pub found: Found,
        focus: Focus,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let directory_props = DirectoryProps::new(props.directory.clone());
            let directory = Directory::new(directory_props);

            let phrase = Phrase::default();

            let found_size = Size::new(props.size.rows.saturating_sub(2), props.size.columns);
            let found_props = FoundProps::new(props.directory, found_size);
            let found = Found::new(found_props);

            let focus = Focus::default();

            Self {
                directory,
                phrase,
                found,
                focus,
            }
        }
    }

    impl State {
        pub fn directory(&self) -> &Directory {
            &self.directory
        }

        pub fn found(&self) -> &Found {
            &self.found
        }

        pub fn focus(&self) -> &Focus {
            &self.focus
        }

        fn find(&mut self, _phrase: String) -> Option<Effect> {
            self.focus = Focus::Found;
            None
        }

        fn focus_phrase(&mut self) -> Option<Effect> {
            self.focus = Focus::Phrase;
            None
        }

        fn quit(&mut self) -> Option<Effect> {
            Some(Effect::Quit)
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Find { phrase } => self.find(phrase),
                Action::FocusPhrase => self.focus_phrase(),
                Action::Quit => self.quit(),
            }
        }
    }
}
use state::State;

mod focus {
    pub enum Focus {
        Directory,
        Phrase,
        Found,
    }

    impl Default for Focus {
        fn default() -> Self {
            Focus::Phrase
        }
    }
}
use focus::Focus;

mod action {
    pub enum Action {
        Find { phrase: String },
        FocusPhrase,
        Quit,
    }
}
use action::Action;

mod effect {
    use crate::programs::VimArgs;
    use std::path::PathBuf;

    pub enum Effect {
        Browse { directory: PathBuf },
        OpenVim(VimArgs),
        Quit,
    }
}
pub use effect::Effect;

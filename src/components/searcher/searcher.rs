mod props {
    use crate::rendering::Size;

    use std::path::PathBuf;

    pub struct Props {
        pub directory: PathBuf,
        pub size: Size,
        pub phrase: Option<String>,
    }

    impl Props {
        pub fn new(directory: PathBuf, size: Size, phrase: Option<String>) -> Self {
            Self {
                directory,
                size,
                phrase,
            }
        }
    }
}
pub use props::Props;

mod searcher {
    use super::super::{ContentsEffect, ContentsEvent};
    use super::{Action, Effect, Focus, Props, State};

    use crate::components::common::{PhraseEffect, PhraseEvent};
    use crate::rendering::{Fabric, Size};
    use crate::{Component, Stateful};

    use crossterm::event::Event as CrosstermEvent;

    pub struct Searcher {
        state: State,
    }

    impl Component<Props, CrosstermEvent, Effect> for Searcher {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Self { state }
        }

        fn handle(&mut self, event: CrosstermEvent) -> Option<Effect> {
            match event {
                CrosstermEvent::Resize(columns, rows) => {
                    let rows: usize = rows.into();
                    let columns: usize = columns.into();
                    let size = Size::new(rows.saturating_sub(2), columns);
                    self.state.contents.handle(ContentsEvent::Resize { size });
                    None
                }
                _ => match self.state.focus() {
                    Focus::Phrase => {
                        let phrase_event = PhraseEvent::CrosstermEvent { event };
                        let phrase_effect = self.state.phrase.handle(phrase_event);
                        let action: Option<Action> = match phrase_effect {
                            Some(PhraseEffect::Enter { phrase }) => {
                                let contents_event = ContentsEvent::Search { phrase };
                                let contents_effect = self.state.contents.handle(contents_event);
                                if let Some(ContentsEffect::Unfocus) = contents_effect {
                                    self.state.phrase.handle(PhraseEvent::Focus);
                                    Some(Action::FocusPhrase)
                                } else {
                                    Some(Action::FocusContents)
                                }
                            }
                            Some(PhraseEffect::Quit) => Some(Action::Quit),
                            None => None,
                        };

                        if let Some(action) = action {
                            self.state.perform(action)
                        } else {
                            None
                        }
                    }
                    Focus::Contents => {
                        let contents_event = ContentsEvent::Crossterm { event };
                        let contents_effect = self.state.contents.handle(contents_event);
                        let action: Option<Action> = match contents_effect {
                            Some(ContentsEffect::Unfocus) => {
                                self.state.phrase.handle(PhraseEvent::Focus);
                                Some(Action::FocusPhrase)
                            }
                            Some(ContentsEffect::OpenVim(vim_args)) => {
                                Some(Action::OpenVim(vim_args))
                            }
                            None => None,
                        };

                        if let Some(action) = action {
                            self.state.perform(action)
                        } else {
                            None
                        }
                    }
                },
            }
        }

        fn render(&self, size: Size) -> Fabric {
            match size.rows {
                0 => Fabric::new(size),
                1 => self.state.phrase().render(size),
                2 => {
                    let columns = size.columns;
                    let phrase_fabric = self.state.phrase().render(Size::new(1, columns));
                    let directory_fabric = self.state.directory().render(Size::new(1, columns));
                    directory_fabric.quilt_bottom(phrase_fabric)
                }
                rows => {
                    let columns = size.columns;

                    let directory_fabric = self.state.directory().render(Size::new(1, columns));
                    let mut fabric: Fabric = directory_fabric;

                    let phrase_fabric = self.state.phrase().render(Size::new(1, columns));
                    fabric = fabric.quilt_bottom(phrase_fabric);

                    let contents_fabric =
                        self.state.contents().render(Size::new(rows - 2, columns));
                    fabric.quilt_bottom(contents_fabric)
                }
            }
        }
    }
}
pub use searcher::Searcher;

mod effect {
    use crate::programs::VimArgs;

    pub enum Effect {
        Quit,
        OpenVim(VimArgs),
    }
}
pub use effect::Effect;

mod state {
    use super::super::{Contents, ContentsEffect, ContentsEvent, ContentsProps};
    use super::{Action, Effect, Props};
    use crate::components::common::{Directory, DirectoryProps, Phrase, PhraseEvent};
    use crate::programs::VimArgs;
    use crate::rendering::Size;
    use crate::{Component, Stateful};

    pub struct State {
        focus: Focus,
        directory: Directory,
        pub phrase: Phrase,
        pub contents: Contents,
    }

    impl State {
        pub fn focus(&self) -> &Focus {
            &self.focus
        }
        pub fn directory(&self) -> &Directory {
            &self.directory
        }

        pub fn phrase(&self) -> &Phrase {
            &self.phrase
        }

        pub fn contents(&self) -> &Contents {
            &self.contents
        }

        fn focus_phrase(&mut self) -> Option<Effect> {
            self.focus = Focus::Phrase;
            None
        }

        fn focus_contents(&mut self) -> Option<Effect> {
            self.focus = Focus::Contents;
            None
        }

        fn open_vim(&mut self, vim_args: VimArgs) -> Option<Effect> {
            Some(Effect::OpenVim(vim_args))
        }

        fn quit(&mut self) -> Option<Effect> {
            Some(Effect::Quit)
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::FocusPhrase => self.focus_phrase(),
                Action::FocusContents => self.focus_contents(),
                Action::OpenVim(vim_args) => self.open_vim(vim_args),
                Action::Quit => self.quit(),
            }
        }
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let focus = Focus::default();

            let directory_props = DirectoryProps::new(props.directory.clone());
            let directory = Directory::new(directory_props);

            let phrase = Phrase::default();

            let contents_size = Size::new(props.size.rows.saturating_sub(2), props.size.columns);
            let contents_props = ContentsProps::new(props.directory, contents_size);
            let contents = Contents::new(contents_props);

            let mut state = Self {
                focus,
                directory,
                phrase,
                contents,
            };

            if let Some(phrase) = props.phrase {
                state.phrase.handle(PhraseEvent::Set {
                    phrase: phrase.clone(),
                });

                let contents_event = ContentsEvent::Search { phrase };
                let contents_effect = state.contents.handle(contents_event);
                if let Some(ContentsEffect::Unfocus) = contents_effect {
                    state.phrase.handle(PhraseEvent::Focus);
                    state.focus = Focus::Phrase;
                } else {
                    state.phrase.handle(PhraseEvent::Unfocus);
                    state.focus = Focus::Contents;
                }
            }

            state
        }
    }

    pub enum Focus {
        Phrase,
        Contents,
    }

    impl Default for Focus {
        fn default() -> Self {
            Self::Phrase
        }
    }
}
use state::{Focus, State};

mod action {
    use crate::programs::VimArgs;

    pub enum Action {
        FocusPhrase,
        FocusContents,
        OpenVim(VimArgs),
        Quit,
    }
}
pub use action::Action;

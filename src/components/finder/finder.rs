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
            Props {
                directory,
                size,
                phrase,
            }
        }
    }
}
pub use props::Props;

mod finder {
    use super::super::{ContentsEffect, ContentsEvent};
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
                    self.state.contents.handle(ContentsEvent::Resize { size });
                    None
                }
                _ => match self.state.focus() {
                    Focus::Phrase => {
                        let mut action: Option<Action> = None;

                        let phrase_event = PhraseEvent::CrosstermEvent { event };
                        let phrase_effect = self.state.phrase.handle(phrase_event);
                        match phrase_effect {
                            Some(PhraseEffect::Enter { phrase }) => {
                                let contents_event = ContentsEvent::Find { phrase };
                                let contents_effect = self.state.contents.handle(contents_event);
                                if let Some(ContentsEffect::Unfocus) = contents_effect {
                                    self.state.perform(Action::FocusPhrase);
                                    self.state.phrase.handle(PhraseEvent::Focus);
                                } else {
                                    action = Some(Action::FocusContents);
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
                    Focus::Contents => {
                        let contents_event = ContentsEvent::Crossterm { event };
                        let contents_effect = self.state.contents.handle(contents_event);
                        match contents_effect {
                            Some(ContentsEffect::Unfocus) => {
                                self.state.perform(Action::FocusPhrase);
                                self.state.phrase.handle(PhraseEvent::Focus);
                                None
                            }
                            Some(ContentsEffect::Goto { directory }) => {
                                Some(Effect::Browse { directory })
                            }
                            Some(ContentsEffect::OpenVim(vim_args)) => {
                                Some(Effect::OpenVim(vim_args))
                            }
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
                    let directory_fabric = self.state.directory().render(Size::new(1, columns));
                    let mut fabric: Fabric = directory_fabric;

                    let phrase_fabric = self.state.phrase.render(Size::new(1, columns));
                    fabric = fabric.quilt_bottom(phrase_fabric);

                    let contents_fabric =
                        self.state.contents().render(Size::new(rows - 2, columns));
                    fabric.quilt_bottom(contents_fabric)
                }
            }
        }
    }
}
pub use finder::Finder;

mod state {
    use super::super::{Contents, ContentsEffect, ContentsEvent, ContentsProps};
    use super::{Action, Effect, Focus, Props};
    use crate::component::Component;
    use crate::components::common::{Directory, DirectoryProps, Phrase, PhraseEvent};
    use crate::rendering::Size;
    use crate::stateful::Stateful;

    pub struct State {
        directory: Directory,
        pub phrase: Phrase,
        pub contents: Contents,
        focus: Focus,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let directory_props = DirectoryProps::new(props.directory.clone());
            let directory = Directory::new(directory_props);

            let phrase = Phrase::default();

            let contents_size = Size::new(props.size.rows.saturating_sub(2), props.size.columns);
            let contents_props = ContentsProps::new(props.directory, contents_size);
            let contents = Contents::new(contents_props);

            let focus = Focus::default();

            let mut state = Self {
                directory,
                phrase,
                contents,
                focus,
            };

            if let Some(phrase) = props.phrase {
                state.phrase.handle(PhraseEvent::Set {
                    phrase: phrase.clone(),
                });

                let contents_event = ContentsEvent::Find { phrase };
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

    impl State {
        pub fn directory(&self) -> &Directory {
            &self.directory
        }

        pub fn contents(&self) -> &Contents {
            &self.contents
        }

        pub fn focus(&self) -> &Focus {
            &self.focus
        }

        fn focus_contents(&mut self) -> Option<Effect> {
            self.focus = Focus::Contents;
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
                Action::FocusContents => self.focus_contents(),
                Action::FocusPhrase => self.focus_phrase(),
                Action::Quit => self.quit(),
            }
        }
    }
}
use state::State;

mod focus {
    pub enum Focus {
        Phrase,
        Contents,
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
        FocusContents,
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

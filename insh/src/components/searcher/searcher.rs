mod props {
    use crate::config::Config;

    use rend::Size;

    use std::path::PathBuf;

    pub struct Props {
        pub config: Config,
        pub dir: PathBuf,
        pub size: Size,
        pub phrase: Option<String>,
    }

    impl Props {
        pub fn new(config: Config, dir: PathBuf, size: Size, phrase: Option<String>) -> Self {
            Self {
                config,
                dir,
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
    use crate::Stateful;

    use rend::{Fabric, Size};
    use term::TermEvent;
    use til::Component;

    pub struct Searcher {
        state: State,
    }

    impl Component<Props, TermEvent, Effect> for Searcher {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Self { state }
        }

        fn handle(&mut self, event: TermEvent) -> Option<Effect> {
            match event {
                TermEvent::Resize(size) => {
                    let contents_size = Size::new(size.rows.saturating_sub(2), size.columns);
                    self.state
                        .contents
                        .handle(ContentsEvent::TermEvent(TermEvent::Resize(contents_size)));
                    None
                }
                _ => match self.state.focus() {
                    Focus::Phrase => {
                        let phrase_event = PhraseEvent::TermEvent(event);
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
                            Some(PhraseEffect::Bell) => {
                                return Some(Effect::Bell);
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
                        let contents_event = ContentsEvent::TermEvent(event);
                        let contents_effect = self.state.contents.handle(contents_event);
                        let action: Option<Action> = match contents_effect {
                            Some(ContentsEffect::Unfocus) => {
                                self.state.phrase.handle(PhraseEvent::Focus);
                                Some(Action::FocusPhrase)
                            }
                            Some(ContentsEffect::Goto { dir, file }) => {
                                Some(Action::Goto { dir, file })
                            }
                            Some(ContentsEffect::OpenVim(vim_args)) => {
                                Some(Action::OpenVim(vim_args))
                            }
                            Some(ContentsEffect::Bell) => {
                                return Some(Effect::Bell);
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
                    let dir_fabric = self.state.dir().render(Size::new(1, columns));
                    dir_fabric.quilt_bottom(phrase_fabric)
                }
                rows => {
                    let columns = size.columns;

                    let dir_fabric = self.state.dir().render(Size::new(1, columns));
                    let mut fabric: Fabric = dir_fabric;

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

    use std::path::PathBuf;

    pub enum Effect {
        Goto { dir: PathBuf, file: Option<PathBuf> },
        OpenVim(VimArgs),
        Bell,
        Quit,
    }
}
pub use effect::Effect;

mod state {
    use super::super::{Contents, ContentsEffect, ContentsEvent, ContentsProps};
    use super::{Action, Effect, Props};
    use crate::auto_completer::AutoCompleter;
    use crate::auto_completers::SearchCompleter;
    use crate::components::common::{Dir, DirProps, Phrase, PhraseEvent, PhraseProps};
    use crate::programs::VimArgs;
    use crate::Stateful;

    use rend::Size;
    use til::Component;

    use std::path::PathBuf;

    pub struct State {
        focus: Focus,
        dir: Dir,
        pub phrase: Phrase,
        pub contents: Contents,
    }

    impl State {
        pub fn focus(&self) -> &Focus {
            &self.focus
        }
        pub fn dir(&self) -> &Dir {
            &self.dir
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

        fn goto(&mut self, dir: PathBuf, file: Option<PathBuf>) -> Option<Effect> {
            Some(Effect::Goto { dir, file })
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
                Action::Goto { dir, file } => self.goto(dir, file),
                Action::OpenVim(vim_args) => self.open_vim(vim_args),
                Action::Quit => self.quit(),
            }
        }
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let focus = Focus::default();

            let dir_props = DirProps::new(props.dir.clone());
            let dir = Dir::new(dir_props);

            let search_completer: Option<Box<dyn AutoCompleter<String, String>>> =
                Some(Box::new(SearchCompleter::new()));
            let phrase_props = PhraseProps::builder()
                .auto_completer(search_completer)
                .build();
            let phrase = Phrase::new(phrase_props);

            let contents_size = Size::new(props.size.rows.saturating_sub(2), props.size.columns);
            let contents_props = ContentsProps::new(props.config, props.dir, contents_size);
            let contents = Contents::new(contents_props);

            let mut state = Self {
                focus,
                dir,
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

    use std::path::PathBuf;

    pub enum Action {
        FocusPhrase,
        FocusContents,
        Goto { dir: PathBuf, file: Option<PathBuf> },
        OpenVim(VimArgs),
        Quit,
    }
}
pub use action::Action;

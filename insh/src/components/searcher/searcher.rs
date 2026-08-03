mod props {
    use crate::config::Config;

    use rend::Size;
    use uuid::Uuid;

    use std::path::PathBuf;

    pub struct Props {
        pub config: Config,
        pub dir: PathBuf,
        pub size: Size,
        pub phrase: Option<String>,
        pub pending_request: Option<Uuid>,
    }

    impl Props {
        pub fn new(
            config: Config,
            dir: PathBuf,
            size: Size,
            phrase: Option<String>,
            pending_request: Option<Uuid>,
        ) -> Self {
            Self {
                config,
                dir,
                size,
                phrase,
                pending_request,
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

    use insh_api::{
        Request, RequestParams, Response, ResponseParams, SuggestSearchPhraseRequestParams,
    };
    use rend::{Fabric, Size};
    use term::TermEvent;
    use til::{Component, Event};

    pub struct Searcher {
        state: State,
    }

    impl Component<Props, Event<Response>, Effect> for Searcher {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Self { state }
        }

        fn handle(&mut self, event: Event<Response>) -> Option<Effect> {
            match event {
                Event::TermEvent(TermEvent::Resize(size)) => {
                    let contents_size = Size::new(size.rows.saturating_sub(2), size.columns);
                    self.state
                        .contents
                        .handle(ContentsEvent::TermEvent(TermEvent::Resize(contents_size)));
                    None
                }
                _ => match self.state.focus() {
                    Focus::Phrase => {
                        let phrase_event = match event {
                            Event::TermEvent(term_event) => PhraseEvent::TermEvent(term_event),
                            Event::Response(response) => {
                                let suggestion = match response.params() {
                                    ResponseParams::SuggestSearchPhrase(params) => {
                                        params.suggestion().clone()
                                    }
                                    _ => {
                                        #[cfg(feature = "logging")]
                                        log::error!("Unexpected response parameters.");
                                        return None;
                                    }
                                };
                                PhraseEvent::Completion {
                                    uuid: *response.uuid(),
                                    completion: suggestion,
                                }
                            }
                        };
                        let phrase_effect = self.state.phrase.handle(phrase_event);
                        match phrase_effect {
                            Some(PhraseEffect::Enter { phrase }) => {
                                self.state.perform(Action::FocusContents);
                                let contents_effect =
                                    self.state.contents.handle(ContentsEvent::Search { phrase });
                                match contents_effect {
                                    Some(ContentsEffect::Request(request)) => {
                                        Some(Effect::Request(request))
                                    }
                                    _ => None,
                                }
                            }
                            Some(PhraseEffect::Bell) => Some(Effect::Bell),
                            Some(PhraseEffect::RequestCompletion { uuid, partial }) => {
                                let params = RequestParams::SuggestSearchPhrase(
                                    SuggestSearchPhraseRequestParams::builder()
                                        .partial(partial)
                                        .build(),
                                );
                                let request = Request::builder().uuid(uuid).params(params).build();
                                Some(Effect::Request(request))
                            }
                            Some(PhraseEffect::Quit) => self.state.perform(Action::Quit),
                            None => None,
                        }
                    }
                    Focus::Contents => {
                        let contents_event = match event {
                            Event::Response(response) => ContentsEvent::Response(response),
                            Event::TermEvent(term_event) => ContentsEvent::TermEvent(term_event),
                        };
                        let contents_effect = self.state.contents.handle(contents_event);
                        let action: Option<Action> = match contents_effect {
                            Some(ContentsEffect::Unfocus) => {
                                self.state.phrase.handle(PhraseEvent::Focus);
                                Some(Action::FocusPhrase)
                            }
                            Some(ContentsEffect::Request(request)) => {
                                return Some(Effect::Request(request));
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

    use insh_api::Request;

    use std::path::PathBuf;

    pub enum Effect {
        Goto { dir: PathBuf, file: Option<PathBuf> },
        OpenVim(VimArgs),
        Bell,
        Request(Request),
        Quit,
    }
}
pub use effect::Effect;

mod state {
    use super::super::{Contents, ContentsProps};
    use super::{Action, Effect, Props};
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
            let dir_props = DirProps::new(props.dir.clone());
            let dir = Dir::new(dir_props);

            let phrase_props = PhraseProps::builder()
                .completable(true)
                .value(props.phrase.clone())
                .build();
            let mut phrase = Phrase::new(phrase_props);

            let contents_size = Size::new(props.size.rows.saturating_sub(2), props.size.columns);
            let contents_props = ContentsProps::new(
                props.config,
                props.dir,
                contents_size,
                props.phrase.clone(),
                props.pending_request,
            );
            let contents = Contents::new(contents_props);

            let focus = if props.phrase.is_some() {
                phrase.handle(PhraseEvent::Unfocus);
                Focus::Contents
            } else {
                Focus::default()
            };

            Self {
                focus,
                dir,
                phrase,
                contents,
            }
        }
    }

    #[derive(Default)]
    pub enum Focus {
        #[default]
        Phrase,
        Contents,
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

//! Contains the [`Searcher`] component.

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use crate::config::Config;

    use rend::Size;

    use uuid::Uuid;

    /// The properties of the searcher.
    pub struct Props {
        /// The configuration.
        pub config: Config,
        /// The directory to search in.
        pub dir: PathBuf,
        /// The size of the searcher.
        pub size: Size,
        /// The phrase to start with.
        pub phrase: Option<String>,
        /// The pending request for the hits.
        pub pending_request: Option<Uuid>,
    }

    impl Props {
        /// Return new properties.
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

/// Contains the [`Searcher`] component.
mod searcher {
    use super::super::{ContentsEffect, ContentsEvent};
    use super::{Action, Effect, Focus, Props, State};
    use crate::components::common::{Footer, FooterProps, PhraseEffect, PhraseEvent};
    use crate::Stateful;

    use insh_api::{
        Request, RequestParams, Response, ResponseParams, SuggestSearchPhraseRequestParams,
    };
    use rend::{Fabric, Size};
    use term::TermEvent;
    use til::{Component, Event};

    /// A file contents searcher.
    pub struct Searcher {
        /// The state of the searcher.
        state: State,
    }

    impl Component<Props, Event<Response>, Effect> for Searcher {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Self { state }
        }

        fn name(&self) -> String {
            String::from("searcher")
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

                    if rows > 3 {
                        let contents_fabric =
                            self.state.contents().render(Size::new(rows - 3, columns));
                        fabric = fabric.quilt_bottom(contents_fabric);
                    }

                    let footer_props = FooterProps::builder()
                        .name(self.name())
                        .info(self.state.contents())
                        .build();
                    let footer_fabric = Footer::new(footer_props).render(Size::new(1, columns));

                    fabric.quilt_bottom(footer_fabric)
                }
            }
        }

        fn handle(&mut self, event: Event<Response>) -> Option<Effect> {
            match event {
                Event::TermEvent(TermEvent::Resize(size)) => {
                    let contents_size = Size::new(size.rows.saturating_sub(3), size.columns);
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
    }
}
pub use searcher::Searcher;

/// Contains the [`Effect`] enum.
mod effect {
    use std::path::PathBuf;

    use crate::programs::VimArgs;

    use insh_api::Request;

    /// A searcher effect.
    pub enum Effect {
        /// Browse a directory.
        Goto {
            /// The directory to browse.
            dir: PathBuf,
            /// The file to select.
            file: Option<PathBuf>,
        },
        /// Edit a file.
        OpenVim(VimArgs),
        /// Ring the bell.
        Bell,
        /// Send a request.
        Request(Request),
        /// Quit.
        Quit,
    }
}
pub use effect::Effect;

/// Contains the [`State`] struct.
mod state {
    use std::path::PathBuf;

    use super::super::{Contents, ContentsProps};
    use super::{Action, Effect, Props};
    use crate::components::common::{Dir, DirProps, Phrase, PhraseEvent, PhraseProps};
    use crate::programs::VimArgs;
    use crate::Stateful;

    use rend::Size;
    use til::Component;

    /// The state of the searcher.
    pub struct State {
        /// What is focused on.
        focus: Focus,
        /// The directory bar.
        dir: Dir,
        /// The phrase typed in.
        pub phrase: Phrase,
        /// The hits found.
        pub contents: Contents,
    }

    impl State {
        /// Return what is focused on.
        pub fn focus(&self) -> &Focus {
            &self.focus
        }
        /// Return the directory bar.
        pub fn dir(&self) -> &Dir {
            &self.dir
        }

        /// Return the phrase typed in.
        pub fn phrase(&self) -> &Phrase {
            &self.phrase
        }

        /// Return the hits found.
        pub fn contents(&self) -> &Contents {
            &self.contents
        }

        /// Send the events to the phrase.
        fn focus_phrase(&mut self) -> Option<Effect> {
            self.focus = Focus::Phrase;
            None
        }

        /// Send the events to the hits.
        fn focus_contents(&mut self) -> Option<Effect> {
            self.focus = Focus::Contents;
            None
        }

        /// Browse a directory.
        fn goto(&mut self, dir: PathBuf, file: Option<PathBuf>) -> Option<Effect> {
            Some(Effect::Goto { dir, file })
        }

        /// Edit a file.
        fn open_vim(&mut self, vim_args: VimArgs) -> Option<Effect> {
            Some(Effect::OpenVim(vim_args))
        }

        /// Quit.
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

            let contents_size = Size::new(props.size.rows.saturating_sub(3), props.size.columns);
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

    /// What the searcher is focused on.
    #[derive(Default)]
    pub enum Focus {
        /// The phrase typed in.
        #[default]
        Phrase,
        /// The hits found.
        Contents,
    }
}
use state::{Focus, State};

/// Contains the [`Action`] enum.
mod action {
    use std::path::PathBuf;

    use crate::programs::VimArgs;

    /// A searcher action.
    pub enum Action {
        /// Send the events to the phrase.
        FocusPhrase,
        /// Send the events to the hits.
        FocusContents,
        /// Browse a directory.
        Goto {
            /// The directory to browse.
            dir: PathBuf,
            /// The file to select.
            file: Option<PathBuf>,
        },
        /// Edit a file.
        OpenVim(VimArgs),
        /// Quit.
        Quit,
    }
}
pub use action::Action;

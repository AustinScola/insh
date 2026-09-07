//! Contains the [`Finder`] component.

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use rend::Size;

    use typed_builder::TypedBuilder;

    /// The properties of the finder.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// The directory to look in.
        #[builder(setter(into))]
        pub dir: PathBuf,
        /// The size of the finder.
        pub size: Size,
        /// The pattern to start with.
        #[builder(setter(into))]
        pub phrase: Option<String>,
    }
}
pub use props::Props;

/// Contains the [`Finder`] component.
mod finder {
    use super::super::{ContentsEffect, ContentsEvent};
    use super::{Action, Effect, Focus, Props, State};
    use crate::components::common::{Footer, FooterProps, PhraseEffect, PhraseEvent};
    use crate::stateful::Stateful;

    use insh_api::{
        Request, RequestParams, Response, ResponseParams, SuggestFindPatternRequestParams,
    };
    use rend::{Fabric, Size};
    use term::TermEvent;
    use til::{Component, Event};

    /// A file finder.
    pub struct Finder {
        /// The state of the finder.
        state: State,
    }

    impl Component<Props, Event<Response>, Effect> for Finder {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Finder { state }
        }

        fn name(&self) -> String {
            String::from("finder")
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
                                    ResponseParams::SuggestFindPattern(params) => {
                                        params.suggestion().clone()
                                    }
                                    // Responses for a find which is still streaming arrive here
                                    // when the contents are unfocused before the find is done.
                                    ResponseParams::FindFiles(_) => {
                                        #[cfg(feature = "logging")]
                                        log::debug!(
                                            "Ignoring a response for a find which is not focused."
                                        );
                                        return None;
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

                        let mut action: Option<Action> = None;

                        let phrase_effect = self.state.phrase.handle(phrase_event);
                        match phrase_effect {
                            Some(PhraseEffect::Enter { phrase }) => {
                                self.state.perform(Action::FocusContents);
                                let contents_effect =
                                    self.state.contents.handle(ContentsEvent::Find { phrase });
                                if let Some(ContentsEffect::Request(request)) = contents_effect {
                                    return Some(Effect::Request(request));
                                }
                            }
                            Some(PhraseEffect::Bell) => {
                                return Some(Effect::Bell);
                            }
                            Some(PhraseEffect::RequestCompletion { uuid, partial }) => {
                                let params = RequestParams::SuggestFindPattern(
                                    SuggestFindPatternRequestParams::builder()
                                        .partial(partial)
                                        .build(),
                                );
                                let request = Request::builder().uuid(uuid).params(params).build();
                                return Some(Effect::Request(request));
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
                        let contents_event = match event {
                            Event::Response(response) => ContentsEvent::Response(response),
                            Event::TermEvent(term_event) => ContentsEvent::TermEvent(term_event),
                        };
                        let contents_effect = self.state.contents.handle(contents_event);
                        match contents_effect {
                            Some(ContentsEffect::Unfocus) => {
                                self.state.perform(Action::FocusPhrase);
                                self.state.phrase.handle(PhraseEvent::Focus);
                                None
                            }
                            Some(ContentsEffect::Request(request)) => {
                                Some(Effect::Request(request))
                            }
                            Some(ContentsEffect::Goto { dir, file }) => {
                                Some(Effect::Browse { dir, file })
                            }
                            Some(ContentsEffect::OpenVim(vim_args)) => {
                                Some(Effect::OpenVim(vim_args))
                            }
                            Some(ContentsEffect::Bell) => Some(Effect::Bell),
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
                    let dir_fabric = self.state.dir().render(Size::new(1, columns));
                    dir_fabric.quilt_bottom(phrase_fabric)
                }
                rows => {
                    let columns = size.columns;
                    let dir_fabric = self.state.dir().render(Size::new(1, columns));
                    let mut fabric: Fabric = dir_fabric;

                    let phrase_fabric = self.state.phrase.render(Size::new(1, columns));
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
    }
}
pub use finder::Finder;

/// Contains the [`State`] struct.
mod state {
    use super::super::{Contents, ContentsProps};
    use super::{Action, Effect, Focus, Props};
    use crate::components::common::{Dir, DirProps, Phrase, PhraseProps};
    use crate::stateful::Stateful;

    use rend::Size;
    use til::Component;

    /// The state of the finder.
    pub struct State {
        /// The directory bar.
        dir: Dir,
        /// The pattern typed in.
        pub phrase: Phrase,
        /// The files found.
        pub contents: Contents,
        /// What is focused on.
        focus: Focus,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let dir_props = DirProps::new(props.dir.clone());
            let dir = Dir::new(dir_props);

            let phrase = Phrase::new(
                PhraseProps::builder()
                    .completable(true)
                    .value(props.phrase)
                    .build(),
            );

            let contents_size = Size::new(props.size.rows.saturating_sub(3), props.size.columns);
            let contents_props = ContentsProps::builder()
                .dir(props.dir)
                .size(contents_size)
                .build();
            let contents = Contents::new(contents_props);

            let focus = Focus::default();

            Self {
                dir,
                phrase,
                contents,
                focus,
            }
        }
    }

    impl State {
        /// Return the directory bar.
        pub fn dir(&self) -> &Dir {
            &self.dir
        }

        /// Return the files found.
        pub fn contents(&self) -> &Contents {
            &self.contents
        }

        /// Return what is focused on.
        pub fn focus(&self) -> &Focus {
            &self.focus
        }

        /// Send the events to the files.
        fn focus_contents(&mut self) -> Option<Effect> {
            self.focus = Focus::Contents;
            None
        }

        /// Send the events to the pattern.
        fn focus_phrase(&mut self) -> Option<Effect> {
            self.focus = Focus::Phrase;
            None
        }

        /// Quit.
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

/// Contains the [`Focus`] enum.
mod focus {
    /// What the finder is focused on.
    #[derive(Default)]
    pub enum Focus {
        /// The pattern typed in.
        #[default]
        Phrase,
        /// The files found.
        Contents,
    }
}
use focus::Focus;

/// Contains the [`Action`] enum.
mod action {
    /// A finder action.
    pub enum Action {
        /// Send the events to the files.
        FocusContents,
        /// Send the events to the pattern.
        FocusPhrase,
        /// Quit.
        Quit,
    }
}
use action::Action;

/// Contains the [`Effect`] enum.
mod effect {
    use std::path::PathBuf;

    use crate::programs::VimArgs;

    use insh_api::Request;

    /// A finder effect.
    pub enum Effect {
        /// Send a request.
        Request(Request),
        /// Browse a directory.
        Browse {
            /// The directory to browse.
            dir: PathBuf,
            /// The file to select.
            file: Option<PathBuf>,
        },
        /// Edit a file.
        OpenVim(VimArgs),
        /// Ring the bell.
        Bell,
        /// Quit.
        Quit,
    }
}
pub use effect::Effect;

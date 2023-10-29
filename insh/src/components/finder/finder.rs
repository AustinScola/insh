mod props {
    use rend::Size;

    use std::path::PathBuf;

    use typed_builder::TypedBuilder;

    #[derive(TypedBuilder)]
    pub struct Props {
        #[builder(setter(into))]
        pub dir: PathBuf,
        pub size: Size,
        #[builder(setter(into))]
        pub phrase: Option<String>,
    }
}
pub use props::Props;

mod finder {
    use super::super::{ContentsEffect, ContentsEvent};
    use super::{Action, Effect, Focus, Props, State};
    use crate::components::common::{PhraseEffect, PhraseEvent};
    use crate::stateful::Stateful;

    use insh_api::Response;
    use rend::{Fabric, Size};
    use til::{Component, Event};

    use term::TermEvent;

    pub struct Finder {
        state: State,
    }

    impl Component<Props, Event<Response>, Effect> for Finder {
        fn new(props: Props) -> Self {
            let state = State::from(props);
            Finder { state }
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
                        let event = match event {
                            Event::TermEvent(event) => event,
                            Event::Response(_) => {
                                #[cfg(feature = "logging")]
                                log::warn!("Phrase doesn't handle responses yet.");
                                return None;
                            }
                        };

                        let mut action: Option<Action> = None;

                        let phrase_event = PhraseEvent::TermEvent(event);
                        let phrase_effect = self.state.phrase.handle(phrase_event);
                        match phrase_effect {
                            Some(PhraseEffect::Enter { phrase }) => {
                                self.state.perform(Action::FocusContents);
                                let contents_effect =
                                    self.state.contents.handle(ContentsEvent::Find { phrase });
                                match contents_effect {
                                    Some(ContentsEffect::SendFindFilesRequest {
                                        uuid,
                                        dir,
                                        pattern,
                                    }) => {
                                        return Some(Effect::SendFindFilesRequest {
                                            uuid,
                                            dir,
                                            pattern,
                                        })
                                    }
                                    _ => {}
                                }
                            }
                            Some(PhraseEffect::Bell) => {
                                return Some(Effect::Bell);
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
                            Some(ContentsEffect::SendFindFilesRequest { uuid, dir, pattern }) => {
                                Some(Effect::SendFindFilesRequest { uuid, dir, pattern })
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
    use super::super::{Contents, ContentsProps};
    use super::{Action, Effect, Focus, Props};
    use crate::components::common::{Dir, DirProps, Phrase, PhraseProps};
    use crate::stateful::Stateful;

    use rend::Size;
    use til::Component;

    pub struct State {
        dir: Dir,
        pub phrase: Phrase,
        pub contents: Contents,
        focus: Focus,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let dir_props = DirProps::new(props.dir.clone());
            let dir = Dir::new(dir_props);

            let phrase = Phrase::new(PhraseProps::builder().value(props.phrase).build());

            let contents_size = Size::new(props.size.rows.saturating_sub(2), props.size.columns);
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
        pub fn dir(&self) -> &Dir {
            &self.dir
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
    #[derive(Default)]
    pub enum Focus {
        #[default]
        Phrase,
        Contents,
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

    use uuid::Uuid;

    pub enum Effect {
        SendFindFilesRequest {
            uuid: Uuid,
            dir: PathBuf,
            pattern: String,
        },
        Browse {
            dir: PathBuf,
            file: Option<PathBuf>,
        },
        OpenVim(VimArgs),
        Bell,
        Quit,
    }
}
pub use effect::Effect;

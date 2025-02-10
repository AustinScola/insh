mod props {
    use typed_builder::TypedBuilder;

    #[derive(TypedBuilder)]
    pub struct Props {}
}
pub use props::Props;

mod ai {
    use super::{Action, Effect, Event, Props, State};
    use crate::Stateful;

    use rend::{Fabric, Size};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::Component;

    pub struct AI {
        state: State,
    }

    impl Component<Props, Event, Effect> for AI {
        fn new(props: Props) -> Self {
            Self {
                state: State::from(props),
            }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Option<Action> = match event {
                Event::Response(response) => Some(Action::HandleResponse(response)),
                Event::TermEvent(TermEvent::Resize(size)) => Some(Action::Resize { size }),
                Event::TermEvent(TermEvent::KeyEvent(key_event)) => match key_event {
                    KeyEvent {
                        key: Key::Char('q'),
                        mods: KeyMods::CONTROL,
                        ..
                    } => Some(Action::Quit),
                    KeyEvent {
                        key: Key::Char(char),
                        mods: KeyMods::NONE,
                        ..
                    } => Some(Action::Input(char)),
                    KeyEvent {
                        key: Key::CarriageReturn,
                        mods: KeyMods::NONE,
                        ..
                    } => Some(Action::Enter),
                    _ => Some(Action::Bell),
                },
            };

            if let Some(action) = action {
                self.state.perform(action)
            } else {
                Some(Effect::Bell)
            }
        }

        fn render(&self, size: Size) -> Fabric {
            Fabric::center(self.state.value(), size)
        }
    }
}
pub use ai::AI;

mod event {
    use insh_api::Response;
    use term::TermEvent;

    pub enum Event {
        Response(Response),
        TermEvent(TermEvent),
    }
}
pub use event::Event;

mod state {
    use uuid::Uuid;

    use insh_api::{AIChatRequestParams, Request, RequestParams, Response, ResponseParams, AIChatResponseParams};
    use rend::Size;

    use super::{Action, Effect, Props};
    use crate::Stateful;

    pub struct State {
        value: String,
        pending_request: Option<Uuid>,
    }

    impl State {
        pub fn value(&self) -> &str {
            return &self.value;
        }

        fn input(&mut self, input: char) -> Option<Effect> {
            self.value.push(input);
            None
        }

        fn enter(&mut self) -> Option<Effect> {
            let request = Request::builder()
                .params(RequestParams::AIChat(
                    AIChatRequestParams::builder()
                        .input(self.value.clone())
                        .build(),
                ))
                .build();

            Some(Effect::Request(request))
        }

        fn handle_response(&mut self, response: Response) -> Option<Effect> {
            #[cfg(feature = "logging")]
            log::debug!("Handling response...");

            let pending_request: Uuid = match self.pending_request {
                Some(pending_request) => pending_request,
                None => {
                    #[cfg(feature = "logging")]
                    log::debug!("There is no pending request.");
                    return None;
                }
            };

            if response.uuid() != &pending_request {
                #[cfg(feature = "logging")]
                log::debug!("The response is not for the pending request.");
                return None;
            }

            let params: &AIChatResponseParams = match response.params() {
                ResponseParams::AIChat(params) => params,
                _ => {
                    #[cfg(feature = "logging")]
                    log::error!("Unexpected response parameters.");
                    return None;
                }
            };

            self.value.extend(params.text());

            None
        }

        fn resize(&mut self, _size: Size) -> Option<Effect> {
            None
        }

        fn bell(&mut self) -> Option<Effect> {
            Some(Effect::Bell)
        }

        fn quit(&mut self) -> Option<Effect> {
            Some(Effect::Quit)
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Input(char) => self.input(char),
                Action::Enter => self.enter(),
                Action::HandleResponse(response) => self.handle_response(response),
                Action::Resize { size } => self.resize(size),
                Action::Bell => self.bell(),
                Action::Quit => self.quit(),
            }
        }
    }

    impl From<Props> for State {
        fn from(_props: Props) -> Self {
            Self {
                value: String::new(),
                pending_request: None,
            }
        }
    }
}
use state::State;

mod action {
    use insh_api::Response;
    use rend::Size;

    pub enum Action {
        Input(char),
        Enter,
        Resize { size: Size },
        HandleResponse(Response),
        Bell,
        Quit,
    }
}
use action::Action;

mod effect {
    use insh_api::Request;

    pub enum Effect {
        Request(Request),
        Bell,
        Quit,
    }
}
pub use effect::Effect;

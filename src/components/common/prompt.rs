mod props {
    use super::Choice;

    pub struct Props<Effect> {
        pub text: String,
        pub choices: Vec<Choice<Effect>>,
    }

    impl<Effect> Props<Effect> {
        pub fn builder() -> Builder<Effect> {
            Builder::new()
        }
    }

    mod builder {
        use super::super::Choice;
        use super::Props;

        pub struct Builder<Effect> {
            text: Option<String>,
            choices: Option<Vec<Choice<Effect>>>,
        }

        impl<Effect> Builder<Effect> {
            pub fn new() -> Self {
                Self {
                    text: None,
                    choices: None,
                }
            }

            pub fn text(mut self, text: String) -> Self {
                self.text = Some(text);
                self
            }

            pub fn choices(mut self, choices: Vec<Choice<Effect>>) -> Self {
                self.choices = Some(choices);
                self
            }

            pub fn build(self) -> Props<Effect> {
                let Self { text, choices } = self;

                Props {
                    text: text.expect("Text not set."),
                    choices: choices.expect("Choices not set."),
                }
            }
        }
    }
    pub use builder::Builder;
}
pub use props::Props;

mod prompt {
    use super::{Choice, Event, Props};

    use crate::misc::align::Align;
    use crate::rendering::{Fabric, JoinAndWrap, Size, VerticallyCentered, Yarn};
    use crate::Component;

    use std::iter;

    pub struct Prompt<Effect> {
        text: String,
        choices: Vec<Choice<Effect>>,
    }

    impl<Effect> Component<Props<Effect>, Event, Effect> for Prompt<Effect> {
        fn new(props: Props<Effect>) -> Self {
            Self {
                text: props.text,
                choices: props.choices,
            }
        }

        fn handle(&mut self, _event: Event) -> Option<Effect> {
            todo!();
        }

        fn render(&self, size: Size) -> Fabric {
            let text_yarns: Vec<Yarn> = Yarn::wrapped(&self.text, size.columns, Align::Center);

            let choices = self.choices.iter().map(|choice| Yarn::from(choice.text()));
            let choices: JoinAndWrap = JoinAndWrap::builder()
                .yarns(choices)
                .wrap(size.columns)
                .build();

            let lines = text_yarns
                .into_iter()
                .chain(iter::once(Yarn::blank(size.columns)))
                .chain(choices.into_iter());

            VerticallyCentered::new(lines).into_fabric(size)
        }
    }
}
pub use prompt::Prompt;

mod choice {
    #[derive(Clone)]
    pub struct Choice<Effect> {
        text: String,
        #[allow(dead_code)]
        effect: Effect,
    }

    impl<Effect> Choice<Effect> {
        pub fn text(&self) -> &str {
            &self.text
        }
    }

    impl<Effect> Choice<Effect> {
        pub fn builder() -> Builder<Effect> {
            Builder::new()
        }
    }

    mod builder {
        use super::Choice;

        pub struct Builder<Effect> {
            text: Option<String>,
            effect: Option<Effect>,
        }

        impl<Effect> Builder<Effect> {
            pub fn new() -> Self {
                Self {
                    text: None,
                    effect: None,
                }
            }

            pub fn text_str(mut self, text: &str) -> Self {
                self.text = Some(text.to_string());
                self
            }

            #[allow(dead_code)]
            pub fn text(mut self, text: String) -> Self {
                self.text = Some(text);
                self
            }

            pub fn effect(mut self, effect: Effect) -> Self {
                self.effect = Some(effect);
                self
            }

            pub fn build(self) -> Choice<Effect> {
                let Self { text, effect } = self;

                Choice {
                    text: text.expect("Text not set."),
                    effect: effect.expect("Effect not set."),
                }
            }
        }
    }
    pub use builder::Builder;
}
pub use choice::Choice;

mod event {
    pub enum Event {}
}
pub use event::Event;

/*!
The bar which is shown along the bottom of a component.
*/

mod info {
    /// What a component shows in its footer.
    ///
    /// Only the name of the component is shown by default. Anything else is up to the component,
    /// since what is worth showing is different for each one.
    pub trait Info {
        /// Return what is shown on the left of the footer.
        fn text(&self) -> String {
            String::new()
        }

        /// Return where you are in what is shown, which goes just before the name.
        fn position(&self) -> String {
            String::new()
        }
    }
}
pub use info::Info;

mod props {
    use super::Info;

    use typed_builder::TypedBuilder;

    #[derive(TypedBuilder)]
    pub struct Props<'a, I: Info> {
        /// The name of the component the footer is for.
        #[builder(setter(into))]
        pub name: String,
        /// What the component shows in its footer.
        pub info: &'a I,
    }
}
pub use props::Props;

mod footer {
    use std::cmp;

    use super::{Effect, Event, Info, Props, State};
    use crate::color::Color;

    use rend::{Fabric, Size, Yarn};
    use til::Component;

    pub struct Footer<'a, I: Info> {
        state: State<'a, I>,
    }

    impl<'a, I: Info> Component<Props<'a, I>, Event, Effect> for Footer<'a, I> {
        fn new(props: Props<'a, I>) -> Self {
            let state = State::from(props);
            Self { state }
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            // The footer is shown from what it is given, so there is nothing to handle.
            match event {}
        }

        /// Render the text of the component on the left and where you are (the position and the
        /// name of the component) on the right.
        ///
        /// The left is what is cut off when there is not enough room for everything, since where
        /// you are is worth more than how you got there.
        fn render(&self, size: Size) -> Fabric {
            let text: String = self.state.info().text();

            let position: String = self.state.info().position();
            let right: String = match position.is_empty() {
                true => self.state.name().to_string(),
                false => format!("{}  {}", position, self.state.name()),
            };

            let right_len: usize = cmp::min(right.chars().count(), size.columns);
            // Keep a blank column between the text and where you are so that they never run
            // together.
            let text_len: usize = cmp::min(
                text.chars().count(),
                size.columns.saturating_sub(right_len + 1),
            );

            let mut characters: Vec<char> = Vec::with_capacity(size.columns);
            characters.extend(text.chars().take(text_len));
            characters.extend(vec![' '; size.columns - text_len - right_len]);
            characters.extend(right.chars().skip(right.chars().count() - right_len));

            let mut yarn = Yarn::from(characters);
            yarn.color(Color::InvertedText.into());
            yarn.background(Color::FooterBackground.into());

            Fabric::from(yarn)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use test_case::test_case;

        /// What a component shows in its footer for testing.
        struct TestInfo {
            text: String,
            position: String,
        }

        impl Info for TestInfo {
            fn text(&self) -> String {
                self.text.clone()
            }

            fn position(&self) -> String {
                self.position.clone()
            }
        }

        /// Return what a footer of the given size shows.
        fn render(text: &str, position: &str, name: &str, columns: usize) -> String {
            let info = TestInfo {
                text: text.to_string(),
                position: position.to_string(),
            };
            let props = Props::builder().name(name).info(&info).build();

            let fabric: Fabric = Footer::new(props).render(Size::new(1, columns));

            fabric.characters()[0].iter().collect()
        }

        #[test_case("", "", "file creator", 20, "        file creator"; "only the name")]
        #[test_case("", "1/10", "browser", 20, "       1/10  browser"; "no text")]
        #[test_case("", "", "browser", 0, ""; "no room for anything")]
        #[test_case("12 files searched (0.05s)", "2/3", "finder", 40, "12 files searched (0.05s)    2/3  finder"; "everything fits")]
        #[test_case("12 files searched (0.05s)", "2/3", "finder", 37, "12 files searched (0.05s) 2/3  finder"; "everything just fits")]
        #[test_case("12 files searched (0.05s)", "2/3", "finder", 20, "12 files 2/3  finder"; "the text is cut off")]
        #[test_case("12 files searched (0.05s)", "2/3", "finder", 8, "  finder"; "where you are is cut off")]
        #[test_case("1 file searched (0.05s)", "-/142", "searcher", 30, "1 file searche -/142  searcher"; "a file is selected")]
        fn test_render(text: &str, position: &str, name: &str, columns: usize, expected: &str) {
            let string: String = render(text, position, name, columns);

            assert_eq!(string, expected);
            assert_eq!(string.chars().count(), columns);
        }
    }
}
pub use footer::Footer;

mod event {
    pub enum Event {}
}
pub use event::Event;

mod state {
    use super::{Info, Props};

    pub struct State<'a, I: Info> {
        /// The name of the component the footer is for.
        name: String,
        /// What the component shows in its footer.
        info: &'a I,
    }

    impl<'a, I: Info> From<Props<'a, I>> for State<'a, I> {
        fn from(props: Props<'a, I>) -> Self {
            Self {
                name: props.name,
                info: props.info,
            }
        }
    }

    impl<'a, I: Info> State<'a, I> {
        /// Return the name of the component the footer is for.
        pub fn name(&self) -> &str {
            &self.name
        }

        /// Return what the component shows in its footer.
        pub fn info(&self) -> &'a I {
            self.info
        }
    }
}
use state::State;

mod effect {
    pub enum Effect {}
}
pub use effect::Effect;

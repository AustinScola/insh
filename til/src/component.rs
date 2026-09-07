//! A component.

use std::any::type_name;

use rend::{Fabric, Size};

/// A component.
pub trait Component<Props, Event, Effect> {
    /// Return a new component.
    fn new(props: Props) -> Self
    where
        Self: Sized;

    /// Return the name of the component.
    ///
    /// The type of the component is used unless it says what it is called, which components which
    /// show their name to the user are expected to do. The words of a type which has more than one
    /// run together, so those are the components most worth naming.
    fn name(&self) -> String {
        let name: &str = type_name::<Self>();
        // Drop the types which a generic component is for and then the modules which it is in.
        let name: &str = name.split('<').next().unwrap_or(name);
        let name: &str = name.rsplit("::").next().unwrap_or(name);

        name.to_lowercase()
    }

    /// Return the effects for when the component is created.
    fn on_created(&mut self) -> Option<Box<dyn Iterator<Item = Effect>>> {
        None
    }

    /// Handle an event, returning the effect to perform for it.
    fn handle(&mut self, event: Event) -> Option<Effect>;

    /// Render the component at the given size.
    fn render(&self, size: Size) -> Fabric;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A component which does not say what it is called.
    struct Unnamed;

    impl Component<(), (), ()> for Unnamed {
        fn new(_props: ()) -> Self {
            Self
        }

        fn handle(&mut self, _event: ()) -> Option<()> {
            None
        }

        fn render(&self, _size: Size) -> Fabric {
            Fabric::default()
        }
    }

    /// A component which is for another component.
    struct Generic<T> {
        /// The component which this one is for.
        _component: T,
    }

    impl<T> Component<T, (), ()> for Generic<T> {
        fn new(component: T) -> Self {
            Self {
                _component: component,
            }
        }

        fn handle(&mut self, _event: ()) -> Option<()> {
            None
        }

        fn render(&self, _size: Size) -> Fabric {
            Fabric::default()
        }
    }

    /// The components which are not owned by another one are boxed, so the name has to be usable
    /// without knowing the type of the component.
    #[test]
    fn test_a_component_is_named_after_its_type() {
        let component: Box<dyn Component<(), (), ()>> = Box::new(Unnamed);

        assert_eq!(component.name(), "unnamed");
    }

    #[test]
    fn test_the_types_a_generic_component_is_for_are_not_part_of_its_name() {
        let component = Generic::new(Unnamed);

        assert_eq!(component.name(), "generic");
    }
}

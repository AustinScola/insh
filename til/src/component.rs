use rend::{Fabric, Size};

pub trait Component<Props, Event, Effect> {
    fn new(props: Props) -> Self
    where
        Self: Sized;

    fn handle(&mut self, event: Event) -> Option<Effect>;

    fn render(&self, size: Size) -> Fabric;
}

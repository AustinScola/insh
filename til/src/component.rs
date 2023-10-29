use rend::{Fabric, Size};

pub trait Component<Props, Event, Effect> {
    fn new(props: Props) -> Self
    where
        Self: Sized;

    fn on_created(&mut self) -> Option<Box<dyn Iterator<Item = Effect>>> {
        None
    }

    fn handle(&mut self, event: Event) -> Option<Effect>;

    fn render(&self, size: Size) -> Fabric;
}

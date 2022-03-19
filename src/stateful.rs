pub trait Stateful<Action, Effect> {
    fn perform(&mut self, action: Action) -> Option<Effect>;
}

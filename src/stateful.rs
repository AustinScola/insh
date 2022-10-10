/*!
State-machine related functionality.
*/

/// A state-machine.
pub trait Stateful<Action, Effect> {
    /// Perform an action on the state and optionally emit an side effect.
    fn perform(&mut self, action: Action) -> Option<Effect>;
}

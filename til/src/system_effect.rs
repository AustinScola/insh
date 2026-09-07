/*!
The side effects which a component can ask the app to perform.
*/
use crate::program::Program;

/// A side effect for the app to perform.
pub enum SystemEffect<Request> {
    /// Run a program.
    RunProgram {
        /// The program to run.
        program: Box<dyn Program>,
    },

    /// A request to the backend.
    Request(Request),

    /// Make the bell sound.
    Bell,

    /// Exit Insh.
    Exit,
}

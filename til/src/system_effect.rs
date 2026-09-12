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

    /// More than one request to the backend, made in the order they are in. One event can call for
    /// more than one, as going to a directory calls for both the files in it and a note that it
    /// was gone to.
    Requests(Vec<Request>),

    /// Make the bell sound.
    Bell,

    /// Exit Insh.
    Exit,
}

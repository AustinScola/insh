/*!
System effects are side-effect that components can emit which the application framework will handle.
*/
use crate::program::Program;

/// A side-effect that components can emit which the application framework will handle.
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

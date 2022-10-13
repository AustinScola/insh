/*!
System effects are side-effect that components can emit which the application framework will handle.
*/
use crate::program::Program;

/// A side-effect that components can emit which the application framework will handle.
pub enum SystemEffect {
    /// Run a program.
    RunProgram {
        /// The program to run.
        program: Box<dyn Program>,
    },

    /// Make the bell sound.
    Bell,

    /// Exit Insh.
    Exit,
}

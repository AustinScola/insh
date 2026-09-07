/*!
The programs which can be run, taking over the terminal while they do.
*/
use std::ffi::OsString;
use std::path::PathBuf;

/**
A program that can be run and is allowed to take over rendering of the terminal.
*/
pub trait Program: Send {
    /// Return the set up that must occur before the program is run.
    fn setup(&self) -> ProgramSetup {
        ProgramSetup::default()
    }

    /// Return the cleanup that must occur after the program is run.
    fn cleanup(&self) -> ProgramCleanup {
        ProgramCleanup::default()
    }

    /// Return the filename of the program.
    fn filename(&self) -> OsString;

    /// Return the arguments for running the program.
    fn args(&self) -> Vec<OsString> {
        vec![]
    }

    /// Return the directory to run the program in.
    fn cwd(&self) -> Option<PathBuf> {
        None
    }

    /// Return the environment variables to run the program with.
    fn env(&self) -> Vec<EnvVar> {
        vec![]
    }

    /// Return the pipe for the program's stdout.
    fn stdout_pipe(&self) -> Option<Box<dyn StdoutPipe>> {
        None
    }
}

/// Contains the [`ProgramSetup`] struct.
mod program_setup {
    /**
    Set up that must occur before an associated [`Program`](super::Program) is run.
    */
    #[derive(Default)]
    pub struct ProgramSetup {
        /// The terminal screen must be cleared.
        pub clear_screen: bool,
        /// The cursor must be moved to the home location.
        pub cursor_home: bool,
        /// The cursor must be set to visible.
        pub cursor_visible: Option<bool>,
    }

    impl ProgramSetup {
        /// Return whether any set up is needed.
        pub fn any(&self) -> bool {
            self.clear_screen | self.cursor_home | (self.cursor_visible == Some(true))
        }
    }
}
pub use program_setup::ProgramSetup;

/// Contains the [`ProgramCleanup`] struct.
mod program_cleanup {

    /// Cleanup after a program runs.
    #[derive(Default)]
    pub struct ProgramCleanup {
        /// The cursor must be hidden.
        pub hide_cursor: bool,
        /// The raw terminal must be enabled.
        pub enable_raw_terminal: bool,
    }

    impl ProgramCleanup {
        /// Return whether any cleanup is needed.
        pub fn any(&self) -> bool {
            self.hide_cursor | self.enable_raw_terminal
        }
    }
}
pub use program_cleanup::ProgramCleanup;

/// Contains the [`StdoutPipe`] trait.
mod stdout_pipe {
    use std::fs::File;

    /// A pipe for a program's stdout.
    pub trait StdoutPipe: Send {
        /// Pass the output of the program through.
        fn run(&mut self, _stdout: &mut File) {}
    }
}
pub use stdout_pipe::StdoutPipe;

/// Contains the [`EnvVar`] struct.
mod env_var {
    use std::ffi::CString;

    use typed_builder::TypedBuilder;

    /// An environment variable to run a program with.
    #[derive(TypedBuilder)]
    pub struct EnvVar {
        /// The name of the variable.
        pub name: CString,
        /// The value of the variable.
        pub value: CString,
    }
}
pub use env_var::EnvVar;

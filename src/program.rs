/*!
This module contains the [`Program`] trait which is used to represent programs that can be run.
*/

/**
A program that can be run and is allowed to take over rendering of the terminal.
*/
pub trait Program {
    /// Return the set up that must occur before the program is run.
    fn setup(&self) -> ProgramSetup {
        ProgramSetup::default()
    }

    /// Return the cleanup that must occur after the program is run.
    fn cleanup(&self) -> ProgramCleanup {
        ProgramCleanup::default()
    }

    /// Run the program.
    fn run(&self);
}

/**
This module contains the [`ProgramSetup`] struct which is used to represent set up that must occur
before an associated [`Program`] is run.
*/
mod program_setup {
    /**
    Set up that must occur before an associated [`Program`](super::Program) is run.
    */
    #[derive(Default)]
    pub struct ProgramSetup {
        /// The terminal should be set to raw.
        pub raw_terminal: Option<bool>,
        /// The terminal screen must be cleared.
        pub clear_screen: bool,
        /// The cursor must be moved to the home location.
        pub cursor_home: bool,
        /// The cursor must be set to visible.
        pub cursor_visible: Option<bool>,
    }

    impl ProgramSetup {
        /// Return if set up must occur before the associated program is run.
        pub fn any(&self) -> bool {
            (self.raw_terminal == Some(true))
                | self.clear_screen
                | self.cursor_home
                | (self.cursor_visible == Some(true))
        }
    }
}
pub use program_setup::ProgramSetup;

/// This module contains the [`ProgramCleanup`] struct which is used to represent cleanup that must
/// happen after a program runs.
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
        /// Return if any program cleanup must occur.
        pub fn any(&self) -> bool {
            self.hide_cursor | self.enable_raw_terminal
        }
    }
}
pub use program_cleanup::ProgramCleanup;

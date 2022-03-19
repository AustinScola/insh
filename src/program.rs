use std::process::Command;

pub trait Program {
    fn setup(&self) -> ProgramSetup {
        ProgramSetup::default()
    }

    fn cleanup(&self) -> ProgramCleanup {
        ProgramCleanup::default()
    }
    fn run(&self) -> Command;
}

mod program_setup {
    #[derive(Default)]
    pub struct ProgramSetup {
        pub raw_terminal: Option<bool>,
        pub clear_screen: bool,
        pub cursor_home: bool,
        pub cursor_visible: Option<bool>,
    }

    impl ProgramSetup {
        pub fn any(&self) -> bool {
            (self.raw_terminal == Some(true))
                | self.clear_screen
                | self.cursor_home
                | (self.cursor_visible == Some(true))
        }
    }
}
pub use program_setup::ProgramSetup;

mod program_cleanup {
    #[derive(Default)]
    pub struct ProgramCleanup {
        pub hide_cursor: bool,
        pub enable_raw_terminal: bool,
    }

    impl ProgramCleanup {
        pub fn any(&self) -> bool {
            self.hide_cursor | self.enable_raw_terminal
        }
    }
}
pub use program_cleanup::ProgramCleanup;

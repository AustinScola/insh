use crate::program::Program;

pub enum SystemEffect {
    Exit,
    RunProgram { program: Box<dyn Program> },
}

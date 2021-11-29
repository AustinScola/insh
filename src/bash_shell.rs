use std::path::Path;
use std::process::{Child, Command, Stdio};

pub struct BashShell {}

impl BashShell {
    pub fn run(directory: &Path) {
        let mut bash_shell: Child = Command::new("bash")
            .current_dir(directory)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .spawn()
            .unwrap();
        bash_shell.wait().unwrap();
    }
}

use std::path::Path;
use std::process::{Child, Command, Stdio};

pub struct Vim {}

impl Vim {
    pub fn run(file: &Path) {
        let mut vim: Child = Command::new("vim")
            .arg(file)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .spawn()
            .unwrap();
        vim.wait().unwrap();
    }
}

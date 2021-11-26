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

    pub fn run_with_command(file: &Path, command: String) {
        let mut command_argument = String::from("-c");
        command_argument.push_str(&command);

        let mut vim: Child = Command::new("vim")
            .arg(command_argument)
            .arg(file)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .spawn()
            .unwrap();
        vim.wait().unwrap();
    }

    pub fn run_at_line(file: &Path, line_number: usize) {
        let one_based_line_number = line_number + 1;
        let mut start_line_argument = String::from("+");
        start_line_argument.push_str(&one_based_line_number.to_string());

        let mut vim: Child = Command::new("vim")
            .arg(start_line_argument)
            .arg(file)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .spawn()
            .unwrap();
        vim.wait().unwrap();
    }
}

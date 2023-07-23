use std::fs::File;
use std::io::{self, Read, Write};

use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct OutputForwarder {
    master_stdout: File,
}

impl OutputForwarder {
    pub fn run(&mut self) {
        let mut stdout = io::stdout().lock();

        let mut buffer: [u8; 1] = [0; 1];
        loop {
            let length = match self.master_stdout.read(&mut buffer) {
                Ok(length) => length,
                Err(_) => break,
            };
            if length == 0 {
                continue;
            }
            let _ = stdout.write(&buffer[0..length]).unwrap();
            stdout.flush().unwrap();
        }
    }
}

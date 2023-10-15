use std::fs::File;
use std::io::{self, Read, Write};

use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct OutputForwarder {
    master_stdout: File,
}

impl OutputForwarder {
    pub fn run(&mut self) {
        #[cfg(feature = "logging")]
        log::debug!("Output forwarder running...");
        let mut stdout = io::stdout().lock();

        let mut buffer: [u8; 1] = [0; 1];
        loop {
            let length = match self.master_stdout.read(&mut buffer) {
                Ok(length) => length,
                Err(_) => break,
            };
            if length == 0 {
                // NOTE: On MacOS, it appears that reading from the master stdout does not return an
                // error when the program terminates. Instead read returns 0 bytes.
                #[cfg(feature = "logging")]
                log::debug!("Output forwarder received no bytes.");
                break;
            }
            let _ = stdout.write(&buffer[0..length]).unwrap();
            stdout.flush().unwrap();
        }
        #[cfg(feature = "logging")]
        log::debug!("Output forwarder stopping...");
    }
}

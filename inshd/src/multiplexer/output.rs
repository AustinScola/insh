//! What a program running in a pane has written, the reader which brings it in, and the writer
//! which sends what was typed back.

use std::fs::File;
use std::io::{Read, Write};

use crossbeam::channel::{Receiver, Sender};
use nix::sys::wait::waitpid;
use nix::unistd::Pid;
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Something which the program running in a pane has done.
#[derive(Debug)]
pub enum Output {
    /// It wrote the bytes.
    Wrote {
        /// Which pane it is running in.
        pane: Uuid,
        /// Which program it is, since a pane may have had more than one in it.
        child: Pid,
        /// What it wrote.
        bytes: Vec<u8>,
    },
    /// It has finished.
    Finished {
        /// Which pane it was running in.
        pane: Uuid,
        /// Which program it was.
        child: Pid,
    },
}

/// Reads what the program running in a pane writes.
///
/// There is one of these per pane, on a thread of its own, because reading from a pseudoterminal
/// waits until there is something to read and a pane must not hold up any of the others.
#[derive(TypedBuilder)]
pub struct Reader {
    /// Which pane it is reading.
    pane: Uuid,
    /// The program it is reading.
    child: Pid,
    /// The pseudoterminal the program is running on.
    master: File,
    /// A sender of what the program writes.
    output_tx: Sender<Output>,
}

/// Sends what was typed to the program running in a pane.
///
/// There is one of these per pane, on a thread of its own, because a pseudoterminal only takes so
/// much before it makes whoever is writing wait for the program to read some of it. A program
/// which stops reading would otherwise stop the one thread which serves every client.
#[derive(TypedBuilder)]
pub struct Writer {
    /// Which pane it is writing to.
    pane: Uuid,
    /// The pseudoterminal the program is running on.
    master: File,
    /// A receiver of what was typed.
    typed_rx: Receiver<Vec<u8>>,
}

impl Writer {
    /// Send what was typed to the program until there is no more of it to send.
    pub fn run(&mut self) {
        log::debug!("Writer for pane {} running.", self.pane);

        while let Ok(bytes) = self.typed_rx.recv() {
            if let Err(error) = self.master.write_all(&bytes) {
                log::debug!("Stopped writing to pane {}: {}", self.pane, error);
                break;
            }
        }

        log::debug!("Writer for pane {} stopping.", self.pane);
    }
}

impl Reader {
    /// How many bytes are read at a time.
    const READ_SIZE: usize = 4096;

    /// Read what the program writes until it has finished.
    pub fn run(&mut self) {
        log::debug!("Reader for pane {} running.", self.pane);

        let mut buffer: [u8; Self::READ_SIZE] = [0; Self::READ_SIZE];
        loop {
            let len: usize = match self.master.read(&mut buffer) {
                // NOTE: Reading nothing at all is what a pseudoterminal does when the program on
                // the other end of it has gone, rather than failing.
                Ok(0) => break,
                Ok(len) => len,
                Err(error) => {
                    log::debug!("Stopped reading pane {}: {}", self.pane, error);
                    break;
                }
            };

            let output = Output::Wrote {
                pane: self.pane,
                child: self.child,
                bytes: buffer[..len].to_vec(),
            };
            if self.output_tx.send(output).is_err() {
                break;
            }
        }

        // NOTE: The program is waited for here rather than wherever the pane is, because this is
        // a thread of its own and waiting is allowed to take as long as it takes. There is no
        // saying a program has gone just because there is nothing left to read from it: it may
        // have left something of its own behind which is holding the pseudoterminal open. Waiting
        // for it anywhere else would stop everything until it did.
        if let Err(error) = waitpid(self.child, None) {
            log::warn!(
                "Failed to wait for the program of pane {}: {}",
                self.pane,
                error
            );
        }

        let _ = self.output_tx.send(Output::Finished {
            pane: self.pane,
            child: self.child,
        });
        log::debug!("Reader for pane {} stopping.", self.pane);
    }
}

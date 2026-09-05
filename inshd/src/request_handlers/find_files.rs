//! Handles requests to find files.
use std::thread::{self, JoinHandle};

use crate::file_finder::{FileFinder, FileFinderOptions};
use crate::file_finder::{FindFilesResult, FoundFiles};

use insh_api::{
    FindFilesRequestParams, FindFilesResponseParams, ResponseParams, ResponseParamsAndLast,
};

use crossbeam::channel::{self, Receiver, Sender};

/// Handles a request to find files.
pub struct FindFiles {
    /// A receiver for results of finding files.
    results_rx: Receiver<FindFilesResult>,
    /// A handle to the thread for finding files.
    file_finder_handle: Option<JoinHandle<()>>,
    /// If finding files is done.
    done: bool,
}

impl FindFiles {
    /// Find files.
    pub fn run(params: &FindFilesRequestParams) -> FindFiles {
        // Create and start a thread to perform the finding of files.
        let (results_tx, results_rx): (Sender<FindFilesResult>, Receiver<FindFilesResult>) =
            channel::unbounded();
        let mut file_finder: FileFinder = FileFinder::builder().results_tx(results_tx).build();
        let file_finder_options: FileFinderOptions = FileFinderOptions::builder()
            .dir(params.dir())
            .pattern(params.pattern())
            .build();
        let file_finder_handle: JoinHandle<()> = thread::Builder::new()
            .name("file-finder".to_string())
            .spawn(move || file_finder.run(file_finder_options))
            .unwrap();

        FindFiles {
            results_rx,
            file_finder_handle: Some(file_finder_handle),
            done: false,
        }
    }
}

impl Iterator for FindFiles {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let result: FindFilesResult = match self.results_rx.recv() {
            Ok(result) => result,
            Err(error) => {
                log::error!(
                    "Error receiving find files result from file finder thread: {}",
                    error
                );
                todo!();
            }
        };

        let found: FoundFiles = match result {
            Ok(found) => found,
            Err(error) => {
                log::error!("Error finding files: {}", error);
                todo!();
            }
        };

        if found.done {
            self.done = true;
            let file_finder_handle: JoinHandle<()> = self.file_finder_handle.take().unwrap();
            let _ = file_finder_handle.join();
        }

        return Some(
            ResponseParamsAndLast::builder()
                .response_params(ResponseParams::FindFiles(
                    FindFilesResponseParams::builder()
                        .entries(found.entries)
                        .searched(found.searched)
                        .duration(found.duration)
                        .build(),
                ))
                .last(found.done)
                .build(),
        );
    }
}

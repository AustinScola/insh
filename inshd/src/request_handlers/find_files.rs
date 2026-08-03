//! Handles requests to find files.
use std::thread::{self, JoinHandle};

use crossbeam::channel::{self, select, Receiver, Sender};

use insh_api::{
    FindFilesRequestParams, FindFilesResponseParams, ResponseParams, ResponseParamsAndLast,
};
use path_finder::Entry;

use crate::file_finder::FindFilesResult;
use crate::file_finder::{FileFinder, FileFinderOptions};

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

        select! {
            recv(self.results_rx) -> result => {
                let result: FindFilesResult = match result {
                    Ok(result) => result,
                    Err(error) => {
                        log::error!("Error receiving find files result from file finder thread: {}", error);
                        todo!();
                    }
                };

                let entry: Option<Entry> = match result {
                    Ok(entry) => entry,
                    Err(error) => {
                        log::error!("Error finding files: {}", error);
                        todo!();
                    }
                };

                let entry: Entry = match entry {
                    Some(entry) => entry,
                    None => {
                        self.done = true;
                        let file_finder_handle: JoinHandle<()> = self.file_finder_handle.take().unwrap();
                        let _ = file_finder_handle.join();
                        return Some(ResponseParamsAndLast::builder()
                            .response_params(
                                ResponseParams::FindFiles(
                                    FindFilesResponseParams::builder()
                                        .entries(vec![])
                                        .build()
                                )
                            )
                            .last(true)
                            .build());
                    }
                };

                return Some(ResponseParamsAndLast::builder()
                    .response_params(
                        ResponseParams::FindFiles(
                            FindFilesResponseParams::builder()
                                .entries(vec![entry])
                                .build()
                        )
                    )
                    .last(false)
                    .build());
            }
        }
    }
}

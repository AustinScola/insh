//! Handles requests from clients.
use std::fs::{DirBuilder, File};
use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use crossbeam::channel::{self, select, Receiver, Sender};
use typed_builder::TypedBuilder;

use file_type::FileType;
use insh_api::{
    CreateFileError, CreateFileRequestParams, CreateFileResponseParams, FindFilesRequestParams,
    FindFilesResponseParams, Request, RequestParams, Response, ResponseParams,
    ResponseParamsAndLast,
};
use path_finder::Entry;

use crate::file_finder::FindFilesResult;
use crate::file_finder::{FileFinder, FileFinderOptions};
use crate::stop::Stop;

/// Handles requests from clients.
#[derive(TypedBuilder)]
pub struct RequestHandler {
    /// The request handler number.
    #[allow(dead_code)]
    number: usize,
    /// A receiver for requests.
    requests: Receiver<Request>,
    /// A sender for responses.
    responses: Sender<Response>,
    /// A receiver for a stop sentinel.
    stop_rx: Receiver<Stop>,
}

impl RequestHandler {
    /// Run the request handler.
    pub fn run(&mut self) {
        log::info!("Request handler running.");

        loop {
            select! {
                recv(self.stop_rx) -> _stop => {
                    break;
                }
                recv(self.requests) -> request => {
                    let request: Request = request.unwrap();
                    log::info!("Handling request {}.", request.uuid());

                    let response_params_and_last_iter: Box<dyn Iterator<Item = ResponseParamsAndLast>> = match request.params() {
                        RequestParams::FindFiles(params) => Box::new(FindFiles::run(params)),
                        RequestParams::CreateFile(params) => Box::new(CreateFile::new(params)),
                    };

                    let mut sent_last: bool = false;
                    let mut send_error: bool = false;
                    for response_params_and_last in response_params_and_last_iter {
                        let response = Response::builder()
                            .uuid(*request.uuid())
                            .last(response_params_and_last.last)
                            .params(response_params_and_last.response_params)
                            .build();

                        if response_params_and_last.last {
                            if sent_last {
                                log::error!("Multiple last responses.");
                                break;
                            }
                            sent_last = true;
                        }

                        if let Err(error) = self.responses.send(response) {
                            log::error!("Error sending response: {}", error);
                            send_error = true;
                            break;
                        }
                    }
                    if !sent_last && !send_error {
                        log::warn!("Never received last response.");
                    }

                    log::info!("Done handling request {}.", request.uuid());
                }
            }
        }

        log::info!("Request handler stopping...");
    }
}

/// Context for a request.
#[derive(TypedBuilder)]
pub struct Context {}

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

/// Handles creating a file.
struct CreateFile {
    /// The path of the file to create.
    path: PathBuf,
    /// The type of file to create.
    file_type: FileType,
    /// Whether or not created the file is done.
    done: bool,
}

impl CreateFile {
    /// Return a file creator.
    fn new(params: &CreateFileRequestParams) -> Self {
        Self {
            path: params.path().to_path_buf(),
            file_type: params.file_type(),
            done: false,
        }
    }
}

impl Iterator for CreateFile {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let response_params: ResponseParams = if self.path.exists() {
            ResponseParams::CreateFile(
                CreateFileResponseParams::builder()
                    .result(Err(CreateFileError::AlreadyExists(self.path.clone())))
                    .build(),
            )
        } else {
            match self.file_type {
                FileType::File => {
                    log::info!("Creating file {:?}...", self.path);
                    match File::create(&self.path) {
                        Ok(_) => {
                            log::info!("Created file {:?}.", self.path);
                            ResponseParams::CreateFile(
                                CreateFileResponseParams::builder().result(Ok(())).build(),
                            )
                        }
                        Err(io_error) => {
                            log::error!("Error creating file: {}", io_error);
                            ResponseParams::CreateFile(
                                CreateFileResponseParams::builder()
                                    .result(Err(CreateFileError::Other(format!("{}", io_error))))
                                    .build(),
                            )
                        }
                    }
                }
                FileType::Dir => {
                    log::info!("Creating directory {:?}...", self.path);
                    match DirBuilder::new().create(&self.path) {
                        Ok(_) => {
                            log::info!("Created directory {:?}.", self.path);
                            ResponseParams::CreateFile(
                                CreateFileResponseParams::builder().result(Ok(())).build(),
                            )
                        }
                        Err(io_error) => {
                            log::error!("Error creating directory: {}", io_error);
                            ResponseParams::CreateFile(
                                CreateFileResponseParams::builder()
                                    .result(Err(CreateFileError::Other(format!("{}", io_error))))
                                    .build(),
                            )
                        }
                    }
                }
            }
        };

        self.done = true;

        Some(
            ResponseParamsAndLast::builder()
                .response_params(response_params)
                .last(true)
                .build(),
        )
    }
}

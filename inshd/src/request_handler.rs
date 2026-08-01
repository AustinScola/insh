//! Handles requests from clients.
use std::ffi::OsStr;
use std::fs::{self, DirBuilder, DirEntry, File, Metadata, ReadDir};
use std::io::{Error as IOError, ErrorKind as IOErrorKind};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use crossbeam::channel::{self, select, Receiver, Sender};
use nix::unistd::{Gid, Group, Uid, User};
use typed_builder::TypedBuilder;

use file_info::{FileInfo, FileMetadata};
use file_type::FileType;
use insh_api::{
    CreateFileError, CreateFileRequestParams, CreateFileResponseParams, CreateFileResult,
    FileSortOptions, FindFilesRequestParams, FindFilesResponseParams, GetFilesError,
    GetFilesRequestParams, GetFilesResponseParams, GetFilesResult, HiddenFileSort, Request,
    RequestParams, Response, ResponseParams, ResponseParamsAndLast,
};
use path_finder::Entry;

use crate::cache::Cache;
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
                        RequestParams::GetFiles(params) => Box::new(GetFiles::new(params)),
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

/// Handles a request to get files.
struct GetFiles {
    /// The directory to get files for.
    dir: PathBuf,
    /// How the files should be sorted, or `None` if they should not be sorted.
    sort: Option<FileSortOptions>,
    /// Whether or not the metadata of the files should be included.
    metadata: bool,
    /// If getting files is done.
    done: bool,
}

impl GetFiles {
    /// Return a new handler for getting files.
    pub fn new(params: &GetFilesRequestParams) -> Self {
        Self {
            dir: params.dir().to_path_buf(),
            sort: params.sort(),
            metadata: params.metadata(),
            done: false,
        }
    }

    /// Return the metadata of a file, or `None` if it could not be read.
    fn metadata(
        dir_entry: &DirEntry,
        file_type: &Result<FileType, String>,
        users: &mut Cache<u32, Option<String>>,
        groups: &mut Cache<u32, Option<String>>,
    ) -> Option<FileMetadata> {
        // NOTE: The metadata of a dir entry is *not* followed through symlinks which is what `ls`
        // does when listing files too.
        let metadata: Metadata = match dir_entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                log::warn!(
                    "Error reading the metadata of {:?}: {}",
                    dir_entry.path(),
                    error
                );
                return None;
            }
        };

        let link_target: Option<PathBuf> = match file_type {
            Ok(FileType::Symlink) => fs::read_link(dir_entry.path()).ok(),
            _ => None,
        };

        let uid: u32 = metadata.uid();
        let gid: u32 = metadata.gid();

        Some(
            FileMetadata::builder()
                .mode(metadata.mode())
                .hard_links(metadata.nlink())
                .uid(uid)
                .user(users.get(uid, |uid| {
                    User::from_uid(Uid::from_raw(*uid))
                        .ok()
                        .flatten()
                        .map(|user| user.name)
                }))
                .gid(gid)
                .group(groups.get(gid, |gid| {
                    Group::from_gid(Gid::from_raw(*gid))
                        .ok()
                        .flatten()
                        .map(|group| group.name)
                }))
                .size(metadata.size())
                .modified(metadata.mtime())
                .link_target(link_target)
                .build(),
        )
    }

    /// Sort the files by name.
    fn sort(file_infos: &mut [FileInfo], options: FileSortOptions) {
        file_infos.sort_by_cached_key(|file_info| {
            let name: &OsStr = file_info.name().unwrap_or_else(|| OsStr::new(""));
            let mut key: String = name.to_string_lossy().to_string();

            // Group the hidden files before or after the other files (if they are not mixed in
            // with them).
            let hidden: bool = key.starts_with('.');
            let group: u8 = match options.hidden() {
                HiddenFileSort::First => !hidden as u8,
                HiddenFileSort::Last => hidden as u8,
                HiddenFileSort::Mixed => {
                    // Sort hidden files as if they were not hidden.
                    if let Some(stripped) = key.strip_prefix('.') {
                        key = stripped.to_string();
                    }
                    0
                }
            };

            if options.case_insensitive() {
                key = key.to_lowercase();
            }

            // Fall back on the name itself so that the order of files with the same key is
            // deterministic.
            (group, key, name.to_os_string())
        });
    }
}

impl Iterator for GetFiles {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let read_dir: Result<ReadDir, IOError> = fs::read_dir(&self.dir);
        let get_files_result: GetFilesResult = match read_dir {
            Ok(dir_entries) => {
                let mut file_infos: Vec<FileInfo> = Vec::new();

                // Names are looked up once per user/group instead of once per file.
                let mut users: Cache<u32, Option<String>> = Cache::new();
                let mut groups: Cache<u32, Option<String>> = Cache::new();

                for dir_entry in dir_entries {
                    let dir_entry: DirEntry = match dir_entry {
                        Ok(dir_entry) => dir_entry,
                        Err(error) => {
                            log::warn!("Error for dir entry: {}", error);
                            continue;
                        }
                    };

                    let file_type: Result<FileType, String> = match dir_entry.file_type() {
                        Ok(std_file_type) => Ok(FileType::from(std_file_type)),
                        Err(io_error) => Err(io_error.to_string()),
                    };

                    let metadata: Option<FileMetadata> = match self.metadata {
                        true => Self::metadata(&dir_entry, &file_type, &mut users, &mut groups),
                        false => None,
                    };

                    let file_info: FileInfo = FileInfo::builder()
                        .path(dir_entry.path().to_path_buf())
                        .r#type(file_type)
                        .metadata(metadata)
                        .build();
                    file_infos.push(file_info);
                }

                // Sort the files alphabetically.
                if let Some(options) = self.sort {
                    Self::sort(&mut file_infos, options);
                }

                Ok(file_infos)
            }
            Err(error) => match error.kind() {
                IOErrorKind::NotFound => Err(GetFilesError::DirDoesNotExist),
                IOErrorKind::PermissionDenied => Err(GetFilesError::PermissionDenied),
                _ => Err(GetFilesError::OtherErrorReading(error.to_string())),
            },
        };

        let response_params = ResponseParams::GetFiles(
            GetFilesResponseParams::builder()
                .result(get_files_result)
                .build(),
        );

        self.done = true;

        Some(
            ResponseParamsAndLast::builder()
                .response_params(response_params)
                .last(true)
                .build(),
        )
    }
}

/// Handles a request to find files.
struct FindFiles {
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

        let create_file_result: CreateFileResult = if self.path.exists() {
            Err(CreateFileError::AlreadyExists(self.path.clone()))
        } else {
            match self.file_type {
                FileType::File => {
                    log::info!("Creating file {:?}...", self.path);
                    match File::create(&self.path) {
                        Ok(_) => {
                            log::info!("Created file {:?}.", self.path);
                            Ok(())
                        }
                        Err(io_error) => {
                            log::error!("Error creating file: {}", io_error);
                            Err(CreateFileError::Other(format!("{}", io_error)))
                        }
                    }
                }
                FileType::Dir => {
                    log::info!("Creating directory {:?}...", self.path);
                    match DirBuilder::new().create(&self.path) {
                        Ok(_) => {
                            log::info!("Created directory {:?}.", self.path);
                            Ok(())
                        }
                        Err(io_error) => {
                            log::error!("Error creating directory: {}", io_error);
                            Err(CreateFileError::Other(format!("{}", io_error)))
                        }
                    }
                }
                file_type => Err(CreateFileError::UnsupportedFileType(file_type)),
            }
        };
        let response_params: ResponseParams = ResponseParams::CreateFile(
            CreateFileResponseParams::builder()
                .result(create_file_result)
                .build(),
        );

        self.done = true;

        Some(
            ResponseParamsAndLast::builder()
                .response_params(response_params)
                .last(true)
                .build(),
        )
    }
}

//! Handles requests to get files.
use std::ffi::OsStr;
use std::fs::{self, DirEntry, Metadata, ReadDir};
use std::io::{Error as IOError, ErrorKind as IOErrorKind};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use nix::unistd::{Gid, Group, Uid, User};

use file_info::{FileInfo, FileMetadata};
use file_type::FileType;
use insh_api::{
    FileSortOptions, GetFilesError, GetFilesRequestParams, GetFilesResponseParams, GetFilesResult,
    HiddenFileSort, ResponseParams, ResponseParamsAndLast,
};

use crate::cache::Cache;

/// Handles a request to get files.
pub struct GetFiles {
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

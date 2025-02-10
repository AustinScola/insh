//! Handles a request to get files.
use file_info::FileInfo;
use file_type::FileType;
use insh_api::{
    GetFilesError, GetFilesRequestParams, GetFilesResponseParams, GetFilesResult, ResponseParams,
    ResponseParamsAndLast,
};
use std::fs::{self, DirEntry, ReadDir};
use std::io::{Error as IOError, ErrorKind as IOErrorKind};
use std::path::PathBuf;

/// Handles a request to get files.
pub struct GetFiles {
    /// The directory to get files for.
    dir: PathBuf,
    /// If getting files is done.
    done: bool,
}

impl GetFiles {
    /// Return a new handler for getting files.
    pub fn new(params: &GetFilesRequestParams) -> Self {
        Self {
            dir: params.dir().to_path_buf(),
            done: false,
        }
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

                    let file_info: FileInfo = FileInfo::builder()
                        .path(dir_entry.path().to_path_buf())
                        .r#type(file_type)
                        .build();
                    file_infos.push(file_info);
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

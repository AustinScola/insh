//! Handles requests to get the contents of a file.
use std::fs::{self, Metadata};
use std::io::{Error as IOError, ErrorKind as IOErrorKind};
use std::path::PathBuf;

use insh_api::{
    GetFileContentsError, GetFileContentsRequestParams, GetFileContentsResponseParams,
    GetFileContentsResult, ResponseParams, ResponseParamsAndLast,
};

/// The size in bytes of the largest file which will be read.
const MAX_SIZE: u64 = 10 * 1024 * 1024;

/// Handles a request to get the contents of a file.
pub struct GetFileContents {
    /// The path of the file to get the contents of.
    path: PathBuf,
    /// If getting the contents of the file is done.
    done: bool,
}

impl GetFileContents {
    /// Return a new handler for getting the contents of a file.
    pub fn new(params: &GetFileContentsRequestParams) -> Self {
        Self {
            path: params.path().to_path_buf(),
            done: false,
        }
    }

    /// Return the contents of the file, or why they could not be read.
    fn contents(&self) -> GetFileContentsResult {
        // The size is checked before reading so that a huge file is not read into memory only to
        // be rejected afterwards.
        let metadata: Metadata = fs::metadata(&self.path).map_err(Self::error)?;
        if metadata.is_dir() {
            return Err(GetFileContentsError::IsADir);
        }
        if metadata.len() > MAX_SIZE {
            return Err(GetFileContentsError::TooBig {
                size: metadata.len(),
                max: MAX_SIZE,
            });
        }

        let bytes: Vec<u8> = fs::read(&self.path).map_err(Self::error)?;
        String::from_utf8(bytes).map_err(|_| GetFileContentsError::NotUtf8)
    }

    /// Return why a file could not be read.
    fn error(error: IOError) -> GetFileContentsError {
        match error.kind() {
            IOErrorKind::NotFound => GetFileContentsError::DoesNotExist,
            IOErrorKind::PermissionDenied => GetFileContentsError::PermissionDenied,
            IOErrorKind::IsADirectory => GetFileContentsError::IsADir,
            _ => GetFileContentsError::OtherErrorReading(error.to_string()),
        }
    }
}

impl Iterator for GetFileContents {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let response_params = ResponseParams::GetFileContents(
            GetFileContentsResponseParams::builder()
                .result(self.contents())
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

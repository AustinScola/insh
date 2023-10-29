#![allow(clippy::needless_return)]

use std::ffi::OsStr;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use uuid::Uuid;

use file_info::FileInfo;
use file_type::FileType;
use path_finder::Entry;

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct Request {
    #[builder(default = Uuid::new_v4())]
    uuid: Uuid,
    params: RequestParams,
}

impl Request {
    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    pub fn params(&self) -> &RequestParams {
        &self.params
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RequestParams {
    GetFiles(GetFilesRequestParams),
    FindFiles(FindFilesRequestParams),
    CreateFile(CreateFileRequestParams),
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct GetFilesRequestParams {
    dir: PathBuf,
}

impl GetFilesRequestParams {
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct FindFilesRequestParams {
    dir: PathBuf,
    pattern: String,
}

impl FindFilesRequestParams {
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct CreateFileRequestParams {
    path: PathBuf,
    file_type: FileType,
}

impl CreateFileRequestParams {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_type(&self) -> FileType {
        self.file_type
    }
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct Response {
    uuid: Uuid,
    #[builder(default)]
    last: bool,
    params: ResponseParams,
}

impl Response {
    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    pub fn last(&self) -> bool {
        self.last
    }

    pub fn params(&self) -> &ResponseParams {
        &self.params
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseParams {
    GetFiles(GetFilesResponseParams),
    FindFiles(FindFilesResponseParams),
    CreateFile(CreateFileResponseParams),
}

#[derive(Debug, TypedBuilder)]
pub struct ResponseParamsAndLast {
    pub response_params: ResponseParams,
    pub last: bool,
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct GetFilesResponseParams {
    result: GetFilesResult,
}

impl GetFilesResponseParams {
    pub fn result(&self) -> &GetFilesResult {
        &self.result
    }
}

pub type GetFilesResult = Result<Vec<FileInfo>, GetFilesError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GetFilesError {
    DirDoesNotExist,
    PermissionDenied,
    OtherErrorReading(String),
}

impl Display for GetFilesError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::DirDoesNotExist => write!(formatter, "The directory does not exist."),
            Self::PermissionDenied => write!(formatter, "Permission denied."),
            Self::OtherErrorReading(string) => write!(formatter, "{}", string),
        }
    }
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct FindFilesResponseParams {
    entries: Vec<Entry>,
}

impl FindFilesResponseParams {
    pub fn entries(&self) -> &Vec<Entry> {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        return self.entries.is_empty();
    }
}

pub type CreateFileResult = Result<(), CreateFileError>;

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct CreateFileResponseParams {
    result: CreateFileResult,
}

impl CreateFileResponseParams {
    pub fn result(&self) -> &CreateFileResult {
        &self.result
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CreateFileError {
    AlreadyExists(PathBuf),
    UnsupportedFileType(FileType),
    Other(String),
}

impl Display for CreateFileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::AlreadyExists(filepath) => write!(
                formatter,
                "The file {:?} already exists.",
                filepath.file_name()
            ),
            Self::UnsupportedFileType(file_type) => write!(
                formatter,
                "Create a file of the type {:?} is not supported",
                file_type
            ),

            Self::Other(string) => write!(formatter, "{}", string),
        }
    }
}

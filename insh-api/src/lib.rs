#![allow(clippy::needless_return)]

use std::fmt::{Display, Error as FmtError, Formatter};
use std::path::{Path, PathBuf};
use std::time::Duration;

use file_info::FileInfo;
use file_type::FileType;
use path_finder::Entry;
use phrase_searcher::FileHit;

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use uuid::Uuid;

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
    GetFileContents(GetFileContentsRequestParams),
    FindFiles(FindFilesRequestParams),
    SuggestFindPattern(SuggestFindPatternRequestParams),
    CreateFile(CreateFileRequestParams),
    SearchPhrase(SearchPhraseRequestParams),
    SuggestSearchPhrase(SuggestSearchPhraseRequestParams),
    StreamLogs(StreamLogsRequestParams),
    DatabaseInfo(DatabaseInfoRequestParams),
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct GetFilesRequestParams {
    dir: PathBuf,
    /// How the files should be sorted, or `None` if they should not be sorted.
    #[builder(default)]
    sort: Option<FileSortOptions>,
    /// Whether or not the metadata of the files should be included.
    #[builder(default)]
    metadata: bool,
}

impl GetFilesRequestParams {
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn sort(&self) -> Option<FileSortOptions> {
        self.sort
    }

    /// Return whether or not the metadata of the files should be included.
    pub fn metadata(&self) -> bool {
        self.metadata
    }
}

/// How files should be sorted.
#[derive(Debug, Clone, Copy, TypedBuilder, Serialize, Deserialize)]
pub struct FileSortOptions {
    /// Whether or not the case of filenames should be ignored.
    #[builder(default)]
    case_insensitive: bool,
    /// How hidden files should be sorted.
    #[builder(default)]
    hidden: HiddenFileSort,
}

impl FileSortOptions {
    pub fn case_insensitive(&self) -> bool {
        self.case_insensitive
    }

    pub fn hidden(&self) -> HiddenFileSort {
        self.hidden
    }
}

/// How hidden files should be sorted.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum HiddenFileSort {
    /// Hidden files should be sorted before all of the other files.
    First,
    /// Hidden files should be sorted after all of the other files.
    #[default]
    Last,
    /// Hidden files should be sorted among the other files as if they were not hidden.
    Mixed,
}

/// Request parameters for getting the contents of a file.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct GetFileContentsRequestParams {
    /// The path of the file to get the contents of.
    path: PathBuf,
}

impl GetFileContentsRequestParams {
    /// Return the path of the file to get the contents of.
    pub fn path(&self) -> &Path {
        &self.path
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
pub struct SuggestFindPatternRequestParams {
    partial: String,
}

impl SuggestFindPatternRequestParams {
    pub fn partial(&self) -> &str {
        &self.partial
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
pub struct SearchPhraseRequestParams {
    dir: PathBuf,
    phrase: String,
}

impl SearchPhraseRequestParams {
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn phrase(&self) -> &str {
        &self.phrase
    }
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestSearchPhraseRequestParams {
    partial: String,
}

impl SuggestSearchPhraseRequestParams {
    pub fn partial(&self) -> &str {
        &self.partial
    }
}

/// Request parameters for streaming logs.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct StreamLogsRequestParams {}

/// Request parameters for getting information about the database.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct DatabaseInfoRequestParams {}

#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseParams {
    GetFiles(GetFilesResponseParams),
    GetFileContents(GetFileContentsResponseParams),
    FindFiles(FindFilesResponseParams),
    SuggestFindPattern(SuggestFindPatternResponseParams),
    CreateFile(CreateFileResponseParams),
    SearchPhrase(SearchPhraseResponseParams),
    SuggestSearchPhrase(SuggestSearchPhraseResponseParams),
    StreamLogs(StreamLogsResponseParams),
    DatabaseInfo(DatabaseInfoResponseParams),
}

#[derive(Debug, TypedBuilder)]
pub struct ResponseParamsAndLast {
    pub response_params: ResponseParams,
    pub last: bool,
}

#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
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

/// Response parameters for getting the contents of a file.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct GetFileContentsResponseParams {
    /// The contents of the file, or why they could not be read.
    result: GetFileContentsResult,
}

impl GetFileContentsResponseParams {
    /// Return the contents of the file, or why they could not be read.
    pub fn result(&self) -> &GetFileContentsResult {
        &self.result
    }
}

/// The contents of a file, or why they could not be read.
pub type GetFileContentsResult = Result<String, GetFileContentsError>;

/// Why the contents of a file could not be read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GetFileContentsError {
    /// The file does not exist.
    DoesNotExist,
    /// The file could not be read because permission was denied.
    PermissionDenied,
    /// The file is a directory.
    IsADir,
    /// The file is larger than the most which will be read.
    TooBig {
        /// The size of the file in bytes.
        size: u64,
        /// The size in bytes of the largest file which will be read.
        max: u64,
    },
    /// The contents of the file are not valid UTF-8.
    NotUtf8,
    /// The file could not be read for some other reason.
    OtherErrorReading(String),
}

impl Display for GetFileContentsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::DoesNotExist => write!(formatter, "the file does not exist"),
            Self::PermissionDenied => write!(formatter, "permission denied"),
            Self::IsADir => write!(formatter, "the file is a directory"),
            Self::TooBig { size, max } => write!(
                formatter,
                "the file is {} bytes which is larger than the maximum of {} bytes",
                size, max
            ),
            Self::NotUtf8 => write!(formatter, "the file is not valid UTF-8"),
            Self::OtherErrorReading(string) => write!(formatter, "{}", string),
        }
    }
}

#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct FindFilesResponseParams {
    entries: Vec<Entry>,
    /// The number of files which have been searched so far.
    searched: usize,
    /// The duration of the search so far.
    duration: Duration,
}

impl FindFilesResponseParams {
    pub fn entries(&self) -> &Vec<Entry> {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        return self.entries.is_empty();
    }

    /// Return the number of files which have been searched so far.
    pub fn searched(&self) -> usize {
        self.searched
    }

    /// Return the duration of the search so far.
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestFindPatternResponseParams {
    suggestion: Option<String>,
}

impl SuggestFindPatternResponseParams {
    pub fn suggestion(&self) -> &Option<String> {
        &self.suggestion
    }
}

pub type CreateFileResult = Result<(), CreateFileError>;

#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct CreateFileResponseParams {
    result: CreateFileResult,
}

impl CreateFileResponseParams {
    pub fn result(&self) -> &CreateFileResult {
        &self.result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
                "The file {} already exists.",
                filepath.file_name().unwrap().to_string_lossy()
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

#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SearchPhraseResponseParams {
    hits: Vec<FileHit>,
    /// The number of files which have been searched so far.
    searched: usize,
    /// The duration of the search so far.
    duration: Duration,
}

impl SearchPhraseResponseParams {
    pub fn hits(&self) -> &Vec<FileHit> {
        &self.hits
    }

    pub fn is_empty(&self) -> bool {
        return self.hits.is_empty();
    }

    /// Return the number of files which have been searched so far.
    pub fn searched(&self) -> usize {
        self.searched
    }

    /// Return the duration of the search so far.
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestSearchPhraseResponseParams {
    suggestion: Option<String>,
}

impl SuggestSearchPhraseResponseParams {
    pub fn suggestion(&self) -> &Option<String> {
        &self.suggestion
    }
}

/// Response parameters for streaming logs.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct StreamLogsResponseParams {
    records: Vec<LogRecord>,
}

impl StreamLogsResponseParams {
    pub fn records(&self) -> &Vec<LogRecord> {
        &self.records
    }
}

/// Response parameters for getting information about the database.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct DatabaseInfoResponseParams {
    /// The version of the database server which is running.
    version: String,
}

impl DatabaseInfoResponseParams {
    /// Return the version of the database server which is running.
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// A log record.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct LogRecord {
    /// When the record was emitted.
    timestamp: String,
    /// The severity of the record.
    level: LogLevel,
    /// The module which emitted the record.
    module: String,
    /// The thread which emitted the record.
    thread: String,
    /// The message of the record.
    message: String,
}

impl LogRecord {
    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }

    pub fn level(&self) -> LogLevel {
        self.level
    }

    pub fn module(&self) -> &str {
        &self.module
    }

    pub fn thread(&self) -> &str {
        &self.thread
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for LogRecord {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        write!(
            formatter,
            "{} {} [{}] [{}] {}",
            self.timestamp, self.level, self.module, self.thread, self.message
        )
    }
}

/// The severity of a log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    /// A very serious error.
    Error,
    /// A hazardous situation.
    Warn,
    /// Useful information.
    Info,
    /// Information which is useful for debugging.
    Debug,
    /// Very low priority information.
    Trace,
}

impl Display for LogLevel {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        let string: &str = match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        };
        write!(formatter, "{}", string)
    }
}

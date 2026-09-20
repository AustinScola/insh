/*!
The requests and responses which insh and inshd send each other.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
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

/// A request.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct Request {
    /// The unique identifier of the request.
    #[builder(default = Uuid::new_v4())]
    uuid: Uuid,
    /// The request parameters.
    params: RequestParams,
}

impl Request {
    /// Return the unique identifier of the request.
    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    /// Return the request parameters.
    pub fn params(&self) -> &RequestParams {
        &self.params
    }
}

/// Request parameters.
#[derive(Debug, Serialize, Deserialize)]
pub enum RequestParams {
    /// Get the files in a directory.
    GetFiles(GetFilesRequestParams),
    /// Suggest a directory.
    SuggestDir(SuggestDirRequestParams),
    /// Take note that a directory was gone to.
    VisitDir(VisitDirRequestParams),
    /// Get the contents of a file.
    GetFileContents(GetFileContentsRequestParams),
    /// Find files.
    FindFiles(FindFilesRequestParams),
    /// Suggest a find pattern.
    SuggestFindPattern(SuggestFindPatternRequestParams),
    /// Create a file.
    CreateFile(CreateFileRequestParams),
    /// Search for a phrase.
    SearchPhrase(SearchPhraseRequestParams),
    /// Suggest a search phrase.
    SuggestSearchPhrase(SuggestSearchPhraseRequestParams),
    /// Stream the logs.
    StreamLogs(StreamLogsRequestParams),
    /// Get information about the database.
    DatabaseInfo(DatabaseInfoRequestParams),
    /// Find out whether an AI inference engine is configured.
    AiStatus(AiStatusRequestParams),
    /// Say something in a chat and get the reply.
    Chat(ChatRequestParams),
    /// List the chats.
    ListChats(ListChatsRequestParams),
    /// Search the chats.
    SearchChats(SearchChatsRequestParams),
    /// Get the messages of a chat.
    GetChat(GetChatRequestParams),
    /// Delete a chat.
    DeleteChat(DeleteChatRequestParams),
}

/// Request parameters for getting the files in a directory.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct GetFilesRequestParams {
    /// The directory to get the files in.
    dir: PathBuf,
    /// How the files should be sorted.
    #[builder(default)]
    sort: Option<FileSortOptions>,
    /// Whether the metadata of the files should be included.
    #[builder(default)]
    metadata: bool,
}

impl GetFilesRequestParams {
    /// Return the directory to get the files in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Return how the files should be sorted.
    pub fn sort(&self) -> Option<FileSortOptions> {
        self.sort
    }

    /// Return whether the metadata of the files should be included.
    pub fn metadata(&self) -> bool {
        self.metadata
    }
}

/// Request parameters for suggesting a directory.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestDirRequestParams {
    /// The path so far.
    partial: String,
}

impl SuggestDirRequestParams {
    /// Return the path so far.
    pub fn partial(&self) -> &str {
        &self.partial
    }
}

/// Request parameters for taking note that a directory was gone to.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct VisitDirRequestParams {
    /// The directory which was gone to.
    dir: PathBuf,
}

impl VisitDirRequestParams {
    /// Return the directory which was gone to.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// How files should be sorted.
#[derive(Debug, Clone, Copy, TypedBuilder, Serialize, Deserialize)]
pub struct FileSortOptions {
    /// Whether the case of filenames should be ignored.
    #[builder(default)]
    case_insensitive: bool,
    /// How hidden files should be sorted.
    #[builder(default)]
    hidden: HiddenFileSort,
}

impl FileSortOptions {
    /// Return whether the case of filenames should be ignored.
    pub fn case_insensitive(&self) -> bool {
        self.case_insensitive
    }

    /// Return how hidden files should be sorted.
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
    /// Hidden files should be sorted as if they were not hidden.
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

/// Request parameters for finding files.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct FindFilesRequestParams {
    /// The directory to look in.
    dir: PathBuf,
    /// The pattern to match file names against.
    pattern: String,
}

impl FindFilesRequestParams {
    /// Return the directory to look in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Return the pattern to match file names against.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

/// Request parameters for suggesting a find pattern.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestFindPatternRequestParams {
    /// The pattern so far.
    partial: String,
}

impl SuggestFindPatternRequestParams {
    /// Return the pattern so far.
    pub fn partial(&self) -> &str {
        &self.partial
    }
}

/// Request parameters for creating a file.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct CreateFileRequestParams {
    /// The path of the file to create.
    path: PathBuf,
    /// The type of file to create.
    file_type: FileType,
}

impl CreateFileRequestParams {
    /// Return the path of the file to create.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Return the type of file to create.
    pub fn file_type(&self) -> FileType {
        self.file_type
    }
}

/// Request parameters for searching for a phrase.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SearchPhraseRequestParams {
    /// The directory to search in.
    dir: PathBuf,
    /// The phrase to search for.
    phrase: String,
}

impl SearchPhraseRequestParams {
    /// Return the directory to search in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Return the phrase to search for.
    pub fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// Request parameters for suggesting a search phrase.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestSearchPhraseRequestParams {
    /// The phrase so far.
    partial: String,
}

impl SuggestSearchPhraseRequestParams {
    /// Return the phrase so far.
    pub fn partial(&self) -> &str {
        &self.partial
    }
}

/// Request parameters for streaming logs.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct StreamLogsRequestParams {}

/// A chat.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct Chat {
    /// Which chat it is.
    id: i64,
    /// What the chat is called.
    #[builder(setter(into))]
    title: String,
    /// The directory the chat was started in.
    #[builder(setter(into))]
    dir: PathBuf,
}

impl Chat {
    /// Return which chat it is.
    pub fn id(&self) -> i64 {
        self.id
    }

    /// Return what the chat is called.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Return the directory the chat was started in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Who sent a message.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChatRole {
    /// The person using insh.
    User,
    /// The inference engine.
    Assistant,
}

/// A message of a chat.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Who sent the message.
    role: ChatRole,
    /// What the message says.
    #[builder(setter(into))]
    content: String,
}

impl ChatMessage {
    /// Return who sent the message.
    pub fn role(&self) -> ChatRole {
        self.role
    }

    /// Return what the message says.
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// A message which a search found, along with the chat it is in.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct ChatHit {
    /// The chat the message is in.
    chat: Chat,
    /// What the message says.
    #[builder(setter(into))]
    content: String,
}

impl ChatHit {
    /// Return the chat the message is in.
    pub fn chat(&self) -> &Chat {
        &self.chat
    }

    /// Return what the message says.
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// How to search the chats.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChatSearchMode {
    /// By the words which were used.
    Keyword,
    /// By what was meant.
    Semantic,
    /// By both.
    #[default]
    Both,
}

/// Request parameters for finding out whether an AI inference engine is configured.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct AiStatusRequestParams {}

/// Request parameters for saying something in a chat.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct ChatRequestParams {
    /// Which chat to say it in, or nothing to start one.
    #[builder(default)]
    chat_id: Option<i64>,
    /// The directory the chat is about.
    dir: PathBuf,
    /// What to say.
    #[builder(setter(into))]
    message: String,
}

impl ChatRequestParams {
    /// Return which chat to say it in, or nothing to start one.
    pub fn chat_id(&self) -> Option<i64> {
        self.chat_id
    }

    /// Return the directory the chat is about.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Return what to say.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Request parameters for listing the chats.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct ListChatsRequestParams {
    /// The most chats to list.
    limit: usize,
}

impl ListChatsRequestParams {
    /// Return the most chats to list.
    pub fn limit(&self) -> usize {
        self.limit
    }
}

/// Request parameters for searching the chats.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SearchChatsRequestParams {
    /// What to search for.
    #[builder(setter(into))]
    text: String,
    /// How to search.
    #[builder(default)]
    mode: ChatSearchMode,
    /// The most chats to return.
    limit: usize,
}

impl SearchChatsRequestParams {
    /// Return what to search for.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Return how to search.
    pub fn mode(&self) -> ChatSearchMode {
        self.mode
    }

    /// Return the most chats to return.
    pub fn limit(&self) -> usize {
        self.limit
    }
}

/// Request parameters for getting the messages of a chat.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct GetChatRequestParams {
    /// Which chat to get the messages of.
    chat_id: i64,
}

impl GetChatRequestParams {
    /// Return which chat to get the messages of.
    pub fn chat_id(&self) -> i64 {
        self.chat_id
    }
}

/// Request parameters for deleting a chat.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct DeleteChatRequestParams {
    /// Which chat to delete.
    chat_id: i64,
}

impl DeleteChatRequestParams {
    /// Return which chat to delete.
    pub fn chat_id(&self) -> i64 {
        self.chat_id
    }
}

/// Request parameters for getting information about the database.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct DatabaseInfoRequestParams {}

/// A response.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct Response {
    /// The unique identifier of the request.
    uuid: Uuid,
    /// Whether this is the last response.
    #[builder(default)]
    last: bool,
    /// The response parameters.
    params: ResponseParams,
}

impl Response {
    /// Return the unique identifier of the request.
    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    /// Return whether this is the last response.
    pub fn last(&self) -> bool {
        self.last
    }

    /// Return the response parameters.
    pub fn params(&self) -> &ResponseParams {
        &self.params
    }
}

/// Response parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseParams {
    /// The files in a directory.
    GetFiles(GetFilesResponseParams),
    /// A suggested directory.
    SuggestDir(SuggestDirResponseParams),
    /// A directory was taken note of.
    VisitDir(VisitDirResponseParams),
    /// The contents of a file.
    GetFileContents(GetFileContentsResponseParams),
    /// The files found.
    FindFiles(FindFilesResponseParams),
    /// A suggested find pattern.
    SuggestFindPattern(SuggestFindPatternResponseParams),
    /// Whether the file was created.
    CreateFile(CreateFileResponseParams),
    /// The hits found.
    SearchPhrase(SearchPhraseResponseParams),
    /// A suggested search phrase.
    SuggestSearchPhrase(SuggestSearchPhraseResponseParams),
    /// Some log records.
    StreamLogs(StreamLogsResponseParams),
    /// Information about the database.
    DatabaseInfo(DatabaseInfoResponseParams),
    /// Whether an AI inference engine is configured.
    AiStatus(AiStatusResponseParams),
    /// A piece of a reply.
    Chat(ChatResponseParams),
    /// The chats.
    ListChats(ListChatsResponseParams),
    /// The chats which were found.
    SearchChats(SearchChatsResponseParams),
    /// The messages of a chat.
    GetChat(GetChatResponseParams),
    /// Whether the chat was deleted.
    DeleteChat(DeleteChatResponseParams),
}

/// Response parameters and whether they are the last.
#[derive(Debug, TypedBuilder)]
pub struct ResponseParamsAndLast {
    /// The response parameters.
    pub response_params: ResponseParams,
    /// Whether this is the last response.
    pub last: bool,
}

/// Response parameters for getting the files in a directory.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct GetFilesResponseParams {
    /// The result.
    result: GetFilesResult,
}

impl GetFilesResponseParams {
    /// Return the result.
    pub fn result(&self) -> &GetFilesResult {
        &self.result
    }
}

/// A get files result.
pub type GetFilesResult = Result<Vec<FileInfo>, GetFilesError>;

/// A get files error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GetFilesError {
    /// The directory does not exist.
    DirDoesNotExist,
    /// Permission was denied.
    PermissionDenied,
    /// Something else went wrong.
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

/// Response parameters for suggesting a directory.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestDirResponseParams {
    /// The suggested path.
    suggestion: Option<String>,
}

impl SuggestDirResponseParams {
    /// Return the suggested path.
    pub fn suggestion(&self) -> &Option<String> {
        &self.suggestion
    }
}

/// Response parameters for taking note that a directory was gone to. There is nothing to say about
/// it, but a response is what says that the request was handled.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct VisitDirResponseParams {}

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

/// Response parameters for finding files.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct FindFilesResponseParams {
    /// The files found.
    entries: Vec<Entry>,
    /// The number of files which have been searched so far.
    searched: usize,
    /// The duration of the search so far.
    duration: Duration,
}

impl FindFilesResponseParams {
    /// Return the files found.
    pub fn entries(&self) -> &Vec<Entry> {
        &self.entries
    }

    /// Return whether no files were found.
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

/// Response parameters for suggesting a find pattern.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestFindPatternResponseParams {
    /// The suggested pattern.
    suggestion: Option<String>,
}

impl SuggestFindPatternResponseParams {
    /// Return the suggested pattern.
    pub fn suggestion(&self) -> &Option<String> {
        &self.suggestion
    }
}

/// A file creation result.
pub type CreateFileResult = Result<(), CreateFileError>;

/// Response parameters for creating a file.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct CreateFileResponseParams {
    /// The result.
    result: CreateFileResult,
}

impl CreateFileResponseParams {
    /// Return the result.
    pub fn result(&self) -> &CreateFileResult {
        &self.result
    }
}

/// A file creation error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CreateFileError {
    /// The file already exists.
    AlreadyExists(PathBuf),
    /// Files of that type cannot be created.
    UnsupportedFileType(FileType),
    /// Something else went wrong.
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

/// Response parameters for searching for a phrase.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SearchPhraseResponseParams {
    /// The hits found.
    hits: Vec<FileHit>,
    /// The number of files which have been searched so far.
    searched: usize,
    /// The duration of the search so far.
    duration: Duration,
}

impl SearchPhraseResponseParams {
    /// Return the hits found.
    pub fn hits(&self) -> &Vec<FileHit> {
        &self.hits
    }

    /// Return whether no hits were found.
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

/// Response parameters for suggesting a search phrase.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestSearchPhraseResponseParams {
    /// The suggested phrase.
    suggestion: Option<String>,
}

impl SuggestSearchPhraseResponseParams {
    /// Return the suggested phrase.
    pub fn suggestion(&self) -> &Option<String> {
        &self.suggestion
    }
}

/// Response parameters for streaming logs.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct StreamLogsResponseParams {
    /// The log records.
    records: Vec<LogRecord>,
}

impl StreamLogsResponseParams {
    /// Return the log records.
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
    /// Return when the record was emitted.
    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }

    /// Return the severity of the record.
    pub fn level(&self) -> LogLevel {
        self.level
    }

    /// Return the module which emitted the record.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// Return the thread which emitted the record.
    pub fn thread(&self) -> &str {
        &self.thread
    }

    /// Return the message of the record.
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

/// Response parameters for whether an AI inference engine is configured.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct AiStatusResponseParams {
    /// Whether an AI inference engine is configured.
    configured: bool,
}

impl AiStatusResponseParams {
    /// Return whether an AI inference engine is configured.
    pub fn configured(&self) -> bool {
        self.configured
    }
}

/// Response parameters for a piece of a reply.
///
/// A reply arrives over many of these. Each carries the text which came since the one before it
/// rather than the reply so far, so the pieces are appended as they arrive.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct ChatResponseParams {
    /// Which chat the reply is in, once there is one.
    ///
    /// This is how the chat which a request started learns which one it is. It is nothing when
    /// the chat could not be started at all, so that a failure is not taken for a chat.
    #[builder(default)]
    chat_id: Option<i64>,
    /// The text which came since the last piece.
    #[builder(default, setter(into))]
    delta: String,
    /// How many tokens the reply came to, once the inference engine has said.
    ///
    /// Engines say this near the end of a reply rather than as it goes, and some do not say at
    /// all, so it is not there for every piece.
    #[builder(default)]
    tokens: Option<usize>,
    /// Whether the inference engine is thinking rather than answering.
    #[builder(default)]
    thinking: bool,
    /// How many of those tokens were the engine thinking rather than answering.
    ///
    /// A model which thinks before it answers is billed for the thinking as well, which is why a
    /// one word answer can come to far more tokens than the words in it.
    #[builder(default)]
    thinking_tokens: Option<usize>,
    /// How long the reply has been coming.
    #[builder(default)]
    duration: Duration,
    /// What went wrong, if anything did.
    #[builder(default)]
    error: Option<String>,
}

impl ChatResponseParams {
    /// Return which chat the reply is in, once there is one.
    pub fn chat_id(&self) -> Option<i64> {
        self.chat_id
    }

    /// Return the text which came since the last piece.
    pub fn delta(&self) -> &str {
        &self.delta
    }

    /// Return how many tokens the reply came to, once the inference engine has said.
    pub fn tokens(&self) -> Option<usize> {
        self.tokens
    }

    /// Return whether the inference engine is thinking rather than answering.
    pub fn thinking(&self) -> bool {
        self.thinking
    }

    /// Return how many of those tokens were the engine thinking rather than answering.
    pub fn thinking_tokens(&self) -> Option<usize> {
        self.thinking_tokens
    }

    /// Return how long the reply has been coming.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Return what went wrong, if anything did.
    pub fn error(&self) -> Option<&String> {
        self.error.as_ref()
    }
}

/// Response parameters for the chats.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct ListChatsResponseParams {
    /// The chats, the ones used most recently first.
    chats: Vec<Chat>,
}

impl ListChatsResponseParams {
    /// Return the chats.
    pub fn chats(&self) -> &Vec<Chat> {
        &self.chats
    }
}

/// Response parameters for the chats which were found.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SearchChatsResponseParams {
    /// The chats which were found by the words which were used.
    #[builder(default)]
    chats: Vec<Chat>,
    /// The messages which were found by what they meant.
    #[builder(default)]
    hits: Vec<ChatHit>,
}

impl SearchChatsResponseParams {
    /// Return the chats which were found by the words which were used.
    pub fn chats(&self) -> &Vec<Chat> {
        &self.chats
    }

    /// Return the messages which were found by what they meant.
    pub fn hits(&self) -> &Vec<ChatHit> {
        &self.hits
    }
}

/// Response parameters for the messages of a chat.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct GetChatResponseParams {
    /// The messages, the oldest first.
    messages: Vec<ChatMessage>,
    /// What went wrong reading the chat, if anything did.
    ///
    /// A chat which could not be read has no messages, which is the same as one which has nothing
    /// said in it, so this is what tells the two apart.
    #[builder(default)]
    error: Option<String>,
}

impl GetChatResponseParams {
    /// Return the messages.
    pub fn messages(&self) -> &Vec<ChatMessage> {
        &self.messages
    }

    /// Return what went wrong reading the chat, if anything did.
    pub fn error(&self) -> Option<&String> {
        self.error.as_ref()
    }
}

/// Response parameters for whether a chat was deleted.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct DeleteChatResponseParams {
    /// Whether the chat was deleted.
    deleted: bool,
}

impl DeleteChatResponseParams {
    /// Return whether the chat was deleted.
    pub fn deleted(&self) -> bool {
        self.deleted
    }
}

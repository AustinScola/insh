/*!
Information about a file.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use file_type::FileType;

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

/// Information about a file.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct FileInfo {
    /// The path of the file.
    path: PathBuf,

    /// The type of the file.
    r#type: Result<FileType, String>,

    /// The metadata of the file.
    #[builder(default)]
    metadata: Option<FileMetadata>,
}

impl FileInfo {
    /// Return the path of the file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Return the name of the file.
    pub fn name(&self) -> Option<&OsStr> {
        self.path.file_name()
    }

    /// Return the type of the file.
    pub fn r#type(&self) -> &Result<FileType, String> {
        &self.r#type
    }

    /// Return the metadata of the file.
    pub fn metadata(&self) -> Option<&FileMetadata> {
        self.metadata.as_ref()
    }
}

/// The metadata of a file.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct FileMetadata {
    /// The mode of the file (including the bits which indicate the type of the file).
    mode: u32,

    /// The number of hard links to the file.
    hard_links: u64,

    /// The id of the user which owns the file.
    uid: u32,

    /// The name of the user which owns the file.
    #[builder(default)]
    user: Option<String>,

    /// The id of the group which owns the file.
    gid: u32,

    /// The name of the group which owns the file.
    #[builder(default)]
    group: Option<String>,

    /// The size of the file in bytes.
    size: u64,

    /// When the file was last modified in seconds since the Unix epoch.
    modified: i64,

    /// The target of the file if it is a symlink.
    #[builder(default)]
    link_target: Option<PathBuf>,
}

impl FileMetadata {
    /// Return the mode of the file (including the bits which indicate the type of the file).
    pub fn mode(&self) -> u32 {
        self.mode
    }

    /// Return the number of hard links to the file.
    pub fn hard_links(&self) -> u64 {
        self.hard_links
    }

    /// Return the id of the user which owns the file.
    pub fn uid(&self) -> u32 {
        self.uid
    }

    /// Return the name of the user which owns the file.
    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    /// Return the id of the group which owns the file.
    pub fn gid(&self) -> u32 {
        self.gid
    }

    /// Return the name of the group which owns the file.
    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    /// Return the size of the file in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Return when the file was last modified in seconds since the Unix epoch.
    pub fn modified(&self) -> i64 {
        self.modified
    }

    /// Return the target of the file if it is a symlink.
    pub fn link_target(&self) -> Option<&Path> {
        self.link_target.as_deref()
    }
}

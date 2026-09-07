/*!
The type of a file.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]

use std::fs::FileType as StdFileType;

use serde::{Deserialize, Serialize};

/// The type of a file.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileType {
    /// A regular file.
    File,
    /// A directory.
    Dir,
    /// A symbolic link.
    Symlink,
    /// Anything else.
    Other,
}

impl From<StdFileType> for FileType {
    fn from(std_file_type: StdFileType) -> Self {
        if std_file_type.is_file() {
            return Self::File;
        }
        if std_file_type.is_dir() {
            return Self::Dir;
        }
        if std_file_type.is_symlink() {
            return Self::Symlink;
        }
        Self::Other
    }
}

impl FileType {
    /// Return whether the file is a directory.
    pub fn is_dir(&self) -> bool {
        self == &Self::Dir
    }
}

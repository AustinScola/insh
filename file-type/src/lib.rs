use serde::{Deserialize, Serialize};
use std::fs::FileType as StdFileType;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileType {
    File,
    Dir,
    Symlink,
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
    pub fn is_dir(&self) -> bool {
        self == &Self::Dir
    }
}

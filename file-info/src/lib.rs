use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

use file_type::FileType;

#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct FileInfo {
    path: PathBuf,
    r#type: Result<FileType, String>,
}

impl FileInfo {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn name(&self) -> Option<&OsStr> {
        self.path.file_name()
    }

    pub fn r#type(&self) -> &Result<FileType, String> {
        &self.r#type
    }
}

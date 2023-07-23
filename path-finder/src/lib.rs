/*!
The module contains the [`PathFinder`] struct which is used to find files with file names matching a
pattern.
*/
use std::ffi::OsStr;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::path::{Path, PathBuf};

use regex::Error as RegexError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::{
    DirEntry as WalkdirEntry, Error as WalkerEntryError, IntoIter as Walker,
    WalkDir as WalkerBuilder,
};

/// Used to find files with file names matching a pattern.
pub struct PathFinder {
    /// The pattern to match file names against.
    regex: Regex,
    /// An iterator over the files in a given directory (recursive).
    walker: Walker,
}

impl PathFinder {
    /// Return a new path finder that can be used to find the files in the given `directory` with
    /// file names that match the regex `pattern`.
    pub fn new(directory: &Path, pattern: &str) -> Result<Self, NewPathFinderError> {
        let regex: Regex = match Regex::new(pattern) {
            Ok(regex) => regex,
            Err(error) => return Err(NewPathFinderError::RegexError(error)),
        };
        let walker = WalkerBuilder::new(directory).min_depth(1).into_iter();

        Ok(PathFinder { regex, walker })
    }
}

pub enum NewPathFinderError {
    RegexError(RegexError),
}

impl Display for NewPathFinderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::RegexError(error) => {
                write!(formatter, "Regex error: {}", error)
            }
        }
    }
}

impl Iterator for PathFinder {
    type Item = Entry;

    fn next(&mut self) -> Option<Entry> {
        loop {
            let entry: Option<Result<WalkdirEntry, WalkerEntryError>> = self.walker.next();

            match entry {
                None => {
                    return None;
                }
                Some(entry) => match entry {
                    Err(_) => continue,
                    Ok(entry) => {
                        if entry.path().is_dir() {
                            continue;
                        }

                        if self.regex.is_match(&entry.file_name().to_string_lossy()) {
                            return Some(entry.into());
                        }
                        continue;
                    }
                },
            }
        }
    }
}

impl From<Walker> for PathFinder {
    fn from(walker: Walker) -> Self {
        PathFinder {
            regex: Regex::new(".*").unwrap(),
            walker,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Entry {
    path: PathBuf,
}

impl From<WalkdirEntry> for Entry {
    fn from(walkdir_entry: WalkdirEntry) -> Self {
        Self {
            path: walkdir_entry.path().to_path_buf(),
        }
    }
}

impl Entry {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_name(&self) -> Option<&OsStr> {
        self.path.file_name()
    }
}

/*!
The module contains the [`PathFinder`] struct which is used to find files with file names matching a
pattern.
*/
use std::ffi::OsStr;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::path::{Path, PathBuf};

use ignore::{DirEntry as WalkdirEntry, Error as WalkEntryError, Walk};
use regex::Error as RegexError;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Used to find files with file names matching a pattern.
pub struct PathFinder {
    /// The pattern to match file names against.
    regex: Regex,
    /// An iterator over the files in a given directory (recursive).
    walk: Walk,
}

impl PathFinder {
    /// Return a new path finder that can be used to find the files in the given `directory` with
    /// file names that match the regex `pattern`.
    pub fn new(directory: &Path, pattern: &str) -> Result<Self, NewPathFinderError> {
        let regex: Regex = match Regex::new(pattern) {
            Ok(regex) => regex,
            Err(error) => return Err(NewPathFinderError::RegexError(error)),
        };
        let walk = Walk::new(directory);

        Ok(PathFinder { regex, walk })
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

/// A file which the path finder examined.
///
/// Every file which is examined is reported (not just the ones which match) so that the progress of
/// finding files can be followed.
pub enum Examined {
    /// The name of the file matched the pattern.
    Matched(Entry),
    /// The name of the file did not match the pattern.
    NotMatched,
}

impl Iterator for PathFinder {
    type Item = Examined;

    fn next(&mut self) -> Option<Examined> {
        loop {
            let entry: Option<Result<WalkdirEntry, WalkEntryError>> = self.walk.next();

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
                            return Some(Examined::Matched(entry.into()));
                        }
                        return Some(Examined::NotMatched);
                    }
                },
            }
        }
    }
}

impl From<Walk> for PathFinder {
    fn from(walk: Walk) -> Self {
        PathFinder {
            regex: Regex::new(".*").unwrap(),
            walk,
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

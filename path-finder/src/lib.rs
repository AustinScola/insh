/*!
Finds the files in a directory with file names matching a pattern.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]

use std::ffi::OsStr;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::path::{Path, PathBuf};

use ignore::{DirEntry as WalkdirEntry, Error as WalkEntryError, Walk, WalkBuilder};
use regex::Error as RegexError;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Finds files by name.
pub struct PathFinder {
    /// The pattern to match file names against.
    regex: Regex,
    /// An iterator over the files in a given directory (recursive).
    walk: Walk,
}

impl PathFinder {
    /// The directory a repository keeps its history in.
    const GIT_DIR: &'static str = ".git";

    /// Return a new path finder for the files in `directory` with names matching `pattern`.
    ///
    /// Hidden files are found along with the rest, since a file like `.envrc` is one which is
    /// looked for by name as much as any other. The history of a repository is not, even though
    /// nothing ignores it: it is hidden only so that it is out of the way, and it is full of files
    /// whose names would bury whatever was being looked for.
    pub fn new(directory: &Path, pattern: &str) -> Result<Self, NewPathFinderError> {
        let regex: Regex = match Regex::new(pattern) {
            Ok(regex) => regex,
            Err(error) => return Err(NewPathFinderError::RegexError(error)),
        };
        let walk: Walk = WalkBuilder::new(directory)
            .hidden(false)
            .filter_entry(|entry| entry.file_name() != Self::GIT_DIR)
            .build();

        Ok(PathFinder { regex, walk })
    }
}

/// A new path finder error.
pub enum NewPathFinderError {
    /// The pattern is not a valid regex.
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

/// A file which was found.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Entry {
    /// The path of the file.
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
    /// Return the path of the file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Return the name of the file.
    pub fn file_name(&self) -> Option<&OsStr> {
        self.path.file_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    #[test]
    fn test_hidden_files_are_found_but_not_the_history_of_a_repository() {
        let dir: PathBuf = std::env::temp_dir().join(format!("path-finder-{}", std::process::id()));
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join(".envrc"), "").unwrap();
        fs::write(dir.join(".git").join("rc"), "").unwrap();

        let found: Vec<PathBuf> = PathFinder::new(&dir, "rc")
            .ok()
            .unwrap()
            .filter_map(|examined| match examined {
                Examined::Matched(entry) => Some(entry.path().to_path_buf()),
                Examined::NotMatched => None,
            })
            .collect();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(found, vec![dir.join(".envrc")]);
    }
}

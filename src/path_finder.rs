use regex::Regex;
use std::path::Path;

use walkdir::{
    DirEntry as Entry, Error as WalkerEntryError, IntoIter as Walker, WalkDir as WalkerBuilder,
};

pub struct PathFinder {
    regex: Regex,
    walker: Walker,
}

impl PathFinder {
    pub fn new(directory: &Path, pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        let walker = WalkerBuilder::new(directory).min_depth(1).into_iter();

        Ok(PathFinder { regex, walker })
    }
}

impl Iterator for PathFinder {
    type Item = Entry;

    fn next(&mut self) -> Option<Entry> {
        loop {
            let entry: Option<Result<Entry, WalkerEntryError>> = self.walker.next();

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
                            return Some(entry);
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

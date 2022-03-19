use regex::Regex;
use std::fs;
use std::path::Path;

use crate::walker::Walker;

pub struct PathFinder {
    regex: Regex,
    walker: Walker,
}

impl PathFinder {
    pub fn new(directory: &Path, pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        let walker = Walker::from(directory);

        Ok(PathFinder { regex, walker })
    }
}

impl Iterator for PathFinder {
    type Item = fs::DirEntry;

    fn next(&mut self) -> Option<fs::DirEntry> {
        let mut next_entry = self.walker.next();
        loop {
            match next_entry {
                Some(entry) => {
                    if self.regex.is_match(&entry.file_name().to_string_lossy()) {
                        return Some(entry);
                    }
                    next_entry = self.walker.next();
                }
                None => {
                    return None;
                }
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

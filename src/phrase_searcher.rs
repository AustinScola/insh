use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use walkdir::{
    DirEntry as Entry, Error as WalkerEntryError, IntoIter as Walker, WalkDir as WalkerBuilder,
};

pub struct PhraseSearcher {
    phrase: String,
    walker: Walker,
}

impl PhraseSearcher {
    pub fn new(directory: &Path, phrase: &str) -> Self {
        let phrase: String = phrase.to_string();
        let walker: Walker = WalkerBuilder::new(directory).min_depth(1).into_iter();
        Self { phrase, walker }
    }
}

impl Iterator for PhraseSearcher {
    type Item = FileHit;

    fn next(&mut self) -> Option<FileHit> {
        loop {
            let entry: Option<Result<Entry, WalkerEntryError>> = self.walker.next();

            match entry {
                None => {
                    return None;
                }
                Some(entry) => match entry {
                    Err(_) => continue,
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_dir() {
                            continue;
                        }

                        let file = File::open(path).unwrap();
                        let reader = BufReader::new(file);

                        let mut failed_to_read_line: bool = false;
                        let mut line_hits: Vec<LineHit> = Vec::new();
                        for (line, line_number) in reader.lines().zip(1..) {
                            if line.is_err() {
                                failed_to_read_line = true;
                                break;
                            }
                            let line = line.unwrap();

                            if line.contains(&self.phrase) {
                                let line_hit = LineHit::new(line_number, &line);
                                line_hits.push(line_hit)
                            }
                        }

                        if failed_to_read_line {
                            continue;
                        }

                        if !line_hits.is_empty() {
                            let file_hit = FileHit::new(path, line_hits);
                            return Some(file_hit);
                        }

                        continue;
                    }
                },
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct FileHit {
    path: PathBuf,
    line_hits: Vec<LineHit>,
}

impl FileHit {
    pub fn new(path: &Path, line_hits: Vec<LineHit>) -> Self {
        let path: PathBuf = path.to_path_buf();
        Self { path, line_hits }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn line_hits(&self) -> &Vec<LineHit> {
        &self.line_hits
    }
}

#[derive(Debug, PartialEq)]
pub struct LineHit {
    line_number: usize,
    line: String,
}

impl LineHit {
    pub fn new(line_number: usize, line: &str) -> Self {
        Self {
            line_number,
            line: line.to_string(),
        }
    }

    pub fn line_number(&self) -> usize {
        self.line_number
    }

    pub fn line(&self) -> &str {
        &self.line
    }
}

use crate::walker::Walker;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub struct Searcher {
    search: String,
    walker: Walker,
}

impl Searcher {
    pub fn new(directory: &Path, search: &str) -> Self {
        Searcher {
            search: search.to_string(),
            walker: Walker::from(directory),
        }
    }
}

impl Iterator for Searcher {
    type Item = SearchFileHit;

    fn next(&mut self) -> Option<SearchFileHit> {
        loop {
            let entry = self.walker.next();
            match entry {
                Some(ref entry) => {
                    let file_path = entry.path();
                    let file_path = file_path.as_os_str();

                    let file = File::open(&*file_path).unwrap();
                    let reader = BufReader::new(file);

                    let mut hits = Vec::new();

                    let mut failed_to_read_line = false;
                    for (line_number, line) in reader.lines().enumerate() {
                        if line.is_err() {
                            failed_to_read_line = true;
                            continue;
                        }

                        let line = line.unwrap();

                        match line.find(&self.search) {
                            Some(start_column) => {
                                let search_hit = SearchHit::new(
                                    line,
                                    line_number,
                                    start_column,
                                    start_column + self.search.len(),
                                );
                                hits.push(search_hit);
                            }
                            None => continue,
                        }
                    }

                    if failed_to_read_line {
                        continue;
                    }

                    if hits.is_empty() {
                        continue;
                    }

                    return Some(SearchFileHit {
                        file: entry.path(),
                        hits,
                    });
                }
                None => {
                    return None;
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct SearchFileHit {
    pub file: PathBuf,
    pub hits: Vec<SearchHit>,
}

#[derive(Clone)]
pub struct SearchHit {
    pub line: Box<String>,
    pub line_number: usize,
    pub start_column: usize,
    pub end_column: usize,
}

impl SearchHit {
    fn new(line: String, line_number: usize, start_column: usize, end_column: usize) -> Self {
        SearchHit {
            line: Box::new(line),
            line_number,
            start_column,
            end_column,
        }
    }
}

use regex::Regex;
use std::fs;
use std::mem::swap;
use std::path::Path;

pub struct Finder {
    regex: Regex,
    stack: Vec<Box<dyn Iterator<Item = fs::DirEntry>>>,
    iterator: Box<dyn Iterator<Item = fs::DirEntry>>,
}

impl Finder {
    pub fn new(directory: &Path, pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        let stack = Vec::new();
        let iterator = Finder::get_directory_iterator(directory, regex.clone());

        Ok(Finder {
            regex,
            stack,
            iterator,
        })
    }

    fn get_directory_iterator(
        directory: &Path,
        regex: Regex,
    ) -> Box<dyn Iterator<Item = fs::DirEntry>> {
        let directory_entries = fs::read_dir(&*directory).unwrap();
        let filtered_entries = directory_entries
            .filter(move |entry| {
                entry.as_ref().unwrap().path().is_dir()
                    || regex.is_match(&entry.as_ref().unwrap().file_name().to_string_lossy())
            })
            .map(|entry| entry.unwrap());
        Box::new(filtered_entries)
    }
}

impl Iterator for Finder {
    type Item = fs::DirEntry;

    fn next(&mut self) -> Option<fs::DirEntry> {
        let next_entry = self.iterator.next();
        match next_entry {
            Some(entry) => {
                if entry.path().is_dir() {
                    let mut iterator =
                        Finder::get_directory_iterator(&entry.path(), self.regex.clone());
                    swap(&mut iterator, &mut self.iterator);
                    self.stack.push(Box::new(iterator));
                    self.next()
                } else {
                    Some(entry)
                }
            }
            None => {
                let next_iterator = self.stack.pop();
                match next_iterator {
                    Some(iterator) => {
                        self.iterator = Box::new(iterator);
                        self.next()
                    }
                    None => None,
                }
            }
        }
    }
}

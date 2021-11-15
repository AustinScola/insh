use std::fs;
use std::mem::swap;
use std::path::Path;

pub struct Walker {
    stack: Vec<Box<dyn Iterator<Item = fs::DirEntry>>>,
    iterator: Box<dyn Iterator<Item = fs::DirEntry>>,
}

impl From<&Path> for Walker {
    fn from(directory: &Path) -> Self {
        let stack = Vec::new();
        let iterator = Box::new(
            fs::read_dir(&*directory)
                .unwrap()
                .map(|entry| entry.unwrap()),
        );

        Walker { stack, iterator }
    }
}

impl Iterator for Walker {
    type Item = fs::DirEntry;

    fn next(&mut self) -> Option<fs::DirEntry> {
        let next_entry = self.iterator.next();
        match next_entry {
            Some(entry) => {
                if entry.path().is_dir() {
                    let mut iterator: Box<dyn Iterator<Item = fs::DirEntry>> = Box::new(
                        fs::read_dir(entry.path())
                            .unwrap()
                            .map(|directory_entry| directory_entry.unwrap()),
                    );
                    swap(&mut iterator, &mut self.iterator);
                    self.stack.push(iterator);
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

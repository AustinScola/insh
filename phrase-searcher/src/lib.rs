/*!
Searches the files in a directory (and all sub-directories) for a phrase.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use highlighter::{Highlighter, Kind, Language, Run};

use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::{
    DirEntry as Entry, Error as WalkerEntryError, IntoIter as Walker, WalkDir as WalkerBuilder,
};

/// How many bytes a file can have for all of it to be parsed to highlight the lines with hits in
/// it.
///
/// What a line is depends on the lines around it, such as where a comment it is in starts and
/// ends, so the whole file is parsed. Past this, each line with a hit is highlighted on its own
/// instead, which is quicker but can be wrong about a line inside something like a comment.
const CONTEXT: usize = 1 << 20;

/// Searches files for a phrase.
pub struct PhraseSearcher {
    /// The phrase to search for.
    phrase: String,
    /// What the names of the files to search must match, which is anything when it is nothing.
    files: Option<Regex>,
    /// A file walker.
    walker: Walker,
    /// Works out which part of each line with a hit is what.
    highlighter: Highlighter,
}

impl PhraseSearcher {
    /// Return a new phrase searcher.
    pub fn new(directory: &Path, phrase: &str) -> Self {
        let phrase: String = phrase.to_string();
        let walker: Walker = WalkerBuilder::new(directory).min_depth(1).into_iter();
        Self {
            phrase,
            files: None,
            walker,
            highlighter: Highlighter::new(),
        }
    }

    /// Return the same, but only searching the files whose names match.
    ///
    /// A file whose name does not match is not searched at all, so it is not reported either.
    pub fn only(mut self, files: Regex) -> Self {
        self.files = Some(files);
        self
    }

    /// Return the runs of each of the lines of the file which have hits, which are nothing for a
    /// file in a language which is not known.
    fn highlight(&mut self, path: &Path, lines: &[String], hits: &[usize]) -> Vec<Vec<Run>> {
        let language: Option<Language> = path
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(Language::of_extension);
        let language: Language = match language {
            Some(language) => language,
            None => return vec![Vec::new(); hits.len()],
        };

        if lines.iter().map(|line| line.len() + 1).sum::<usize>() <= CONTEXT {
            let Some(document) = self.highlighter.document(language) else {
                return vec![Vec::new(); hits.len()];
            };
            return hits
                .iter()
                .map(|hit| {
                    document
                        .highlight(lines, *hit..*hit + 1)
                        .pop()
                        .unwrap_or_default()
                })
                .collect();
        }

        hits.iter()
            .map(|hit| {
                let line: &[String] = std::slice::from_ref(&lines[*hit]);
                match self.highlighter.document(language) {
                    Some(document) => document.highlight(line, 0..1).pop().unwrap_or_default(),
                    None => Vec::new(),
                }
            })
            .collect()
    }
}

/// A file which the phrase searcher searched.
///
/// Every file which is searched is reported (not just the ones with hits) so that the progress of
/// searching files can be followed.
pub enum Searched {
    /// The file has lines which contain the phrase.
    Hit(FileHit),
    /// The file does not have any lines which contain the phrase.
    NoHit,
}

impl Iterator for PhraseSearcher {
    type Item = Searched;

    fn next(&mut self) -> Option<Searched> {
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
                        if let Some(files) = &self.files {
                            if !files.is_match(&entry.file_name().to_string_lossy()) {
                                continue;
                            }
                        }

                        let file = match File::open(path) {
                            Ok(file) => file,
                            Err(error) => {
                                log::warn!("Failed to open {:?}: {}", path, error);
                                return Some(Searched::NoHit);
                            }
                        };
                        let reader = BufReader::new(file);

                        // NOTE: Every line is kept rather than only the ones with hits, since
                        // what a line is depends on the lines around it.
                        let mut failed_to_read_line: bool = false;
                        let mut lines: Vec<String> = Vec::new();
                        let mut hits: Vec<usize> = Vec::new();
                        for line in reader.lines() {
                            let line: String = match line {
                                Ok(line) => line,
                                Err(_) => {
                                    failed_to_read_line = true;
                                    break;
                                }
                            };

                            if line.contains(&self.phrase) {
                                hits.push(lines.len());
                            }
                            lines.push(line);
                        }

                        if failed_to_read_line || hits.is_empty() {
                            return Some(Searched::NoHit);
                        }

                        let runs: Vec<Vec<Run>> = self.highlight(path, &lines, &hits);
                        let line_hits: Vec<LineHit> = hits
                            .iter()
                            .zip(runs)
                            .map(|(hit, runs)| LineHit::new(hit + 1, &lines[*hit]).with_runs(runs))
                            .collect();

                        let file_hit = FileHit::new(path, line_hits);
                        return Some(Searched::Hit(file_hit));
                    }
                },
            }
        }
    }
}

/// A file with hits for a phrase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHit {
    /// The path of the file.
    path: PathBuf,
    /// The lines containing hits.
    line_hits: Vec<LineHit>,
}

impl FileHit {
    /// Return a new file hit.
    pub fn new(path: &Path, line_hits: Vec<LineHit>) -> Self {
        let path: PathBuf = path.to_path_buf();
        Self { path, line_hits }
    }

    /// Return the path of the file containing line hits.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Return the line hits.
    pub fn line_hits(&self) -> &Vec<LineHit> {
        &self.line_hits
    }
}

/// A line with a hit for a phrase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineHit {
    /// The line number in the file.
    line_number: usize,
    /// The contents of the line.
    line: String,
    /// What each piece of the line is, which is nothing for a file in a language which is not
    /// known.
    runs: Vec<Run>,
}

impl LineHit {
    /// Return a new line hit.
    pub fn new(line_number: usize, line: &str) -> Self {
        Self {
            line_number,
            line: line.to_string(),
            runs: Vec::new(),
        }
    }

    /// Return the same, along with what each piece of the line is.
    pub fn with_runs(mut self, runs: Vec<Run>) -> Self {
        self.runs = runs;
        self
    }

    /// Return the line number of the line hit.
    pub fn line_number(&self) -> usize {
        self.line_number
    }

    /// Return the contents of the line hit.
    pub fn line(&self) -> &str {
        &self.line
    }

    /// Return what each piece of the line is.
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// Return the same line hit with the phrase replaced everywhere in the line.
    ///
    /// NOTE: The line is not parsed again, so what is put in place of the phrase is whatever the
    /// first character of the phrase was, and the rest of the line stays what it was.
    pub fn replaced(&self, phrase: &str, replacement: &str) -> Self {
        let line: String = self.line.replace(phrase, replacement);
        if self.runs.is_empty() || phrase.is_empty() {
            return Self::new(self.line_number, &line);
        }

        let kinds: Vec<Option<Kind>> = self
            .runs
            .iter()
            .flat_map(|run| std::iter::repeat_n(run.kind, run.chars))
            .collect();
        let kind = |character: usize| kinds.get(character).copied().flatten();

        let mut runs: Vec<Run> = Vec::new();
        let mut push = |kind: Option<Kind>, chars: usize| match runs.last_mut() {
            _ if chars == 0 => {}
            Some(last) if last.kind == kind => last.chars += chars,
            _ => runs.push(Run { chars, kind }),
        };

        let phrase_chars: usize = phrase.chars().count();
        let replacement_chars: usize = replacement.chars().count();
        let mut byte: usize = 0;
        let mut character: usize = 0;
        for (start, _) in self.line.match_indices(phrase) {
            for _ in self.line[byte..start].chars() {
                push(kind(character), 1);
                character += 1;
            }
            push(kind(character), replacement_chars);
            character += phrase_chars;
            byte = start + phrase.len();
        }
        for _ in self.line[byte..].chars() {
            push(kind(character), 1);
            character += 1;
        }

        Self::new(self.line_number, &line).with_runs(runs)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{create_dir_all, remove_dir_all, write};
    use std::path::PathBuf;

    use super::{FileHit, LineHit, PhraseSearcher, Searched};

    use highlighter::{Kind, Run};

    /// A directory which is removed again once it is done with.
    struct Scratch {
        /// Where it is.
        dir: PathBuf,
    }

    impl Scratch {
        /// Return a new directory with a file of the name and the contents in it.
        fn with(name: &str, contents: &str) -> Self {
            let dir: PathBuf = std::env::temp_dir().join(format!(
                "phrase-searcher-{}-{}",
                std::process::id(),
                name
            ));
            create_dir_all(&dir).unwrap();
            write(dir.join(name), contents).unwrap();
            Self { dir }
        }

        /// Return the one file hit when searching it for the phrase.
        fn search(&self, phrase: &str) -> FileHit {
            for searched in PhraseSearcher::new(&self.dir, phrase) {
                if let Searched::Hit(file_hit) = searched {
                    return file_hit;
                }
            }
            panic!("Expected a hit.");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn test_a_hit_inside_a_comment_started_above_it_is_a_comment() {
        let scratch: Scratch = Scratch::with("a.rs", "/*\nfn needle\n*/\n");

        let file_hit: FileHit = scratch.search("needle");

        assert_eq!(
            file_hit.line_hits()[0].runs(),
            &[Run {
                chars: 9,
                kind: Some(Kind::Comment),
            }]
        );
    }

    #[test]
    fn test_every_hit_is_highlighted() {
        let scratch: Scratch = Scratch::with("b.rs", "fn needle() {}\n\nfn needle() {}\n");

        let file_hit: FileHit = scratch.search("needle");

        for line_hit in file_hit.line_hits() {
            assert_eq!(
                line_hit.runs()[0],
                Run {
                    chars: 2,
                    kind: Some(Kind::Keyword),
                }
            );
        }
        assert_eq!(file_hit.line_hits().len(), 2);
    }

    #[test]
    fn test_a_file_in_a_language_which_is_not_known_has_no_runs() {
        let scratch: Scratch = Scratch::with("c.txt", "fn needle\n");

        let file_hit: FileHit = scratch.search("needle");

        assert!(file_hit.line_hits()[0].runs().is_empty());
    }

    #[test]
    fn test_what_replaces_the_phrase_is_what_the_phrase_was() {
        let line_hit: LineHit = LineHit::new(1, "fn ab(\"ab\")").with_runs(vec![
            Run {
                chars: 3,
                kind: Some(Kind::Keyword),
            },
            Run {
                chars: 2,
                kind: Some(Kind::Function),
            },
            Run {
                chars: 1,
                kind: None,
            },
            Run {
                chars: 4,
                kind: Some(Kind::String),
            },
            Run {
                chars: 1,
                kind: None,
            },
        ]);

        let replaced: LineHit = line_hit.replaced("ab", "xyz");

        assert_eq!(replaced.line(), "fn xyz(\"xyz\")");
        assert_eq!(
            replaced.runs(),
            &[
                Run {
                    chars: 3,
                    kind: Some(Kind::Keyword),
                },
                Run {
                    chars: 3,
                    kind: Some(Kind::Function),
                },
                Run {
                    chars: 1,
                    kind: None,
                },
                Run {
                    chars: 5,
                    kind: Some(Kind::String),
                },
                Run {
                    chars: 1,
                    kind: None,
                },
            ]
        );
    }

    #[test]
    fn test_a_line_with_no_runs_is_replaced_without_any() {
        let replaced: LineHit = LineHit::new(1, "a b a").replaced("a", "c");

        assert_eq!(replaced.line(), "c b c");
        assert!(replaced.runs().is_empty());
    }
}

//! Handles requests to replace a phrase in some of the lines of some files.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use insh_api::{
    ReplacePhraseRequestParams, ReplacePhraseResponseParams, ReplacedLine, ResponseParams,
    ResponseParamsAndLast,
};

/// Handles replacing a phrase in some of the lines of some files.
pub struct ReplacePhrase {
    /// The phrase to replace.
    phrase: String,
    /// What to replace it with.
    replacement: String,
    /// The lines to replace it in, by the file they are in.
    lines: BTreeMap<PathBuf, Vec<ReplacedLine>>,
    /// Whether replacing it is done.
    done: bool,
}

impl ReplacePhrase {
    /// Return a replacer of the phrase in the lines.
    pub fn new(params: &ReplacePhraseRequestParams) -> Self {
        let mut lines: BTreeMap<PathBuf, Vec<ReplacedLine>> = BTreeMap::new();
        for line in params.lines() {
            lines
                .entry(line.path().to_path_buf())
                .or_default()
                .push(line.clone());
        }

        Self {
            phrase: params.phrase().to_string(),
            replacement: params.replacement().to_string(),
            lines,
            done: false,
        }
    }

    /// Replace the phrase in the lines of the file, and return how many of them it was replaced
    /// in.
    ///
    /// A line which is not what it was when the phrase was found in it is left alone, since the
    /// file has been changed since and the line may not be the one which was looked at. What ends
    /// each line is kept as it was, and the file is written back in place, so that it keeps who
    /// owns it and who can read it.
    fn replace_in(&self, path: &Path, lines: &[ReplacedLine]) -> Result<usize, String> {
        let contents: String = fs::read_to_string(path)
            .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;

        let mut replaced: usize = 0;
        let mut written = String::with_capacity(contents.len());
        for (line_number, line) in (1..).zip(contents.split_inclusive('\n')) {
            let ending: usize = line.len() - line.trim_end_matches(['\n', '\r']).len();
            let (text, end) = line.split_at(line.len() - ending);

            let wanted: bool = lines
                .iter()
                .any(|wanted| wanted.line_number() == line_number && wanted.line() == text);
            match wanted && text.contains(&self.phrase) {
                true => {
                    written.push_str(&text.replace(&self.phrase, &self.replacement));
                    replaced += 1;
                }
                false => written.push_str(text),
            }
            written.push_str(end);
        }

        if replaced < lines.len() {
            log::warn!(
                "{} of the lines of {} were not what they were when the phrase was found.",
                lines.len() - replaced,
                path.display()
            );
        }
        if replaced > 0 {
            fs::write(path, written)
                .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
        }

        Ok(replaced)
    }
}

impl Iterator for ReplacePhrase {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }
        self.done = true;

        // NOTE: Nothing is replaced in place of nothing, since nothing is in every line, between
        // every character of it.
        if self.phrase.is_empty() {
            log::error!("Failed to replace the phrase: it is empty.");
            return Some(
                ResponseParamsAndLast::builder()
                    .response_params(ResponseParams::ReplacePhrase(
                        ReplacePhraseResponseParams::builder()
                            .replaced(0)
                            .failures(vec![String::from(
                                "Failed to replace the phrase: it is empty",
                            )])
                            .build(),
                    ))
                    .last(true)
                    .build(),
            );
        }

        let mut replaced: usize = 0;
        let mut failures: Vec<String> = Vec::new();
        for (path, lines) in &self.lines {
            match self.replace_in(path, lines) {
                Ok(count) => {
                    replaced += count;
                    if count < lines.len() {
                        failures.push(format!(
                            "Failed to find {} of the lines in {}",
                            lines.len() - count,
                            path.display()
                        ));
                    }
                }
                Err(failure) => {
                    log::error!("{}", failure);
                    failures.push(failure);
                }
            }
        }
        log::info!("Replaced the phrase in {} lines.", replaced);

        let response_params: ResponseParams = ResponseParams::ReplacePhrase(
            ReplacePhraseResponseParams::builder()
                .replaced(replaced)
                .failures(failures)
                .build(),
        );
        Some(
            ResponseParamsAndLast::builder()
                .response_params(response_params)
                .last(true)
                .build(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use test_case::test_case;

    /// Return the line of the file, as it was when the phrase was found in it.
    fn line(path: &Path, line_number: usize, line: &str) -> ReplacedLine {
        ReplacedLine::builder()
            .path(path.to_path_buf())
            .line_number(line_number)
            .line(line.to_string())
            .build()
    }

    #[test_case("one foo\nfoo foo\nfoo\n", &[(2, "foo foo")], "one foo\nbar bar\nfoo\n"; "only the lines asked for")]
    #[test_case("foo\r\nfoo\r\n", &[(1, "foo"), (2, "foo")], "bar\r\nbar\r\n"; "keeping what ends the lines")]
    #[test_case("foo\nfoo", &[(2, "foo")], "foo\nbar"; "the last line with nothing ending it")]
    #[test_case("foo\nchanged foo\n", &[(2, "foo")], "foo\nchanged foo\n"; "not a line which has changed since")]
    fn test_replace(contents: &str, lines: &[(usize, &str)], expected: &str) {
        let dir: PathBuf =
            std::env::temp_dir().join(format!("insh-replace-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let path: PathBuf = dir.join("file");
        fs::write(&path, contents).unwrap();

        let params = ReplacePhraseRequestParams::builder()
            .phrase(String::from("foo"))
            .replacement(String::from("bar"))
            .lines(
                lines
                    .iter()
                    .map(|(number, text)| line(&path, *number, text))
                    .collect(),
            )
            .build();
        let responses: Vec<ResponseParamsAndLast> = ReplacePhrase::new(&params).collect();

        assert_eq!(fs::read_to_string(&path).unwrap(), expected);
        assert_eq!(responses.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_nothing_is_replaced_in_place_of_nothing() {
        let dir: PathBuf =
            std::env::temp_dir().join(format!("insh-replace-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let path: PathBuf = dir.join("file");
        fs::write(&path, "foo\n").unwrap();

        let params = ReplacePhraseRequestParams::builder()
            .phrase(String::new())
            .replacement(String::from("bar"))
            .lines(vec![line(&path, 1, "foo")])
            .build();
        let responses: Vec<ResponseParamsAndLast> = ReplacePhrase::new(&params).collect();

        assert_eq!(fs::read_to_string(&path).unwrap(), "foo\n");
        match &responses[0].response_params {
            ResponseParams::ReplacePhrase(params) => assert!(!params.failures().is_empty()),
            _ => panic!("Expected how replacing went."),
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}

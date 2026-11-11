//! Handles requests to suggest a file in a directory.
use std::fs::{self, DirEntry};
use std::path::{PathBuf, MAIN_SEPARATOR as PATH_SEPARATOR};

use insh_api::{ResponseParams, ResponseParamsAndLast, SuggestFileResponseParams};

use typed_builder::TypedBuilder;

/// Handles a request to suggest a file in a directory.
#[derive(TypedBuilder)]
pub struct SuggestFile {
    /// The directory the path is in.
    dir: PathBuf,
    /// The path so far, from the directory.
    #[builder(setter(into))]
    partial: String,
    /// If suggesting a file is done.
    #[builder(default)]
    done: bool,
}

impl SuggestFile {
    /// Return the first entry of the directory named by all but the last part of the path so far
    /// whose name starts with that last part, which ends with a separator when it is a directory
    /// so that the rest of the path can be gone on with.
    ///
    /// Hidden entries are only suggested once the leading dot has been typed, the way a shell
    /// completes them.
    fn suggest(&self) -> Option<String> {
        let (parent, name): (&str, &str) = match self.partial.rfind(PATH_SEPARATOR) {
            Some(separator) => self.partial.split_at(separator + 1),
            None => ("", self.partial.as_str()),
        };
        let dir: PathBuf = self.dir.join(parent);

        let read_dir = match fs::read_dir(&dir) {
            Ok(read_dir) => read_dir,
            Err(error) => {
                log::debug!("Error reading {:?}: {}", dir, error);
                return None;
            }
        };

        let mut entries: Vec<(String, bool)> = Vec::new();
        for dir_entry in read_dir {
            let dir_entry: DirEntry = match dir_entry {
                Ok(dir_entry) => dir_entry,
                Err(error) => {
                    log::debug!("Error reading an entry of {:?}: {}", dir, error);
                    continue;
                }
            };

            let entry_name: String = dir_entry.file_name().to_string_lossy().into_owned();
            if !entry_name.starts_with(name) {
                continue;
            }
            if entry_name.starts_with('.') && !name.starts_with('.') {
                continue;
            }

            entries.push((entry_name, dir_entry.path().is_dir()));
        }

        entries.sort();

        let (entry_name, is_dir): (String, bool) = entries.into_iter().next()?;
        let mut suggestion: String = format!("{}{}", parent, entry_name);
        if is_dir {
            suggestion.push(PATH_SEPARATOR);
        }
        Some(suggestion)
    }
}

impl Iterator for SuggestFile {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let suggestion: Option<String> = self.suggest();

        let response_params: ResponseParams = ResponseParams::SuggestFile(
            SuggestFileResponseParams::builder()
                .suggestion(suggestion)
                .build(),
        );

        self.done = true;

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

    use std::fs::File;

    use test_case::test_case;

    #[test_case("ma", Some("main.rs"); "a file")]
    #[test_case("sr", Some("src/"); "a directory")]
    #[test_case("src/li", Some("src/lib.rs"); "a file in a directory")]
    #[test_case("", Some("main.rs"); "nothing typed")]
    #[test_case(".hi", Some(".hidden"); "a hidden file once the dot is typed")]
    #[test_case("x", None; "nothing which matches")]
    fn test_suggest(partial: &str, expected: Option<&str>) {
        let dir: PathBuf =
            std::env::temp_dir().join(format!("insh-suggest-file-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        File::create(dir.join("main.rs")).unwrap();
        File::create(dir.join(".hidden")).unwrap();
        fs::create_dir(dir.join("src")).unwrap();
        File::create(dir.join("src").join("lib.rs")).unwrap();

        let suggest_file = SuggestFile::builder()
            .dir(dir.clone())
            .partial(partial)
            .build();

        assert_eq!(suggest_file.suggest().as_deref(), expected);

        fs::remove_dir_all(&dir).unwrap();
    }
}

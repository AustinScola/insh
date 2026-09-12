//! Handles requests to suggest a directory.
use std::error::Error;
use std::fs::{self, DirEntry};
use std::path::{Path, MAIN_SEPARATOR as PATH_SEPARATOR};

use insh_api::{ResponseParams, ResponseParamsAndLast, SuggestDirResponseParams};
use insh_db::{dir_history, DbConnPool};

use typed_builder::TypedBuilder;

/// The number of visited directories to look at before falling back to the file system. More than
/// one is looked at because a directory which was visited may since have been moved or removed.
const HISTORY_CANDIDATES: usize = 16;

/// Handles a request to suggest a directory.
#[derive(TypedBuilder)]
pub struct SuggestDir {
    /// The path so far.
    #[builder(setter(into))]
    partial: String,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// If suggesting a directory is done.
    #[builder(default)]
    done: bool,
}

impl SuggestDir {
    /// Return the path to suggest, or `None` if there is not one.
    ///
    /// A directory which has been visited is preferred over one which has not, because the one
    /// which has been visited is the one which is more likely to be wanted again.
    fn suggest(&self) -> Option<String> {
        match self.suggest_visited() {
            Ok(Some(path)) => {
                return Some(path);
            }
            Ok(None) => {}
            Err(error) => {
                log::error!("Failed to read the directory history: {}", error);
            }
        }

        self.suggest_child()
    }

    /// Return the most recently visited directory which starts with the path so far and which is
    /// still a directory.
    fn suggest_visited(&self) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
        let paths: Vec<String> =
            dir_history::suggest(&self.db_conn_pool, &self.partial, HISTORY_CANDIDATES)?;

        let path: Option<String> = paths
            .into_iter()
            .find(|path| Path::new(path).is_dir())
            .map(Self::with_separator);

        return Ok(path);
    }

    /// Return the first subdirectory of the directory named by all but the last part of the path so
    /// far whose name starts with that last part.
    ///
    /// Hidden subdirectories are only suggested once the leading dot has been typed, the way a
    /// shell completes them.
    fn suggest_child(&self) -> Option<String> {
        let separator: usize = self.partial.rfind(PATH_SEPARATOR)?;
        let (dir, name): (&str, &str) = self.partial.split_at(separator + 1);

        let read_dir = match fs::read_dir(dir) {
            Ok(read_dir) => read_dir,
            Err(error) => {
                log::debug!("Error reading {:?}: {}", dir, error);
                return None;
            }
        };

        let mut names: Vec<String> = Vec::new();
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

            // NOTE: This follows symlinks, so a link to a directory is suggested too. Changing
            // directories to one works just as well as changing to the directory it points at.
            if !dir_entry.path().is_dir() {
                continue;
            }

            names.push(entry_name);
        }

        names.sort();

        let name: String = names.into_iter().next()?;
        Some(Self::with_separator(format!("{}{}", dir, name)))
    }

    /// Return a path which ends with a path separator.
    fn with_separator(mut path: String) -> String {
        if !path.ends_with(PATH_SEPARATOR) {
            path.push(PATH_SEPARATOR);
        }
        path
    }
}

impl Iterator for SuggestDir {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let suggestion: Option<String> = self.suggest();

        let response_params: ResponseParams = ResponseParams::SuggestDir(
            SuggestDirResponseParams::builder()
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

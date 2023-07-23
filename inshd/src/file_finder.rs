//! Finds files.
use path_finder::Entry;
use path_finder::NewPathFinderError;
use path_finder::PathFinder;

use std::fmt::{Display, Error as FmtError, Formatter};
use std::path::PathBuf;

use crossbeam::channel::Sender;
use typed_builder::TypedBuilder;

/// Finds files.
#[derive(TypedBuilder)]
pub struct FileFinder {
    /// A sender of results of finding files.
    results_tx: Sender<FindFilesResult>,
}

impl FileFinder {
    /// Run the file finder.
    pub fn run(&mut self, options: FileFinderOptions) {
        log::info!("File finder running...");

        let mut path_finder = match PathFinder::new(&options.dir, &options.pattern) {
            Ok(path_finder) => path_finder,
            Err(error) => {
                self.results_tx
                    .send(Err(FindFilesError::FailedToConstructPathFinder(error)))
                    .unwrap();
                return;
            }
        };

        loop {
            let entry: Option<Entry> = path_finder.next();
            let entry: Entry = match entry {
                Some(entry) => entry,
                None => {
                    log::info!("No more entries.");
                    self.results_tx.send(Ok(None)).unwrap();
                    break;
                }
            };

            log::debug!("Found matching entry {:?}.", entry.path());

            if let Err(error) = self.results_tx.send(Ok(Some(entry))) {
                log::error!("Error sending found entry: {}", error);
                break;
            }
        }

        log::info!("File finder stopping...");
    }
}

/// Options for finding files.
#[derive(TypedBuilder)]
pub struct FileFinderOptions {
    /// The directory to look for files in.
    #[builder(setter(into))]
    pub dir: PathBuf,
    /// A pattern to look for.
    #[builder(setter(into))]
    pub pattern: String,
}

/// An error finding files.
pub enum FindFilesError {
    /// A failure to construct the path finder.
    FailedToConstructPathFinder(NewPathFinderError),
}

impl Display for FindFilesError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::FailedToConstructPathFinder(error) => {
                write!(formatter, "Failed to construct path finder: {}", error)
            }
        }
    }
}

/// A result of finding files.
pub type FindFilesResult = Result<Option<Entry>, FindFilesError>;

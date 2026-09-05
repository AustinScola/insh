//! Finds files.
use std::fmt::{Display, Error as FmtError, Formatter};
use std::mem;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use path_finder::Entry;
use path_finder::Examined;
use path_finder::NewPathFinderError;
use path_finder::PathFinder;

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

        let started: Instant = Instant::now();

        let mut path_finder = match PathFinder::new(&options.dir, &options.pattern) {
            Ok(path_finder) => path_finder,
            Err(error) => {
                self.results_tx
                    .send(Err(FindFilesError::FailedToConstructPathFinder(error)))
                    .unwrap();
                return;
            }
        };

        // The files which have been found since the last time results were sent.
        let mut entries: Vec<Entry> = Vec::new();
        // The number of files which have been examined.
        let mut searched: usize = 0;
        // When the results were last sent.
        let mut sent: Instant = started;

        loop {
            match path_finder.next() {
                Some(Examined::Matched(entry)) => {
                    log::debug!("Found matching entry {:?}.", entry.path());
                    searched += 1;
                    entries.push(entry);
                }
                Some(Examined::NotMatched) => {
                    searched += 1;
                }
                None => {
                    log::info!("No more entries.");
                    let found = FoundFiles::builder()
                        .entries(entries)
                        .searched(searched)
                        .duration(started.elapsed())
                        .done(true)
                        .build();
                    self.results_tx.send(Ok(found)).unwrap();
                    break;
                }
            }

            if sent.elapsed() < options.update_interval {
                continue;
            }

            let found = FoundFiles::builder()
                .entries(mem::take(&mut entries))
                .searched(searched)
                .duration(started.elapsed())
                .done(false)
                .build();
            if let Err(error) = self.results_tx.send(Ok(found)) {
                log::error!("Error sending found entries: {}", error);
                break;
            }
            sent = Instant::now();
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
    /// How often the files which have been found (and the progress of finding them) are reported.
    ///
    /// The results are reported on an interval instead of as soon as each file is examined so that
    /// a directory with a lot of files in it does not flood the client with updates.
    #[builder(default = Duration::from_millis(100))]
    pub update_interval: Duration,
}

/// The files which have been found since the last result (and the progress of finding files).
#[derive(TypedBuilder)]
pub struct FoundFiles {
    /// The files which have been found since the last result.
    pub entries: Vec<Entry>,
    /// The number of files which have been searched.
    pub searched: usize,
    /// The duration of the search so far.
    pub duration: Duration,
    /// Whether or not there are any more files to find.
    pub done: bool,
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
pub type FindFilesResult = Result<FoundFiles, FindFilesError>;

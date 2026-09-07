//! Searches files for a phrase.
use std::mem;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use phrase_searcher::FileHit;
use phrase_searcher::PhraseSearcher;
use phrase_searcher::Searched;

use crossbeam::channel::Sender;
use typed_builder::TypedBuilder;

/// Searches files for a phrase.
#[derive(TypedBuilder)]
pub struct FileSearcher {
    /// A sender of results of searching files.
    results_tx: Sender<SearchPhraseResult>,
}

impl FileSearcher {
    /// Run the file searcher.
    pub fn run(&mut self, options: FileSearcherOptions) {
        log::info!("File searcher running...");

        let started: Instant = Instant::now();

        let mut phrase_searcher = PhraseSearcher::new(&options.dir, &options.phrase);

        // The hits which have been found since the last time results were sent.
        let mut hits: Vec<FileHit> = Vec::new();
        // The number of files which have been searched.
        let mut searched: usize = 0;
        // When the results were last sent.
        let mut sent: Instant = started;

        loop {
            match phrase_searcher.next() {
                Some(Searched::Hit(hit)) => {
                    log::debug!("Found hit in {:?}.", hit.path());
                    searched += 1;
                    hits.push(hit);
                }
                Some(Searched::NoHit) => {
                    searched += 1;
                }
                None => {
                    log::info!("No more hits.");
                    let found = FoundHits::builder()
                        .hits(hits)
                        .searched(searched)
                        .duration(started.elapsed())
                        .done(true)
                        .build();
                    self.results_tx.send(found).unwrap();
                    break;
                }
            }

            if sent.elapsed() < options.update_interval {
                continue;
            }

            let found = FoundHits::builder()
                .hits(mem::take(&mut hits))
                .searched(searched)
                .duration(started.elapsed())
                .done(false)
                .build();
            if let Err(error) = self.results_tx.send(found) {
                log::error!("Error sending found hits: {}", error);
                break;
            }
            sent = Instant::now();
        }

        log::info!("File searcher stopping...");
    }
}

/// Options for searching files.
#[derive(TypedBuilder)]
pub struct FileSearcherOptions {
    /// The directory to search files in.
    #[builder(setter(into))]
    pub dir: PathBuf,
    /// The phrase to search for.
    #[builder(setter(into))]
    pub phrase: String,
    /// How often the hits which have been found (and the progress of searching for them) are
    /// reported.
    ///
    /// The results are reported on an interval instead of as soon as each file is searched so that
    /// a directory with a lot of files in it does not flood the client with updates.
    #[builder(default = Duration::from_millis(100))]
    pub update_interval: Duration,
}

/// The hits found since the last result, and the progress so far.
#[derive(TypedBuilder)]
pub struct FoundHits {
    /// The hits which have been found since the last result.
    pub hits: Vec<FileHit>,
    /// The number of files which have been searched.
    pub searched: usize,
    /// The duration of the search so far.
    pub duration: Duration,
    /// Whether there are any more files to search.
    pub done: bool,
}

/// A result of searching files.
pub type SearchPhraseResult = FoundHits;

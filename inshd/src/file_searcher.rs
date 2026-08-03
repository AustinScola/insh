//! Searches files for a phrase.
use phrase_searcher::FileHit;
use phrase_searcher::PhraseSearcher;

use std::path::PathBuf;

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

        let mut phrase_searcher = PhraseSearcher::new(&options.dir, &options.phrase);

        loop {
            let hit: Option<FileHit> = phrase_searcher.next();
            let hit: FileHit = match hit {
                Some(hit) => hit,
                None => {
                    log::info!("No more hits.");
                    self.results_tx.send(None).unwrap();
                    break;
                }
            };

            log::debug!("Found hit in {:?}.", hit.path());

            if let Err(error) = self.results_tx.send(Some(hit)) {
                log::error!("Error sending found hit: {}", error);
                break;
            }
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
}

/// A result of searching files.
pub type SearchPhraseResult = Option<FileHit>;

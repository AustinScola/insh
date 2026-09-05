//! Handles requests to search files for a phrase.
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::config::Config;
use crate::file_searcher::SearchPhraseResult;
use crate::file_searcher::{FileSearcher, FileSearcherOptions};

use insh_api::{
    ResponseParams, ResponseParamsAndLast, SearchPhraseRequestParams, SearchPhraseResponseParams,
};
use insh_db::{search_history, DbConnPool};

use crossbeam::channel::{self, Receiver, Sender};

/// Handles a request to search the contents of files for a phrase.
pub struct SearchPhrase {
    /// A receiver for results of searching the contents of files.
    results_rx: Receiver<SearchPhraseResult>,
    /// A handle to the thread for searching the contents of files.
    file_searcher_handle: Option<JoinHandle<()>>,
    /// If searching the contents of files is done.
    done: bool,
    /// The number of files which have been searched.
    searched: usize,
    /// The duration of the search so far.
    duration: Duration,
}

impl SearchPhrase {
    /// Search the contents of files for a phrase.
    pub fn run(
        params: &SearchPhraseRequestParams,
        config: Config,
        db_conn_pool: &DbConnPool,
    ) -> SearchPhrase {
        // Record the search in the search history right away, since the phrase was submitted
        // regardless of how the search itself turns out. Failing to record it should not stop the
        // search from happening, so the error is only logged.
        let history_length: usize = config.searcher().history().length();
        if let Err(error) = search_history::add(db_conn_pool, params.phrase(), history_length) {
            log::error!("Failed to add the phrase to the search history: {}", error);
        }

        // Create and start a thread to perform the searching of contents.
        let (results_tx, results_rx): (Sender<SearchPhraseResult>, Receiver<SearchPhraseResult>) =
            channel::unbounded();
        let mut file_searcher: FileSearcher =
            FileSearcher::builder().results_tx(results_tx).build();
        let file_searcher_options: FileSearcherOptions = FileSearcherOptions::builder()
            .dir(params.dir())
            .phrase(params.phrase())
            .build();
        let file_searcher_handle: JoinHandle<()> = thread::Builder::new()
            .name("file-searcher".to_string())
            .spawn(move || file_searcher.run(file_searcher_options))
            .unwrap();

        SearchPhrase {
            results_rx,
            file_searcher_handle: Some(file_searcher_handle),
            done: false,
            searched: 0,
            duration: Duration::ZERO,
        }
    }
}

impl Iterator for SearchPhrase {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let found: SearchPhraseResult = match self.results_rx.recv() {
            Ok(found) => found,
            Err(error) => {
                log::error!(
                    "Error receiving search phrase result from file searcher thread: {}",
                    error
                );
                self.done = true;
                if let Some(file_searcher_handle) = self.file_searcher_handle.take() {
                    let _ = file_searcher_handle.join();
                }
                return Some(
                    ResponseParamsAndLast::builder()
                        .response_params(ResponseParams::SearchPhrase(
                            SearchPhraseResponseParams::builder()
                                .hits(vec![])
                                .searched(self.searched)
                                .duration(self.duration)
                                .build(),
                        ))
                        .last(true)
                        .build(),
                );
            }
        };

        self.searched = found.searched;
        self.duration = found.duration;

        if found.done {
            self.done = true;
            let file_searcher_handle: JoinHandle<()> = self.file_searcher_handle.take().unwrap();
            let _ = file_searcher_handle.join();
        }

        return Some(
            ResponseParamsAndLast::builder()
                .response_params(ResponseParams::SearchPhrase(
                    SearchPhraseResponseParams::builder()
                        .hits(found.hits)
                        .searched(found.searched)
                        .duration(found.duration)
                        .build(),
                ))
                .last(found.done)
                .build(),
        );
    }
}

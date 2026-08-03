//! Handles requests to search files for a phrase.
use std::thread::{self, JoinHandle};

use crossbeam::channel::{self, select, Receiver, Sender};

use insh_api::{
    ResponseParams, ResponseParamsAndLast, SearchPhraseRequestParams, SearchPhraseResponseParams,
};
use phrase_searcher::FileHit;

use crate::config::Config;
use crate::data::Data;
use crate::file_searcher::SearchPhraseResult;
use crate::file_searcher::{FileSearcher, FileSearcherOptions};

/// Handles a request to search the contents of files for a phrase.
pub struct SearchPhrase {
    /// A receiver for results of searching the contents of files.
    results_rx: Receiver<SearchPhraseResult>,
    /// A handle to the thread for searching the contents of files.
    file_searcher_handle: Option<JoinHandle<()>>,
    /// If searching the contents of files is done.
    done: bool,
}

impl SearchPhrase {
    /// Search the contents of files for a phrase.
    pub fn run(params: &SearchPhraseRequestParams, config: Config) -> SearchPhrase {
        // Record the search in the search history right away, since the phrase was submitted
        // regardless of how the search itself turns out.
        let mut data: Data = Data::read();
        data.searcher
            .add_to_history(params.phrase(), config.searcher().history().length());
        data.write();
        data.release();

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
        }
    }
}

impl Iterator for SearchPhrase {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        select! {
            recv(self.results_rx) -> result => {
                let hit: SearchPhraseResult = match result {
                    Ok(hit) => hit,
                    Err(error) => {
                        log::error!("Error receiving search phrase result from file searcher thread: {}", error);
                        self.done = true;
                        if let Some(file_searcher_handle) = self.file_searcher_handle.take() {
                            let _ = file_searcher_handle.join();
                        }
                        return Some(ResponseParamsAndLast::builder()
                            .response_params(
                                ResponseParams::SearchPhrase(
                                    SearchPhraseResponseParams::builder()
                                        .hits(vec![])
                                        .build()
                                )
                            )
                            .last(true)
                            .build());
                    }
                };

                let hit: FileHit = match hit {
                    Some(hit) => hit,
                    None => {
                        self.done = true;
                        let file_searcher_handle: JoinHandle<()> = self.file_searcher_handle.take().unwrap();
                        let _ = file_searcher_handle.join();
                        return Some(ResponseParamsAndLast::builder()
                            .response_params(
                                ResponseParams::SearchPhrase(
                                    SearchPhraseResponseParams::builder()
                                        .hits(vec![])
                                        .build()
                                )
                            )
                            .last(true)
                            .build());
                    }
                };

                return Some(ResponseParamsAndLast::builder()
                    .response_params(
                        ResponseParams::SearchPhrase(
                            SearchPhraseResponseParams::builder()
                                .hits(vec![hit])
                                .build()
                        )
                    )
                    .last(false)
                    .build());
            }
        }
    }
}

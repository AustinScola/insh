/*!
Functionality for building `insh_api::Request`s which is shared between the components and the
entry point.
*/

/// Contains functionality for building requests for getting the files of a directory.
mod get_files {
    use std::path::PathBuf;

    use crate::config::{BrowserSortHiddenConfig, Config};

    use insh_api::{
        FileSortOptions, GetFilesRequestParams, HiddenFileSort, Request, RequestParams,
    };

    /// Return a request for getting the files of the directory.
    pub fn get_files_request(dir: PathBuf, config: &Config, metadata: bool) -> Request {
        let sort: Option<FileSortOptions> = config.browser().sort().map(|sort| {
            FileSortOptions::builder()
                .case_insensitive(sort.case_insensitive())
                .hidden(match sort.hidden() {
                    BrowserSortHiddenConfig::First => HiddenFileSort::First,
                    BrowserSortHiddenConfig::Last => HiddenFileSort::Last,
                    BrowserSortHiddenConfig::Mixed => HiddenFileSort::Mixed,
                })
                .build()
        });

        Request::builder()
            .params(RequestParams::GetFiles(
                GetFilesRequestParams::builder()
                    .dir(dir)
                    .sort(sort)
                    .metadata(metadata)
                    .build(),
            ))
            .build()
    }
}
pub use get_files::get_files_request;

/// Contains functionality for building requests for finding files.
mod find_files {
    use std::path::PathBuf;

    use insh_api::{FindFilesRequestParams, Request, RequestParams};

    /// Return a request for finding the files of the directory which match the pattern.
    pub fn find_files_request(dir: PathBuf, pattern: String) -> Request {
        Request::builder()
            .params(RequestParams::FindFiles(
                FindFilesRequestParams::builder()
                    .dir(dir)
                    .pattern(pattern)
                    .build(),
            ))
            .build()
    }
}
pub use find_files::find_files_request;

/// Contains functionality for building requests for searching files for a phrase.
mod search_phrase {
    use std::path::PathBuf;

    use insh_api::{Request, RequestParams, SearchPhraseRequestParams};

    /// Return a request for searching the files of the directory for the phrase.
    pub fn search_phrase_request(dir: PathBuf, phrase: String) -> Request {
        Request::builder()
            .params(RequestParams::SearchPhrase(
                SearchPhraseRequestParams::builder()
                    .dir(dir)
                    .phrase(phrase)
                    .build(),
            ))
            .build()
    }
}
pub use search_phrase::search_phrase_request;

/*!
Miscellaneous functionality which is shared between the components and the entry point.
*/

/// Contains functionality for building requests for getting the files of a directory.
mod get_files {
    use std::path::PathBuf;

    use insh_api::{
        FileSortOptions, GetFilesRequestParams, HiddenFileSort, Request, RequestParams,
    };

    use crate::config::{BrowserSortHiddenConfig, Config};

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

//! Handles requests to take note that a directory was gone to.
use std::path::PathBuf;

use crate::config::Config;

use insh_api::{ResponseParams, ResponseParamsAndLast, VisitDirResponseParams};
use insh_db::{dir_history, DbConnPool};

use typed_builder::TypedBuilder;

/// Handles a request to take note that a directory was gone to.
#[derive(TypedBuilder)]
pub struct VisitDir {
    /// The directory which was gone to.
    dir: PathBuf,
    /// The configuration for inshd.
    config: Config,
    /// A pool of connections to the database.
    db_conn_pool: DbConnPool,
    /// If taking note of the directory is done.
    #[builder(default)]
    done: bool,
}

impl Iterator for VisitDir {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let max_length: usize = self.config.browser().history().length();
        let dir: String = self.dir.to_string_lossy().into_owned();
        if let Err(error) = dir_history::add(&self.db_conn_pool, &dir, max_length) {
            log::error!(
                "Failed to add the directory to the directory history: {}",
                error
            );
        }

        self.done = true;

        Some(
            ResponseParamsAndLast::builder()
                .response_params(ResponseParams::VisitDir(
                    VisitDirResponseParams::builder().build(),
                ))
                .last(true)
                .build(),
        )
    }
}

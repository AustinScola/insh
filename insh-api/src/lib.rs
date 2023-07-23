#![allow(clippy::needless_return)]

use path_finder::Entry;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use uuid::Uuid;

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct Request {
    #[builder(default = Uuid::new_v4())]
    uuid: Uuid,
    params: RequestParams,
}

impl Request {
    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    pub fn params(&self) -> &RequestParams {
        &self.params
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RequestParams {
    FindFiles(FindFilesRequestParams),
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct FindFilesRequestParams {
    dir: PathBuf,
    pattern: String,
}

impl FindFilesRequestParams {
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct Response {
    uuid: Uuid,
    #[builder(default)]
    last: bool,
    params: ResponseParams,
}

impl Response {
    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    pub fn last(&self) -> bool {
        self.last
    }

    pub fn params(&self) -> &ResponseParams {
        &self.params
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseParams {
    FindFilesResponseParams(FindFilesResponseParams),
}

#[derive(Debug, TypedBuilder)]
pub struct ResponseParamsAndLast {
    pub response_params: ResponseParams,
    pub last: bool,
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct FindFilesResponseParams {
    entries: Vec<Entry>,
}

impl FindFilesResponseParams {
    pub fn entries(&self) -> &Vec<Entry> {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        return self.entries.is_empty();
    }
}

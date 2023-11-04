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
    Create(CreateRequestParams),
}

#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct CreateRequestParams {}

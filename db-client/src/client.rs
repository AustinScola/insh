use typed_builder::TypedBuilder;

use db_api::Request;

#[derive(TypedBuilder)]
pub struct DbClient {}

impl DbClient {
    pub fn handle(&self) -> DbClientHandle {
        DbClientHandle::builder().build()
    }
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""))]
pub struct DbClientHandle {}

impl DbClientHandle {
    pub fn request(&self, request: Request) {}
}

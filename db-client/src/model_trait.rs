use crate::DbClientHandle;

pub trait Model: Sized {
    type Objects: ObjectManager<ModelType = Self>;
}

pub trait ObjectManager {
    type ModelType;
    type CreatorType;
    type CreatorBuilderType;

    fn new(db_client_handle: DbClientHandle) -> Self;

    fn all() -> Vec<Self::ModelType>;

    fn creator(&self) -> Self::CreatorBuilderType;
}

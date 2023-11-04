mod model_trait;
pub use model_trait::{Model, ObjectManager};

mod field;

pub mod fields;

mod field_spec;

pub use model_macro::model;

mod client;
pub use client::{DbClient, DbClientHandle};

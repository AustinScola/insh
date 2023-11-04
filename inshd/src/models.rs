use db_client::{fields, model};

#[model]
pub struct Search {
    #[field(pk)]
    pub id: fields::U64,
    pub phrase: fields::Str,
    #[field(default=fields::DateTime::now_local_or_utc())]
    pub timestamp: fields::DateTime,
}

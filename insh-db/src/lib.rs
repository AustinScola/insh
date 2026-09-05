/*!
The database that inshd stores data in.

The database is an embedded PostgreSQL server which the caller starts and stops. The PostgreSQL
binaries are embedded in the executable, so PostgreSQL does not have to be installed separately.
*/
#![deny(missing_docs)]
#![allow(clippy::needless_return)]

mod database;
mod db_conn_pool_events;
pub mod find_history;
mod helpers;
mod schema;
pub mod search_history;

pub use database::{Database, DbConnPool, StartError};

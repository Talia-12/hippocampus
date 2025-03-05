use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel::r2d2::{Pool, ConnectionManager};

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub fn init_pool(database_url: &str) -> DbPool {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder().build(manager).expect("Failed to create pool.")
} 
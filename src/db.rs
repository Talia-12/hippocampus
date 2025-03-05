use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel::r2d2::{Pool, ConnectionManager};

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub fn init_pool(database_url: &str) -> DbPool {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder().build(manager).expect("Failed to create pool.")
} 

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_init_pool() {
        // Use an in-memory SQLite database for testing
        let database_url = ":memory:";
        let pool = init_pool(database_url);
        
        // Verify we can get a connection from the pool
        let conn_result = pool.get();
        assert!(conn_result.is_ok(), "Should be able to get a connection from the pool");
        
        // Verify the connection works by executing a simple query
        let mut conn = conn_result.unwrap();
        let result = diesel::sql_query("SELECT 1").execute(&mut *conn);
        assert!(result.is_ok(), "Should be able to execute a simple query");
    }
} 
/// Database connection module
///
/// This module provides functionality for creating and managing database connections
/// using Diesel's r2d2 connection pooling. It abstracts away the details of
/// connection management to provide a simple interface for the rest of the application.
use diesel::sqlite::SqliteConnection;
use diesel::r2d2::{Pool, ConnectionManager};

/// Type alias for a connection pool of SQLite connections
///
/// This type is used throughout the application to represent a pool of database
/// connections. Using a connection pool allows for efficient reuse of connections
/// and helps manage database resources.
pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

/// Initializes a new database connection pool
///
/// ### Arguments
///
/// * `database_url` - A string slice containing the database connection URL
///
/// ### Returns
///
/// A new connection pool configured with the provided database URL
///
/// ### Panics
///
/// This function will panic if the connection pool cannot be created
///
/// ### Examples
///
/// ```
/// use hippocampus::db;
/// use std::fs;
///
/// let pool = db::init_pool("database.db");
/// 
/// // Clean up the test database
/// fs::remove_file("database.db").ok();
/// ```
pub fn init_pool(database_url: &str) -> DbPool {
    // Create a new connection manager for SQLite
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    
    // Build a connection pool with default configuration
    // This will panic if the pool cannot be created
    Pool::builder().build(manager).expect("Failed to create pool.")
} 

#[cfg(test)]
mod tests {
    use diesel::prelude::*;

    use super::*;
    
    /// Tests the initialization of a database connection pool
    ///
    /// This test verifies that:
    /// 1. A connection pool can be created with an in-memory SQLite database
    /// 2. A connection can be successfully obtained from the pool
    /// 3. A simple SQL query can be executed on the connection
    #[test]
    fn test_init_pool() {
        // Use an in-memory SQLite database for testing
        // This is faster than using a file-based database and avoids cleanup
        let database_url = ":memory:";
        let pool = init_pool(database_url);
        
        // Verify we can get a connection from the pool
        // This ensures the pool is properly configured
        let conn_result = pool.get();
        assert!(conn_result.is_ok(), "Should be able to get a connection from the pool");
        
        // Verify the connection works by executing a simple query
        // This ensures the connection is valid and can execute SQL
        let mut conn = conn_result.unwrap();
        let result = diesel::sql_query("SELECT 1").execute(&mut *conn);
        assert!(result.is_ok(), "Should be able to execute a simple query");
    }
} 
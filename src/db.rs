use diesel::query_dsl::methods::LoadQuery;
/// Database connection module
///
/// This module provides functionality for creating and managing database connections
/// using Diesel's r2d2 connection pooling. It abstracts away the details of
/// connection management to provide a simple interface for the rest of the application.
use diesel::sqlite::SqliteConnection;
use diesel::r2d2::{Pool, ConnectionManager};
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use diesel::query_dsl::load_dsl::ExecuteDsl;
use diesel::RunQueryDsl;
use std::time::Duration;
use tokio::time::sleep;

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
    Pool::builder().build(manager).expect("Failed to create DB pool.")
}

// Constants for retry configuration
const INITIAL_DELAY_MS: u64 = 100;
const MAX_RETRIES: u32 = 5;

/// Checks if a Diesel error is likely temporary and worth retrying.
fn is_retryable_error(err: &DieselError) -> bool {
    match err {
        DieselError::DatabaseError(kind, info) => {
            match kind {
                // Explicitly retryable kinds
                DatabaseErrorKind::SerializationFailure => true,
                // For SQLite, "database is locked" or "busy" often comes as Unknown.
                // We check the specific error message provided by the database driver.
                DatabaseErrorKind::Unknown => {
                    let message = info.message().to_lowercase();
                    message.contains("database is locked") || message.contains("database busy")
                }
                _ => false, // Other database errors are not considered retryable by default
            }
        }
        
        // Most other top-level Diesel errors (like ConnectionError, QueryBuilderError, NotFound)
        // are generally not transient or recoverable by simple retries.
        // CouldntGetConnection is handled by the pool usually.
        _ => false,
    }
}

/// An async trait for executing Diesel queries with automatic retries on transient errors.
pub trait ExecuteWithRetry: RunQueryDsl<SqliteConnection> + ExecuteDsl<SqliteConnection> + Clone + Send + Sync + 'static {
    /// Executes the query, retrying with exponential backoff if a transient error occurs.
    ///
    /// ### Arguments
    ///
    /// * `conn` - A mutable reference to the SQLite connection.
    ///
    /// ### Returns
    ///
    /// A `Result` containing the number of affected rows on success, or a `DieselError`
    /// if the operation fails after exhausting retries or encounters a non-retryable error.
    ///
    /// ### Panics
    ///
    /// This function itself doesn't panic, but the underlying `execute` call might
    /// depending on the Diesel operation.
    async fn execute_with_retry(&self, conn: &mut SqliteConnection) -> Result<usize, DieselError> {
        let mut attempts = 0;
        let mut delay = Duration::from_millis(INITIAL_DELAY_MS);

        loop {
            // Clone the query builder for this attempt, as `execute` consumes it.
            let query_clone = self.clone();

            // Execute the query
            let result = query_clone.execute(conn);

            match result {
                Ok(rows_affected) => return Ok(rows_affected),
                Err(e) => {
                    if attempts >= MAX_RETRIES || !is_retryable_error(&e) {
                        // If max retries reached or error is not retryable, return the error
                        return Err(e);
                    }
                    // Log the retry attempt (optional, but good practice)
                    // tracing::warn!(error = ?e, attempt = attempts + 1, delay_ms = delay.as_millis(), "Query failed, retrying after delay...");

                    attempts += 1;
                    sleep(delay).await;
                    delay *= 2; // Double the delay for the next attempt
                }
            }
        }
    }
}

// Automatically implement the trait for any type that meets the bounds.
impl<T> ExecuteWithRetry for T where T: RunQueryDsl<SqliteConnection> + ExecuteDsl<SqliteConnection> + Clone + Send + Sync + 'static {}

/// An async trait for executing Diesel queries with automatic retries on transient errors.
pub trait LoadWithRetry<'a>: RunQueryDsl<SqliteConnection> + Clone + Send + Sync + 'static {
    /// Loads the query results, retrying with exponential backoff if a transient error occurs.
    ///
    /// ### Arguments
    ///
    /// * `conn` - A mutable reference to the SQLite connection.
    ///
    /// ### Returns
    ///
    /// A `Result` containing the loaded results on success, or a `DieselError`
    /// if the operation fails after exhausting retries or encounters a non-retryable error.
    ///
    /// ### Panics
    ///
    /// This function itself doesn't panic, but the underlying `load` call might
    /// depending on the Diesel operation.
    async fn load_with_retry<U>(&self, conn: &mut SqliteConnection) -> Result<Vec<U>, DieselError>
    where
        Self: LoadQuery<'a, SqliteConnection, U>;
}

impl<'a, T> LoadWithRetry<'a> for T
where
    T: RunQueryDsl<SqliteConnection> + Clone + Send + Sync + 'static,
{
    async fn load_with_retry<U>(&self, conn: &mut SqliteConnection) -> Result<Vec<U>, DieselError>
    where
        Self: LoadQuery<'a, SqliteConnection, U>,
    {
        let mut attempts = 0;
        let mut delay = Duration::from_millis(INITIAL_DELAY_MS);

        loop {
            // Clone the query builder for this attempt, as `load` consumes it.
            let query_clone = self.clone();

            // Execute the query
            let result = query_clone.load(conn);

            match result {
                Ok(loaded_results) => return Ok(loaded_results),
                Err(e) => {
                    if attempts >= MAX_RETRIES || !is_retryable_error(&e) {
                        // If max retries reached or error is not retryable, return the error
                        return Err(e);
                    }
                    // Log the retry attempt (optional, but good practice)
                    // tracing::warn!(error = ?e, attempt = attempts + 1, delay_ms = delay.as_millis(), "Query failed, retrying after delay...");

                    attempts += 1;
                    sleep(delay).await;
                    delay *= 2; // Double the delay for the next attempt
                }
            }
        }
    }
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
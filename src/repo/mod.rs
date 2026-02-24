/// Repository module
///
/// This module provides the data access layer for the application.
/// It contains functions for interacting with the database, including
/// creating, retrieving, and updating items and reviews.
/// 
/// The repository pattern abstracts away the details of database access
/// and provides a clean API for the rest of the application to use.

mod item_type_repo;
mod item_repo;
mod card_repo;
mod tag_repo;
mod review_repo;

// Re-export all repository functions
pub use item_type_repo::*;
pub use item_repo::*;
pub use card_repo::*;
pub use tag_repo::*;
pub use review_repo::*;

#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use crate::db::{self, DbPool};
    use diesel::connection::SimpleConnection;
    use diesel_migrations::MigrationHarness;
    
    /// Sets up a test database with migrations applied
    ///
    /// This function:
    /// 1. Creates an in-memory SQLite database
    /// 2. Enables foreign key constraints
    /// 3. Runs all migrations to set up the schema
    ///
    /// ### Returns
    ///
    /// A database connection pool connected to the in-memory database
    pub fn setup_test_db() -> Arc<DbPool> {
        // Use a unique shared in-memory database for each test.
        // Plain ":memory:" gives each connection its own separate database,
        // so migrations run on one connection wouldn't be visible on others.
        // By using a unique URI with cache=shared, all connections in this pool
        // share the same in-memory database while remaining isolated from other tests.
        let unique_id = uuid::Uuid::new_v4();
        let database_url = format!("file:test_{}?mode=memory&cache=shared", unique_id);
        let pool = db::init_pool(&database_url);
        
        // Run migrations on the in-memory database
        let mut conn = pool.get().expect("Failed to get connection");
        
        // Enable foreign key constraints for SQLite
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        
        // Run all migrations to set up the schema
        let migrations = diesel_migrations::FileBasedMigrations::find_migrations_directory().expect("Failed to find migrations directory");
        conn.run_pending_migrations(migrations).expect("Failed to run migrations");
        
        Arc::new(pool)
    }
} 
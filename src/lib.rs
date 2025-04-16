/// Hippocampus: A Spaced Repetition System Library
///
/// This library provides the core functionality for a spaced repetition system,
/// including data models, database access, and a web API.
///
/// The name "Hippocampus" refers to the part of the brain involved in memory formation,
/// which is fitting for a spaced repetition system designed to help with memorization.
///
/// ### Modules
///
/// - `db`: Database connection management
/// - `models`: Data structures representing items and reviews
/// - `repo`: Repository layer for database operations
/// - `schema`: Database schema definitions
///
/// ### Web API
///
/// The library exposes a RESTful API using Axum with the following endpoints:
///
/// Routes for item types:
/// - GET /item_types: List all item types (handlers::list_item_types_handler)
/// - POST /item_types: Create a new item type (handlers::create_item_type_handler)
/// - GET /item_types/{id}: Get a specific item type (handlers::get_item_type_handler)
/// - GET /item_types/{id}/items: List all items of a specific type (handlers::list_items_by_item_type_handler)
///
/// Routes for items:
/// - GET /items: List all items (handlers::list_items_handler)
/// - POST /items: Create a new item (handlers::create_item_handler)
/// - GET /items/{id}: Get a specific item (handlers::get_item_handler)
/// - GET /items/{id}/cards: List all cards for an item (handlers::list_cards_by_item_handler)
/// - POST /items/{id}/cards: Create a new card for an item (handlers::create_card_handler)
/// - GET /items/{item_id}/tags: List all tags for an item (handlers::list_tags_for_item_handler)
/// - PUT /items/{item_id}/tags/{tag_id}: Add a tag to an item (handlers::add_tag_to_item_handler)
/// - DELETE /items/{item_id}/tags/{tag_id}: Remove a tag from an item (handlers::remove_tag_from_item_handler)
///
/// Routes for cards:
/// - GET /cards: List all cards (handlers::list_cards_handler)
/// - GET /cards/{id}: Get a specific card (handlers::get_card_handler)
/// - GET /cards/{card_id}/reviews: List all reviews for a card (handlers::list_reviews_by_card_handler)
/// - PUT /cards/{card_id}/priority: Update the priority of a card (handlers::update_card_priority_handler)
/// - GET /cards/{card_id}/tags: List all tags for a card (handlers::list_tags_for_card_handler)
///
/// Routes for reviews:
/// - POST /reviews: Create a new review (handlers::create_review_handler)
///
/// Routes for tags:
/// - GET /tags: List all tags (handlers::list_tags_handler)
/// - POST /tags: Create a new tag (handlers::create_tag_handler)

/// Database connection module
pub mod db;

/// Data models module
pub mod models;

/// Repository module for database operations
pub mod repo;

/// Database schema module
pub mod schema;

/// API handlers module
pub mod handlers;

/// API errors module
pub mod errors;

/// Data transfer objects module
pub mod dto;

use axum::{
    routing::{get, post, put}, Router
};
use std::sync::Arc;

pub use dto::*;
pub use errors::ApiError;

/// Creates the application router with all routes
///
/// This function sets up the Axum router with all the API endpoints.
///
/// ### Arguments
///
/// * `pool` - The database connection pool to be shared with all handlers
///
/// ### Returns
///
/// An Axum Router configured with all routes and the database pool as state
pub fn create_app(pool: Arc<db::DbPool>) -> Router {
    Router::new()
        // Routes for item types
        .route("/item_types", post(handlers::create_item_type_handler).get(handlers::list_item_types_handler))
        .route("/item_types/{item_type_id}", get(handlers::get_item_type_handler))
        .route("/item_types/{item_type_id}/items", get(handlers::list_items_by_item_type_handler))
        
        // Routes for items
        .route("/items", post(handlers::create_item_handler).get(handlers::list_items_handler))
        .route("/items/{item_id}", get(handlers::get_item_handler))
        .route("/items/{item_id}/cards", post(handlers::create_card_handler).get(handlers::list_cards_by_item_handler))
        .route("/items/{item_id}/tags", get(handlers::list_tags_for_item_handler))
        .route("/items/{item_id}/tags/{tag_id}", put(handlers::add_tag_to_item_handler).delete(handlers::remove_tag_from_item_handler))
        
        // Routes for cards
        .route("/cards", get(handlers::list_cards_handler))
        .route("/cards/{card_id}", get(handlers::get_card_handler))
        .route("/cards/{card_id}/reviews", get(handlers::list_reviews_by_card_handler))
        .route("/cards/{card_id}/priority", put(handlers::update_card_priority_handler))
        .route("/cards/{card_id}/tags", get(handlers::list_tags_for_card_handler))
        .route("/cards/{card_id}/suspend", post(handlers::suspend_card_handler))
        
        // Routes for reviews
        .route("/reviews", post(handlers::create_review_handler))
        
        // Routes for tags
        .route("/tags", post(handlers::create_tag_handler).get(handlers::list_tags_handler))
        
        // Add the database pool to the application state
        .with_state(pool)
}

/// Runs the embedded migrations
///
/// This function applies all database migrations to set up the schema. Note that this is currently only used in tests.
///
/// ### Arguments
///
/// * `conn` - A mutable reference to a SQLite connection
///
/// ### Panics
///
/// This function will panic if the migrations fail to run
pub fn run_migrations(conn: &mut diesel::SqliteConnection) {
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    
    // Define the embedded migrations
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
    
    // Run all pending migrations
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
}


/// Enum representing the type of backup
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BackupType {
    Startup,
    Periodic,
}

/// Backs up the SQLite database file
///
/// This function creates a backup of the database if it's a local file (not an in-memory database).
/// The backup is stored in a folder called `backups` at the same level as the database file.
///
/// ### Arguments
///
/// * `database_path` - The path to the database file
/// * `backup_type` - The type of backup: Startup or Periodic
///
/// ### Returns
///
/// A `Result` indicating success or failure, with an error message on failure
///
/// ### Notes
///
/// Startup backups are all kept, but there will be at most five periodic backups at any time.
pub fn backup_database(database_path: &str, backup_type: BackupType) -> Result<(), String> {
    // Skip if using in-memory database
    if database_path == ":memory:" {
        return Ok(());
    }

    use std::fs::{self, File};
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tracing::{debug, info};

    // Get the directory containing the database
    let db_path = Path::new(database_path);
    if !db_path.exists() {
        // Database doesn't exist yet, nothing to back up
        debug!("Database file doesn't exist yet, skipping backup");
        return Ok(());
    }

    // Create backup directory if it doesn't exist
    let backup_dir = if let Some(parent) = db_path.parent() {
        parent.join("backups")
    } else {
        PathBuf::from("backups")
    };

    if !backup_dir.exists() {
        debug!("Creating backup directory: {:?}", backup_dir);
        fs::create_dir_all(&backup_dir).map_err(|e| format!("Failed to create backup directory: {}", e))?;
    }

    // Generate timestamp for backup file
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs();

    // Generate backup file name with db name and timestamp
    let db_filename = db_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("database.db");

    let backup_path = backup_dir.join(format!("{}.{}.{}.backup", db_filename, match backup_type { BackupType::Startup => "startup", BackupType::Periodic => "periodic" }, timestamp));
    
    debug!("Creating backup at {:?}", backup_path);

    // TODO: this really should be thread locked so no other operations are happening on the database during backup
    // but I'm not sure how to do that
    // so instead we're accepting that some of the 20-minute backups will be corrupted and this is better than nothing

    // Copy the database file to the backup location
    let mut src = File::open(database_path).map_err(|e| format!("Failed to open database: {}", e))?;
    let mut dst = File::create(&backup_path).map_err(|e| format!("Failed to create backup file: {}", e))?;
    
    let mut buffer = Vec::new();
    src.read_to_end(&mut buffer).map_err(|e| format!("Failed to read database: {}", e))?;
    dst.write_all(&buffer).map_err(|e| format!("Failed to write backup: {}", e))?;
    
    info!("Database backup created at {:?}", backup_path);

    // If this is a periodic backup, check if we need to clean up old backups
    if backup_type == BackupType::Periodic {
        cleanup_periodic_backups(&backup_dir, db_filename)?;
    }

    Ok(())
}

/// Cleans up old periodic backups, keeping only the 5 most recent
///
/// ### Arguments
///
/// * `backup_dir` - Path to the backup directory
/// * `db_filename` - Base filename of the database
///
/// ### Returns
///
/// A `Result` indicating success or failure, with an error message on failure
fn cleanup_periodic_backups(backup_dir: &std::path::Path, db_filename: &str) -> Result<(), String> {
    use std::fs;
    use std::time::SystemTime;
    use tracing::{debug, error};

    // Find all periodic backups for this database
    let pattern = format!("{}.periodic.", db_filename);
    
    let mut backups = match fs::read_dir(backup_dir) {
        Ok(entries) => {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    if let Some(name) = entry.file_name().to_str() {
                        name.contains(&pattern)
                    } else {
                        false
                    }
                })
                .collect::<Vec<_>>()
        }
        Err(e) => {
            error!("Failed to read backup directory: {}", e);
            return Err(format!("Failed to read backup directory: {}", e));
        }
    };

    // Sort backups by modification time (newest first)
    backups.sort_by(|a, b| {
        let time_a = a.metadata().and_then(|m| m.modified()).unwrap_or_else(|_| SystemTime::UNIX_EPOCH);
        let time_b = b.metadata().and_then(|m| m.modified()).unwrap_or_else(|_| SystemTime::UNIX_EPOCH);
        time_b.cmp(&time_a)
    });

    // Keep only the 5 newest backups
    const MAX_PERIODIC_BACKUPS: usize = 5;
    
    if backups.len() > MAX_PERIODIC_BACKUPS {
        for old_backup in backups.iter().skip(MAX_PERIODIC_BACKUPS) {
            debug!("Removing old backup: {:?}", old_backup.path());
            if let Err(e) = fs::remove_file(old_backup.path()) {
                error!("Failed to remove old backup {:?}: {}", old_backup.path(), e);
                // Continue with other deletions even if one fails
            }
        }
    }

    Ok(())
}


/// Starts a background task to periodically back up the database
///
/// This function spawns a background task that backs up the database every 20 minutes.
///
/// ### Arguments
///
/// * `database_path` - The path to the database file
///
/// ### Notes
///
/// This should only be called once at application startup.
pub fn start_periodic_backup(database_path: String) {
    // Skip for in-memory databases
    if database_path == ":memory:" {
        return;
    }

    use std::time::Duration;
    use tokio::time;
    use tracing::{info, error};

    // Clone the database path for the async task
    let db_path = database_path.clone();

    // Spawn a background task for periodic backups
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(20 * 60)); // 20 minutes

        info!("Starting periodic database backup task (every 20 minutes)");
        
        loop {
            // Wait for the next interval tick
            interval.tick().await;

            // Perform the backup
            info!("Performing periodic database backup");

            if let Err(e) = backup_database(&db_path, BackupType::Periodic) {
                error!("Periodic backup failed: {}", e);
            }
        }
    });
}


#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use chrono::Utc;
    use diesel::connection::SimpleConnection;
    use diesel::{SqliteConnection, RunQueryDsl, Connection};
    use serde_json::Value;
    use std::sync::Arc;
    use tower::ServiceExt;

    /// Sets up a test database with migrations applied
    ///
    /// This function:
    /// 1. Creates an in-memory SQLite database
    /// 2. Enables foreign key constraints
    /// 3. Runs all migrations to set up the schema
    ///
    /// ### Returns
    ///
    /// An Arc-wrapped database connection pool connected to the in-memory database
    pub fn setup_test_db() -> Arc<db::DbPool> {
        // Use an in-memory database for testing
        let database_url = ":memory:";
        let pool = db::init_pool(database_url);
        
        // Get a connection from the pool
        let mut conn = pool.get().expect("Failed to get connection");
        
        // Enable foreign key constraints for SQLite
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        
        // Run all migrations to set up the schema
        run_migrations(&mut conn);
        
        // Wrap the pool in an Arc for thread-safe sharing
        Arc::new(pool)
    }


    use diesel::sql_types::Text;
    use diesel::QueryableByName;

    #[derive(QueryableByName, Debug)]
    struct TableName {
        #[diesel(sql_type = Text)]
        name: String,
    }

    /// Tests the setup_test_db function
    ///
    /// This test verifies that:
    /// 1. The test database can be created and connected to
    /// 2. The database has the expected tables
    /// 3. The database can be queried successfully
    #[tokio::test]
    async fn test_setup_test_db() {
        let pool = setup_test_db();
        assert!(pool.get().is_ok());

        // Check that all migrations were run, i.e. the tables were created
        let mut conn = pool.get().unwrap();
        let result = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table'")
            .execute(&mut conn);
        assert!(result.is_ok());
        
        println!("Result: {:?}", result);

        // Get the names of the tables
        let table_names: Vec<TableName> = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table'")
            .load(&mut conn)
            .expect("Failed to load table names");
        
        println!("Tables: {:?}", table_names);
        
        // Verify that we have the expected tables
        assert!(table_names.len() > 0, "No tables found in the database");

        // test interacting with each of the found tables
        let expected_tables = vec![
            "cards", "item_tags", "item_types", "items", "reviews", "tags", 
            "__diesel_schema_migrations" // Diesel's migration tracking table
        ];
        
        for table in expected_tables {
            let exists = table_names.iter().any(|t| t.name == table);
            assert!(exists, "Table '{}' not found in database", table);
            
            // Test a simple query on each table
            let query = format!("SELECT COUNT(*) FROM {}", table);
            let result = diesel::sql_query(&query).execute(&mut conn);
            assert!(result.is_ok(), "Failed to query table '{}': {:?}", table, result.err());
            
            println!("Table '{}' exists and is queryable", table);
        }

        drop(conn);

        // test interacting with the app
        let app = create_app(pool.clone());

        // test interacting with the item_types table
        let request = Request::builder()
            .uri("/item_types")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        // send the request to the app
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "Response status is not OK (err: {:?})", axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap());
    }
    
    
    /// Tests the create item handler
    ///
    /// This test verifies that:
    /// 1. A POST request to /items creates a new item
    /// 2. The response has a 200 OK status
    /// 3. The response body contains the created item with the correct title
    #[tokio::test]
    async fn test_create_item_handler() {
        // Set up a test database and application
        let pool = setup_test_db();
        let app = create_app(pool.clone());
        
        // Create an item type first
        let item_type = repo::create_item_type(&pool, "Test Item Type".to_string()).await.unwrap();
        
        // Create a request with a JSON body
        let request = Request::builder()
            .uri("/items")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"item_type_id":"{}","title":"Test Item","item_data":null}}"#, item_type.get_id())))
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response status
        assert_eq!(response.status(), StatusCode::OK, "Response status is not OK (err: {:?})", axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap());
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let item: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response contains the correct item
        assert_eq!(item["title"], "Test Item");
        assert!(item["id"].is_string());
    }
    

    /// Tests the update card priority handler - successful update
    ///
    /// This test verifies that:
    /// 1. A PUT request to /cards/{card_id}/priority updates the priority of a card
    /// 2. The response has a 200 OK status
    /// 3. The response body contains the updated card with the correct priority
    #[tokio::test]
    async fn test_update_card_priority_handler_success() {
        // Set up a test database
        let pool = setup_test_db();
        let app = create_app(pool.clone());
        
        // Create test data
        let item_type = repo::create_item_type(&pool, "Test Item Type".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            serde_json::json!({"front": "Hello", "back": "World"})
        ).await.unwrap();
        
        // Create a card with initial priority
        let initial_priority = 0.5;
        let card = repo::create_card(&pool, &item.get_id(), 2, initial_priority).await.unwrap();
        
        // Create a request to update the card's priority
        let new_priority = 0.8;
        let request = Request::builder()
            .uri(format!("/cards/{}/priority", card.get_id()))
            .method("PUT")
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"priority":{}}}"#, new_priority)))
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response status
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let updated_card: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response contains the card with updated priority
        assert_eq!(updated_card["id"], card.get_id());
        assert!((updated_card["priority"].as_f64().unwrap() - new_priority).abs() < 0.0001);
    }
    

    /// Tests the update card priority handler - boundary values
    ///
    /// This test verifies that:
    /// 1. The handler correctly processes minimum (0.0) and maximum (1.0) priority values
    #[tokio::test]
    async fn test_update_card_priority_handler_boundary_values() {
        // Set up a test database
        let pool = setup_test_db();
        let app = create_app(pool.clone());
        
        // Create test data
        let item_type = repo::create_item_type(&pool, "Test Item Type".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            serde_json::json!({"front": "Hello", "back": "World"})
        ).await.unwrap();
        
        let card = repo::create_card(&pool, &item.get_id(), 2, 0.5).await.unwrap();
        
        // Test minimum valid priority (0.0)
        let min_priority = 0.0;
        let request = Request::builder()
            .uri(format!("/cards/{}/priority", card.get_id()))
            .method("PUT")
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"priority":{}}}"#, min_priority)))
            .unwrap();
        
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let updated_card: Value = serde_json::from_slice(&body).unwrap();
        assert!((updated_card["priority"].as_f64().unwrap() - min_priority).abs() < 0.0001);
        
        // Test maximum valid priority (1.0)
        let max_priority = 1.0;
        let request = Request::builder()
            .uri(format!("/cards/{}/priority", card.get_id()))
            .method("PUT")
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"priority":{}}}"#, max_priority)))
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let updated_card: Value = serde_json::from_slice(&body).unwrap();
        assert!((updated_card["priority"].as_f64().unwrap() - max_priority).abs() < 0.0001);
    }
    

    /// Tests the update card priority handler - card not found
    ///
    /// This test verifies that:
    /// 1. The handler returns a 404 Not Found status when the card ID doesn't exist
    #[tokio::test]
    async fn test_update_card_priority_handler_not_found() {
        // Set up a test database
        let pool = setup_test_db();
        let app = create_app(pool.clone());
        
        // Create a request with a non-existent card ID
        let request = Request::builder()
            .uri("/cards/nonexistent-id/priority")
            .method("PUT")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"priority":0.7}"#))
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check that we get a 404 Not Found status
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    

    /// Tests the update card priority handler - invalid priority value
    ///
    /// This test verifies that:
    /// 1. The handler returns a 400 Bad Request status when the priority is outside the valid range
    #[tokio::test]
    async fn test_update_card_priority_handler_invalid_priority() {
        // Set up a test database
        let pool = setup_test_db();
        let app = create_app(pool.clone());
        
        // Create test data
        let item_type = repo::create_item_type(&pool, "Test Item Type".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            serde_json::json!({"front": "Hello", "back": "World"})
        ).await.unwrap();
        
        let card = repo::create_card(&pool, &item.get_id(), 2, 0.5).await.unwrap();
        
        // Test with priority > 1.0 (invalid)
        let invalid_priority = 1.5;
        let request = Request::builder()
            .uri(format!("/cards/{}/priority", card.get_id()))
            .method("PUT")
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"priority":{}}}"#, invalid_priority)))
            .unwrap();
        
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        
        // Test with priority < 0.0 (invalid)
        let invalid_priority = -0.5;
        let request = Request::builder()
            .uri(format!("/cards/{}/priority", card.get_id()))
            .method("PUT")
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"priority":{}}}"#, invalid_priority)))
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }


    /// Tests the list items handler
    ///
    /// This test verifies that:
    /// 1. A GET request to /items returns all items
    /// 2. The response has a 200 OK status
    /// 3. The response body contains all the expected items
    #[tokio::test]
    async fn test_list_items_handler() {
        // Set up a test database
        let pool = setup_test_db();

        let item_type = repo::create_item_type(&pool, "Test Item Type".to_string()).await.unwrap();
        
        // Create a few items first
        let titles = vec!["Item 1", "Item 2", "Item 3"];
        for title in &titles {
            repo::create_item(&pool, &item_type.get_id(), title.to_string(), serde_json::Value::Null).await.unwrap();
        }
        
        // Create the application
        let app = create_app(pool.clone());
        
        // Create a GET request
        let request = Request::builder()
            .uri("/items")
            .method("GET")
            .body(Body::empty())
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response status
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let items: Vec<Value> = serde_json::from_slice(&body).unwrap();
        
        // Verify the response contains the correct number of items
        assert_eq!(items.len(), titles.len());
        
        // Check that all titles are present in the response
        let item_titles: Vec<String> = items.iter()
            .map(|item| item["title"].as_str().unwrap().to_string())
            .collect();
        
        for title in titles {
            assert!(item_titles.contains(&title.to_string()));
        }
    }

    
    /// Tests the get item handler
    ///
    /// This test verifies that:
    /// 1. A GET request to /items/{id} returns the specific item
    /// 2. The response has a 200 OK status
    /// 3. The response body contains the expected item
    #[tokio::test]
    async fn test_get_item_handler() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item first
        let title = "Item to Get".to_string();
        let item_type = repo::create_item_type(&pool, "Test Item Type".to_string()).await.unwrap();
        let item = repo::create_item(&pool, &item_type.get_id(), title.clone(), serde_json::Value::Null).await.unwrap();
        
        // Create the application
        let app = create_app(pool.clone());
        
        // Create a GET request with the item ID in the path
        let request = Request::builder()
            .uri(format!("/items/{}", item.get_id()))
            .method("GET")
            .body(Body::empty())
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response status
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let response_item: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response contains the correct item
        assert_eq!(response_item["id"], item.get_id());
        assert_eq!(response_item["title"], title);
    }
    

    /// Tests the create review handler
    ///
    /// This test verifies that:
    /// 1. A POST request to /reviews creates a new review
    /// 2. The response has a 200 OK status
    /// 3. The response body contains the created review
    /// 4. The item is updated with review information
    #[tokio::test]
    async fn test_create_review_handler() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item first
        let title = "Item to Review".to_string();
        let item_type = repo::create_item_type(&pool, "Test Item Type".to_string()).await.unwrap();
        let item = repo::create_item(&pool, &item_type.get_id(), title.clone(), serde_json::Value::Null).await.unwrap();
        let cards = repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
        let card = cards.first().unwrap();
        
        // Create the application
        let app = create_app(pool.clone());
        
        // Create a request with a JSON body containing the item ID and rating
        let request = Request::builder()
            .uri("/reviews")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"card_id":"{}","rating":3}}"#, card.get_id())))
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response status
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let review: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response contains the correct review
        assert_eq!(review["card_id"], card.get_id());
        assert_eq!(review["rating"], 3);
        assert!(review["id"].is_string());
        
        // Check that the item was updated with review information
        let updated_card = repo::get_card(&pool, &card.get_id()).unwrap().unwrap();
        assert!(updated_card.get_last_review().is_some());
        assert!(updated_card.get_next_review() > Utc::now());
    }
    

    /// Tests the run_migrations function
    ///
    /// This test verifies that:
    /// 1. Migrations can be run successfully
    /// 2. The expected tables are created in the database
    #[test]
    fn test_run_migrations() {
        // Create a connection to an in-memory database
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        
        // Run migrations
        run_migrations(&mut conn);
        
        // Verify that the tables were created by querying the schema
        let result = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table' AND name='items'")
            .execute(&mut conn);
        assert!(result.is_ok());
        
        let result = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table' AND name='reviews'")
            .execute(&mut conn);
        assert!(result.is_ok());
    }

    
    /// Tests the create item type handler
    ///
    /// This test verifies that:
    /// 1. A POST request to /item_types creates a new item type
    /// 2. The response has a 200 OK status
    /// 3. The response body contains the created item type with the correct name
    #[tokio::test]
    async fn test_create_item_type_handler() {
        // Set up a test database and application
        let pool = setup_test_db();
        let app = create_app(pool.clone());
        
        // Create a request with a JSON body
        let request = Request::builder()
            .uri("/item_types")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"name":"Test Item Type"}"#))
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response status
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let item_type: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response contains the correct item type
        assert_eq!(item_type["name"], "Test Item Type");
        assert!(item_type["id"].is_string());
    }
    

    /// Tests the get item type handler
    ///
    /// This test verifies that:
    /// 1. A GET request to /item_types/{id} returns the specific item type
    /// 2. The response has a 200 OK status
    /// 3. The response body contains the expected item type
    #[tokio::test]
    async fn test_get_item_type_handler() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item type first
        let name = "Item Type to Get".to_string();
        let item_type = repo::create_item_type(&pool, name.clone()).await.unwrap();
        
        // Create the application
        let app = create_app(pool.clone());
        
        // Create a GET request with the item type ID in the path
        let request = Request::builder()
            .uri(format!("/item_types/{}", item_type.get_id()))
            .method("GET")
            .body(Body::empty())
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response status
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let response_item_type: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response contains the correct item type
        assert_eq!(response_item_type["id"], item_type.get_id());
        assert_eq!(response_item_type["name"], name);
    }


    /// Tests the database backup functionality 
    ///
    /// This test verifies that:
    /// 1. Backups can be created successfully for an existing database
    /// 2. In-memory databases are skipped appropriately
    /// 3. The backup directory is created if it doesn't exist
    #[test]
    fn test_database_backup() {
        use std::fs::{self, File};
        use std::io::Write;
        use std::thread::sleep;
        use std::time::Duration;
        
        // Create a temporary test database file
        let test_db_dir = tempfile::tempdir().expect("Failed to create temp directory");
        let test_db_path = test_db_dir.path().join("test_backup.db");
        
        // Create a dummy database file with some content
        {
            let mut file = File::create(&test_db_path).expect("Failed to create test database file");
            file.write_all(b"Test database content").expect("Failed to write to test database");
        }
        
        // Test backup creation
        let db_path_str = test_db_path.to_str().unwrap();
        let result = super::backup_database(db_path_str, BackupType::Startup);
        assert!(result.is_ok(), "Backup should succeed");
        
        // Check that the backup directory was created
        let backup_dir = test_db_dir.path().join("backups");
        assert!(backup_dir.exists(), "Backup directory should exist");
        
        // Check that a backup file was created
        let entries = fs::read_dir(&backup_dir).expect("Failed to read backup directory");
        let backup_count = entries.count();
        assert!(backup_count > 0, "At least one backup file should exist");
        
        // Test in-memory database
        let result = super::backup_database(":memory:", BackupType::Startup);
        assert!(result.is_ok(), "In-memory database backup should be skipped successfully");
        
        // Test cleanup of periodic backups
        
        // Create multiple periodic backups
        let test_backups = 7; // More than MAX_PERIODIC_BACKUPS
        for _ in 0..test_backups {
            // Add a longer delay to ensure file modification times are distinct
            // (because file timestamps are only accurate to the second, having a delay of less than a second will cause the backup to be overwritten)
            // (and we double that to two seconds to be guaranteed that the backups will have different timestamps)
            sleep(Duration::from_millis(2000));
            let _ = super::backup_database(db_path_str, BackupType::Periodic);
        }
        
        // Check for periodic backups
        let periodic_backups: Vec<_> = fs::read_dir(&backup_dir)
            .expect("Failed to read backup directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_name().to_str()
                    .map(|name| name.contains("periodic"))
                    .unwrap_or(false)
            })
            .collect();
        
        println!("Periodic backups: {:?}", 
            periodic_backups.iter()
                .map(|entry| entry.file_name())
                .collect::<Vec<_>>()
        );
        
        assert_eq!(periodic_backups.len(), 5, "Should keep exactly 5 periodic backups");
    }
}

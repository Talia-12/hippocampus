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
/// - `POST /items`: Create a new item
/// - `GET /items`: List all items
/// - `GET /items/{id}`: Get a specific item by ID
/// - `POST /reviews`: Record a review for an item

/// Database connection module
pub mod db;

/// Data models module
pub mod models;

/// Repository module for database operations
pub mod repo;

/// Database schema module
pub mod schema;

use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{State, Path},
    response::{IntoResponse, Response},
    http::StatusCode,
};
use models::{Item, Review};
use serde::Deserialize;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Database error: {0}")]
    Database(#[from] anyhow::Error),
    #[error("Item not found")]
    NotFound,
    #[error("Invalid rating: {0}")]
    InvalidRating(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Database(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "Item not found".to_string()),
            ApiError::InvalidRating(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        let body = Json(serde_json::json!({
            "error": message
        }));

        (status, body).into_response()
    }
}

/// Data transfer object for creating a new item
///
/// This struct is used to deserialize JSON requests for creating items.
#[derive(Deserialize)]
pub struct CreateItemDto {
    /// The title or content of the item to be remembered
    pub item_type_id: String,
    pub title: String,
    pub item_data: serde_json::Value,
}

/// Data transfer object for creating a new review
///
/// This struct is used to deserialize JSON requests for recording reviews.
#[derive(Deserialize)]
pub struct CreateReviewDto {
    /// The ID of the card being reviewed
    pub card_id: String,
    
    /// The rating given during the review (typically 1-3)
    pub rating: i32,
}

/// Data transfer object for creating a new item type
///
/// This struct is used to deserialize JSON requests for creating item types.
#[derive(Deserialize)]
pub struct CreateItemTypeDto {
    /// The name of the item type
    pub name: String,
}

/// Data transfer object for creating a new card
///
/// This struct is used to deserialize JSON requests for creating cards.
#[derive(Deserialize)]
pub struct CreateCardDto {
    /// The index of the card within its item
    pub card_index: i32,
}

/// Handler for creating a new item
///
/// This function handles POST requests to `/items`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `payload` - The request payload containing the item title
///
/// ### Returns
///
/// The newly created item as JSON
async fn create_item_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateItemDto>,
) -> Result<Json<Item>, ApiError> {
    // Call the repository function to create the item
    let item = repo::create_item(&pool, &payload.item_type_id, payload.title, payload.item_data)
        .map_err(ApiError::Database)?;

    // Return the created item as JSON
    Ok(Json(item))
}

/// Handler for retrieving a specific item
///
/// This function handles GET requests to `/items/{id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The ID of the item to retrieve, extracted from the URL path
///
/// ### Returns
///
/// The requested item as JSON, or null if not found
async fn get_item_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
    // Extract the item ID from the URL path
    Path(item_id): Path<String>,
) -> Result<Json<Option<Item>>, ApiError> {
    // Call the repository function to get the item
    let item = repo::get_item(&pool, &item_id)
        .map_err(ApiError::Database)?;
    // Return the item (or None) as JSON
    Ok(Json(item))
}

/// Handler for listing all items
///
/// This function handles GET requests to `/items`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
///
/// ### Returns
///
/// A list of all items as JSON
async fn list_items_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
) -> Result<Json<Vec<Item>>, ApiError> {
    // Call the repository function to list all items
    let all_items = repo::list_items(&pool)
        .map_err(ApiError::Database)?;
    // Return the list of items as JSON
    Ok(Json(all_items))
}


/// Handler for recording a review
///
/// This function handles POST requests to `/reviews`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `payload` - The request payload containing the item ID and rating
///
/// ### Returns
///
/// The newly created review as JSON
async fn create_review_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateReviewDto>,
) -> Result<Json<Review>, ApiError> {
    // Validate rating range
    if !(1..=3).contains(&payload.rating) {
        return Err(ApiError::InvalidRating(
            "Rating must be between 1 and 3".to_string()
        ));
    }
    
    // First check if the item exists
    let item_exists = repo::get_card(&pool, &payload.card_id)
        .map_err(ApiError::Database)?
        .is_some();
    
    if !item_exists {
        return Err(ApiError::NotFound);
    }
    
    let review = repo::record_review(&pool, &payload.card_id, payload.rating)
        .map_err(ApiError::Database)?;
    // Return the created review as JSON
    Ok(Json(review))
}

/// Handler for creating a new item type
///
/// This function handles POST requests to `/item_types`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `payload` - The request payload containing the item type name
///
/// ### Returns
///
/// The newly created item type as JSON
async fn create_item_type_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateItemTypeDto>,
) -> Result<Json<models::ItemType>, ApiError> {
    // Call the repository function to create the item type
    let item_type = repo::create_item_type(&pool, payload.name)
        .map_err(ApiError::Database)?;

    // Return the created item type as JSON
    Ok(Json(item_type))
}

/// Handler for retrieving a specific item type
///
/// This function handles GET requests to `/item_types/{id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `id` - The ID of the item type to retrieve, extracted from the URL path
///
/// ### Returns
///
/// The requested item type as JSON, or null if not found
async fn get_item_type_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
    // Extract the item type ID from the URL path
    Path(id): Path<String>,
) -> Result<Json<Option<models::ItemType>>, ApiError> {
    // Call the repository function to get the item type
    let item_type = repo::get_item_type(&pool, &id)
        .map_err(ApiError::Database)?;
    // Return the item type (or None) as JSON
    Ok(Json(item_type))
}

/// Handler for listing all item types
///
/// This function handles GET requests to `/item_types`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
///
/// ### Returns
///
/// A list of all item types as JSON
async fn list_item_types_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
) -> Result<Json<Vec<models::ItemType>>, ApiError> {
    // Call the repository function to list all item types
    let all_item_types = repo::list_item_types(&pool)
        .map_err(ApiError::Database)?;
    // Return the list of item types as JSON
    Ok(Json(all_item_types))
}

/// Handler for listing items by item type
///
/// This function handles GET requests to `/item_types/{id}/items`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `id` - The ID of the item type to filter by, extracted from the URL path
///
/// ### Returns
///
/// A list of items of the specified type as JSON
async fn list_items_by_item_type_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
    // Extract the item type ID from the URL path
    Path(id): Path<String>,
) -> Result<Json<Vec<models::Item>>, ApiError> {
    // First check if the item type exists
    let item_type_exists = repo::get_item_type(&pool, &id)
        .map_err(ApiError::Database)?
        .is_some();
    
    if !item_type_exists {
        return Err(ApiError::NotFound);
    }
    
    // Call the repository function to get items by type
    let items = repo::get_items_by_type(&pool, &id)
        .map_err(ApiError::Database)?;
    // Return the list of items as JSON
    Ok(Json(items))
}

/// Handler for creating a new card for an item
///
/// This function handles POST requests to `/items/{id}/cards`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The ID of the item to create a card for, extracted from the URL path
/// * `payload` - The request payload containing the card index
///
/// ### Returns
///
/// The newly created card as JSON
async fn create_card_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
    // Extract the item ID from the URL path
    Path(item_id): Path<String>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateCardDto>,
) -> Result<Json<models::Card>, ApiError> {
    // First check if the item exists
    let item_exists = repo::get_item(&pool, &item_id)
        .map_err(ApiError::Database)?
        .is_some();
    
    if !item_exists {
        return Err(ApiError::NotFound);
    }
    
    // Call the repository function to create the card
    let card = repo::create_card(&pool, &item_id, payload.card_index)
        .map_err(ApiError::Database)?;
    // Return the created card as JSON
    Ok(Json(card))
}

/// Handler for retrieving a specific card
///
/// This function handles GET requests to `/cards/{id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `id` - The ID of the card to retrieve, extracted from the URL path
///
/// ### Returns
///
/// The requested card as JSON, or null if not found
async fn get_card_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
    // Extract the card ID from the URL path
    Path(id): Path<String>,
) -> Result<Json<Option<models::Card>>, ApiError> {
    // Call the repository function to get the card
    let card = repo::get_card(&pool, &id)
        .map_err(ApiError::Database)?;
    // Return the card (or None) as JSON
    Ok(Json(card))
}

/// Handler for listing all cards
///
/// This function handles GET requests to `/cards`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
///
/// ### Returns
///
/// A list of all cards as JSON
async fn list_cards_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
) -> Result<Json<Vec<models::Card>>, ApiError> {
    // Call the repository function to list all cards
    let all_cards = repo::list_cards(&pool)
        .map_err(ApiError::Database)?;
    // Return the list of cards as JSON
    Ok(Json(all_cards))
}

/// Handler for listing cards by item
///
/// This function handles GET requests to `/items/{id}/cards`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The ID of the item to get cards for, extracted from the URL path
///
/// ### Returns
///
/// A list of cards for the specified item as JSON
async fn list_cards_by_item_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<db::DbPool>>,
    // Extract the item ID from the URL path
    Path(item_id): Path<String>,
) -> Result<Json<Vec<models::Card>>, ApiError> {
    // First check if the item exists
    let item_exists = repo::get_item(&pool, &item_id)
        .map_err(ApiError::Database)?
        .is_some();
    
    if !item_exists {
        return Err(ApiError::NotFound);
    }
    
    // Call the repository function to get cards for the item
    let cards = repo::get_cards_for_item(&pool, &item_id)
        .map_err(ApiError::Database)?;
    // Return the list of cards as JSON
    Ok(Json(cards))
}

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
        // Route for creating an item type
        .route("/item_types", post(create_item_type_handler).get(list_item_types_handler))
        // Route for getting a specific item type by ID
        .route("/item_types/{id}", get(get_item_type_handler))
        // Route for listing items by item type
        .route("/item_types/{id}/items", get(list_items_by_item_type_handler))
        // Route for creating and listing items
        .route("/items", post(create_item_handler).get(list_items_handler))
        // Route for getting a specific item by ID
        .route("/items/{id}", get(get_item_handler))
        // Route for creating a card for an item
        .route("/items/{id}/cards", post(create_card_handler).get(list_cards_by_item_handler))
        // Route for getting a specific card by ID
        .route("/cards/{id}", get(get_card_handler).get(list_cards_handler))
        // Route for recording reviews
        .route("/reviews", post(create_review_handler))
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use diesel::connection::SimpleConnection;
    use diesel::{SqliteConnection, RunQueryDsl, Connection};
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    use serde_json::Value;
    use std::sync::Arc;
    use tower::ServiceExt;
    
    /// Embedded migrations for testing
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
    
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
    fn setup_test_db() -> Arc<db::DbPool> {
        // Use an in-memory database for testing
        let database_url = ":memory:";
        let pool = db::init_pool(database_url);
        
        // Run migrations on the in-memory database
        let mut conn = pool.get().expect("Failed to get connection");
        
        // Enable foreign key constraints for SQLite
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        
        // Run all migrations to set up the schema
        conn.run_pending_migrations(MIGRATIONS).expect("Failed to run migrations");
        
        // Wrap the pool in an Arc for thread-safe sharing
        Arc::new(pool)
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
        
        // Create a request with a JSON body
        let request = Request::builder()
            .uri("/items")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"title":"Test Item"}"#))
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response status
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let item: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response contains the correct item
        assert_eq!(item["title"], "Test Item");
        assert!(item["id"].is_string());
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

        let item_type = repo::create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        
        // Create a few items first
        let titles = vec!["Item 1", "Item 2", "Item 3"];
        for title in &titles {
            repo::create_item(&pool, &item_type.id, title.to_string(), serde_json::Value::Null).unwrap();
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
        let item_type = repo::create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item = repo::create_item(&pool, &item_type.id, title.clone(), serde_json::Value::Null).unwrap();
        
        // Create the application
        let app = create_app(pool.clone());
        
        // Create a GET request with the item ID in the path
        let request = Request::builder()
            .uri(format!("/items/{}", item.id))
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
        assert_eq!(response_item["id"], item.id);
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
        let item_type = repo::create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item = repo::create_item(&pool, &item_type.id, title.clone(), serde_json::Value::Null).unwrap();
        let card = repo::create_card(&pool, &item.id, 0).unwrap();
        
        // Create the application
        let app = create_app(pool.clone());
        
        // Create a request with a JSON body containing the item ID and rating
        let request = Request::builder()
            .uri("/reviews")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"card_id":"{}","rating":3}}"#, card.id)))
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response status
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let review: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response contains the correct review
        assert_eq!(review["card_id"], item.id);
        assert_eq!(review["rating"], 3);
        assert!(review["id"].is_string());
        
        // Check that the item was updated with review information
        let updated_card = repo::get_card(&pool, &card.id).unwrap().unwrap();
        assert!(updated_card.last_review.is_some());
        assert!(updated_card.next_review.is_some());
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
}

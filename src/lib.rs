pub mod db;
pub mod models;
pub mod repo;
pub mod schema;

use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{State, Path},
};
use models::{Item, Review};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct CreateItemDto {
    pub title: String,
}

#[derive(Deserialize)]
pub struct CreateReviewDto {
    pub item_id: String,
    pub rating: i32,
}

async fn create_item_handler(
    State(pool): State<Arc<db::DbPool>>,
    Json(payload): Json<CreateItemDto>,
) -> Json<Item> {
    let item = repo::create_item(&pool, payload.title)
        .expect("Failed to create item");
    Json(item)
}

async fn get_item_handler(
    State(pool): State<Arc<db::DbPool>>,
    Path(item_id): Path<String>,
) -> Json<Option<Item>> {
    let item = repo::get_item(&pool, &item_id)
        .expect("Failed to retrieve item");
    Json(item)
}

async fn list_items_handler(
    State(pool): State<Arc<db::DbPool>>,
) -> Json<Vec<Item>> {
    let all_items = repo::list_items(&pool)
        .expect("Failed to list items");
    Json(all_items)
}

async fn create_review_handler(
    State(pool): State<Arc<db::DbPool>>,
    Json(payload): Json<CreateReviewDto>,
) -> Json<Review> {
    let review = repo::record_review(&pool, &payload.item_id, payload.rating)
        .expect("Failed to record review");
    Json(review)
}

/// Creates the application router with all routes
pub fn create_app(pool: Arc<db::DbPool>) -> Router {
    Router::new()
        .route("/items", post(create_item_handler).get(list_items_handler))
        .route("/items/{id}", get(get_item_handler))
        .route("/reviews", post(create_review_handler))
        .with_state(pool)
}

/// Runs the embedded migrations
pub fn run_migrations(conn: &mut diesel::SqliteConnection) {
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
    
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
    
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
    
    fn setup_test_db() -> Arc<db::DbPool> {
        let database_url = ":memory:";
        let pool = db::init_pool(database_url);
        
        // Run migrations on the in-memory database
        let mut conn = pool.get().expect("Failed to get connection");
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        conn.run_pending_migrations(MIGRATIONS).expect("Failed to run migrations");
        
        Arc::new(pool)
    }
    
    #[tokio::test]
    async fn test_create_item_handler() {
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
        
        // Check the response
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let item: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response
        assert_eq!(item["title"], "Test Item");
        assert!(item["id"].is_string());
    }
    
    #[tokio::test]
    async fn test_list_items_handler() {
        let pool = setup_test_db();
        
        // Create a few items first
        let titles = vec!["Item 1", "Item 2", "Item 3"];
        for title in &titles {
            repo::create_item(&pool, title.to_string()).unwrap();
        }
        
        let app = create_app(pool.clone());
        
        // Create a GET request
        let request = Request::builder()
            .uri("/items")
            .method("GET")
            .body(Body::empty())
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let items: Vec<Value> = serde_json::from_slice(&body).unwrap();
        
        // Verify the response
        assert_eq!(items.len(), titles.len());
        
        // Check that all titles are present
        let item_titles: Vec<String> = items.iter()
            .map(|item| item["title"].as_str().unwrap().to_string())
            .collect();
        
        for title in titles {
            assert!(item_titles.contains(&title.to_string()));
        }
    }
    
    #[tokio::test]
    async fn test_get_item_handler() {
        let pool = setup_test_db();
        
        // Create an item first
        let title = "Item to Get".to_string();
        let item = repo::create_item(&pool, title.clone()).unwrap();
        
        let app = create_app(pool.clone());
        
        // Create a GET request
        let request = Request::builder()
            .uri(format!("/items/{}", item.id))
            .method("GET")
            .body(Body::empty())
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let response_item: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response
        assert_eq!(response_item["id"], item.id);
        assert_eq!(response_item["title"], title);
    }
    
    #[tokio::test]
    async fn test_create_review_handler() {
        let pool = setup_test_db();
        
        // Create an item first
        let title = "Item to Review".to_string();
        let item = repo::create_item(&pool, title).unwrap();
        
        let app = create_app(pool.clone());
        
        // Create a request with a JSON body
        let request = Request::builder()
            .uri("/reviews")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"item_id":"{}","rating":3}}"#, item.id)))
            .unwrap();
        
        // Send the request to the app
        let response = app.oneshot(request).await.unwrap();
        
        // Check the response
        assert_eq!(response.status(), StatusCode::OK);
        
        // Parse the response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let review: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify the response
        assert_eq!(review["item_id"], item.id);
        assert_eq!(review["rating"], 3);
        assert!(review["id"].is_string());
        
        // Check that the item was updated
        let updated_item = repo::get_item(&pool, &item.id).unwrap().unwrap();
        assert!(updated_item.last_review.is_some());
        assert!(updated_item.next_review.is_some());
    }
    
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

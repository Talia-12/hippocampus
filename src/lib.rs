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

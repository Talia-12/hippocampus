mod db;
mod models;
mod repo;
mod schema;

use axum::{
	routing::{get, post},
	Router,
	Json,
	extract::{State, Path},
};
use dotenv::dotenv;
use models::Item;
use serde::Deserialize;
use std::{env, sync::Arc, net::SocketAddr};

#[derive(Deserialize)]
struct CreateItemDto {
	title: String,
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

#[tokio::main]
async fn main() {
	// Initialize logging
	tracing_subscriber::fmt::init();

	// Load environment variables
	dotenv().ok();
	let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
	
	// Initialize the database pool
	let pool = Arc::new(db::init_pool(&database_url));
	
	// Build our application with routes
	let app = Router::new()
		.route("/items", post(create_item_handler).get(list_items_handler))
		.route("/items/:id", get(get_item_handler))
		.with_state(pool);

	// Run it
	let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
	println!("Listening on {}", addr);
	axum::Server::bind(&addr)
		.serve(app.into_make_service())
		.await
		.unwrap();
}

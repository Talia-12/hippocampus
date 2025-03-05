use axum::{
	routing::{get, post},
	Router,
	Json,
	extract::{State, Path},
};
use models::{Item, Review};
use serde::Deserialize;
use hippocampus::*;
use std::{env, sync::Arc, net::SocketAddr};
use tracing_subscriber;

#[tokio::main]
async fn main() {
	// Initialize logging
	tracing_subscriber::fmt::init();

	// Load environment variables
	if std::fs::metadata(".env").is_ok() {
		println!("Loading .env file");
		dotenv::dotenv().ok();
	}
	
	let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "srs_server.db".to_string());
	
	// Initialize the database pool
	let pool = Arc::new(db::init_pool(&database_url));
	
	// Build our application with routes
	let app = create_app(pool);

	// Run it
	let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
	println!("Listening on {}", addr);
	
	println!("Starting server, press Ctrl+C to stop");
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	axum::serve(listener, app).await.unwrap();
}

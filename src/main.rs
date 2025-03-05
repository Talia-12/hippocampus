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

#[cfg(test)]
mod tests {
	#[test]
	fn test_env_variables() {
		// Test that the DATABASE_URL fallback works
		// Save the original value if it exists
		let original_value = std::env::var("DATABASE_URL").ok();
		
		// Remove the var for first test
		unsafe { std::env::remove_var("DATABASE_URL") };
		let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "srs_server.db".to_string());
		assert_eq!(database_url, "srs_server.db");
		
		// Test with a custom value
		unsafe { std::env::set_var("DATABASE_URL", "test.db") };
		let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "srs_server.db".to_string());
		assert_eq!(database_url, "test.db");
		
		// Restore original value or remove if there wasn't one
		match original_value {
			Some(value) => unsafe { std::env::set_var("DATABASE_URL", value) },
			None => unsafe { std::env::remove_var("DATABASE_URL") },
		}
	}
	
	#[test]
	fn test_socket_addr_creation() {
		let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
		assert_eq!(addr.ip().to_string(), "127.0.0.1");
		assert_eq!(addr.port(), 3000);
	}
}

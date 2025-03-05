/// Hippocampus: A Spaced Repetition System
///
/// This is the main entry point for the Hippocampus application.
/// It initializes the database, sets up the web server, and starts
/// listening for incoming requests.
///
/// The application provides a RESTful API for managing items and reviews
/// in a spaced repetition system, which helps users memorize information
/// more effectively by scheduling reviews at optimal intervals.
use hippocampus::*;
use std::{env, sync::Arc, net::SocketAddr};
use tracing_subscriber;

/// Main function - entry point of the application
///
/// This async function:
/// 1. Initializes logging
/// 2. Loads environment variables
/// 3. Sets up the database connection pool
/// 4. Creates the web application
/// 5. Starts the web server
#[tokio::main]
async fn main() {
	// Initialize logging for better debugging and monitoring
	tracing_subscriber::fmt::init();

	// Load environment variables from .env file if it exists
	if std::fs::metadata(".env").is_ok() {
		println!("Loading .env file");
		dotenv::dotenv().ok();
	}
	
	// Get the database URL from environment variables or use a default
	let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "srs_server.db".to_string());
	
	// Initialize the database connection pool
	// This pool will be shared across all request handlers
	let pool = Arc::new(db::init_pool(&database_url));
	
	// Build our application with routes
	// This sets up all the API endpoints
	let app = create_app(pool);

	// Define the address to listen on (localhost:3000)
	let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
	println!("Listening on {}", addr);
	
	// Start the server and wait for connections
	println!("Starting server, press Ctrl+C to stop");
	
	// Create a TCP listener bound to the specified address
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	
	// Start serving requests
	// This will run until the program is terminated
	axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
	
	/// Tests environment variable handling
	///
	/// This test verifies that:
	/// 1. The DATABASE_URL fallback works when the environment variable is not set
	/// 2. The environment variable is correctly read when it is set
	#[test]
	fn test_env_variables() {
		// Test that the DATABASE_URL fallback works
		unsafe {
			std::env::remove_var("DATABASE_URL");
		}
		let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "srs_server.db".to_string());
		assert_eq!(database_url, "srs_server.db");
		
		// Test with a custom value
		unsafe {
			std::env::set_var("DATABASE_URL", "test.db");
		}
		let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "srs_server.db".to_string());
		assert_eq!(database_url, "test.db");
		
		// Clean up
		unsafe {
			std::env::remove_var("DATABASE_URL");
		}
	}
	
	/// Tests socket address creation
	///
	/// This test verifies that:
	/// 1. A socket address can be created with the correct IP and port
	/// 2. The IP and port can be correctly extracted from the address
	#[test]
	fn test_socket_addr_creation() {
		// Create a socket address for localhost:3000
		let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
		
		// Verify the IP address is correct
		assert_eq!(addr.ip().to_string(), "127.0.0.1");
		
		// Verify the port is correct
		assert_eq!(addr.port(), 3000);
	}
}

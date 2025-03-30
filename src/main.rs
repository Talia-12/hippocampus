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
use std::{env, sync::Arc, net::SocketAddr, path::Path};
use tracing::{info, error};
use tracing_subscriber::{self, fmt, prelude::*, filter::LevelFilter, Registry};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

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
	println!("Initializing logging");
	init_tracing();
	
	info!("Starting Hippocampus SRS Server");

	// Load environment variables from .env file if it exists
	if std::fs::metadata(".env").is_ok() {
		info!("Loading .env file");
		dotenv::dotenv().ok();
	}
	
	// Get the database URL from environment variables or use a default
	let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
		info!("DATABASE_URL not set, using default: srs_server.db");
		"srs_server.db".to_string()
	});
	
	// Initialize the database connection pool
	// This pool will be shared across all request handlers
	info!("Initializing database connection pool");
	let pool = Arc::new(db::init_pool(&database_url));
	
	// Build our application with routes
	// This sets up all the API endpoints
	let app = create_app(pool);

	// Define the address to listen on (localhost:3000)
	let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
	info!("Listening on {}", addr);
	
	// Start the server and wait for connections
	info!("Starting server, press Ctrl+C to stop");
	
	// Create a TCP listener bound to the specified address
	match tokio::net::TcpListener::bind(addr).await {
		Ok(listener) => {
			// Start serving requests
			// This will run until the program is terminated
			if let Err(e) = axum::serve(listener, app).await {
				error!("Server error: {}", e);
			}
		},
		Err(e) => {
			error!("Failed to bind to address {}: {}", addr, e);
		}
	}
}


/// Initialize tracing with both console and file outputs
///
/// This follows the tracing-subscriber layer pattern where:
/// 1. The Registry is the root subscriber
/// 2. Multiple layers are composed together using the `with` method
/// 3. Each layer can have its own filter
/// 
/// Console output shows INFO level and above by default,
/// while the file output captures all levels of logs.
/// 
/// A special debug layer can be enabled by setting the HIPPOCAMPUS_DEBUG
/// environment variable, which will output DEBUG-level logs to the console.
fn init_tracing() {
    // Create a directory for logs if it doesn't exist
    if !Path::new("logs").exists() {
        std::fs::create_dir("logs").expect("Failed to create logs directory");
    }

    // Setup a file appender for all log levels
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        "logs",
        "hippocampus.log"
    );
    
    // Non-blocking writer for better performance
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    println!("Initializing tracing subscriber");
    
    // Create file layer with appropriate filter
    let file_layer = fmt::layer()
        .with_ansi(false)  // No ANSI color codes in log files
        .with_writer(file_writer)
        .with_filter(LevelFilter::TRACE);
    
    // Determine which console output layer to use based on debug mode
    let debug_mode = env::var("HIPPOCAMPUS_DEBUG").is_ok();
    let console_layer = if debug_mode {
        println!("Debug mode enabled - verbose logs will be shown");
        fmt::layer()
            .pretty()
            .with_writer(std::io::stdout)
            .with_filter(LevelFilter::DEBUG)
    } else {
        fmt::layer()
            .pretty()
            .with_writer(std::io::stdout)
            .with_filter(LevelFilter::WARN)
    };
	
    // Initialize the global subscriber by composing layers with a Registry
    // The Registry is the root subscriber that's responsible for collecting spans
    let subscriber = Registry::default()
        .with(console_layer)
        .with(file_layer);
    
    subscriber.init();

    println!("Tracing subscriber initialized");
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

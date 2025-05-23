/// Hippocampus: A Spaced Repetition System
///
/// This is the main entry point for the Hippocampus application.
/// It initializes the database, sets up the web server, and starts
/// listening for incoming requests.
///
/// The application provides a RESTful API for managing items and reviews
/// in a spaced repetition system, which helps users memorize information
/// more effectively by scheduling reviews at optimal intervals.
use hippocampus::{config::CliArgs, *};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tracing::{info, error};
use tracing_subscriber::{self, fmt, prelude::*, filter::LevelFilter, Registry};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use clap::Parser;

/// Main function - entry point of the application
///
/// This async function:
/// 1. Initializes logging
/// 2. Loads configuration
/// 3. Sets up the database connection pool
/// 4. Creates the web application
/// 5. Starts the web server
#[tokio::main]
async fn main() {
	let args = CliArgs::parse();

	// Initialize logging for better debugging and monitoring
	println!("Initializing logging");
	let _guard = init_tracing(args.debug);
	
	info!("Starting Hippocampus SRS Server");

	// Load environment variables from .env file if it exists
	if std::fs::metadata(".env").is_ok() {
		info!("Loading .env file");
		dotenv::dotenv().ok();
	}
	
	// Load configuration from all sources
	let config = config::get_config(args).unwrap_or_else(|e| {
		error!("Failed to load configuration: {}", e);
		panic!("Failed to load configuration: {}", e);
	});
	
	info!("Using database at {}", config.database_url);

	// Backup the database if it is a local file
	info!("Checking if database backup is needed");
	match backup_database(&config.database_url, BackupType::Startup, config.backup_count) {
		Ok(_) => info!("Database backup completed successfully"),
		Err(e) => {
			error!("Database backup failed: {}", e);

			// We want to panic here because if there is a problem with the database backup
			// that we didn't know about, there could be other problems we don't know about
			// and our backup system won't save us from data loss.
			panic!("Database backup failed: {}", e);
		}
	}
	
	// Start periodic backup task
	info!("Starting periodic backup task");
	start_periodic_backup(config.database_url.clone(), config.backup_interval(), config.backup_count);
	
	// Initialize the database connection pool
	// This pool will be shared across all request handlers
	info!("Initializing database connection pool");
	let pool = Arc::new(db::init_pool(&config.database_url));
	
	// Build our application with routes
	// This sets up all the API endpoints
	let app = create_app(pool);

	// Define the address to listen on (localhost:3000), or (localhost:3001) if we are running in debug mode
	let addr = if cfg!(debug_assertions) {
		SocketAddr::from(([127, 0, 0, 1], 3001))
	} else {
		SocketAddr::from(([127, 0, 0, 1], 3000))
	};
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
fn init_tracing(debug: bool) -> impl Drop {
	// If the config dir path is not None, we should do our logging in there
	let config_dir_path = config::get_config_dir_path();

	let log_dir_path = config_dir_path.map(|path| path.join("logs")).unwrap_or_else(|| PathBuf::from("logs"));

    // Create a directory for logs if it doesn't exist
    if !log_dir_path.exists() {
        std::fs::create_dir(log_dir_path.clone()).expect("Failed to create logs directory");
    }

    // Setup a file appender for all log levels
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        log_dir_path,
        "hippocampus.log"
    );
    
    // Non-blocking writer for better performance
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    println!("Initializing tracing subscriber");
    
    // Create file layer with appropriate filter
    let file_layer = fmt::layer()
        .with_ansi(false)  // No ANSI color codes in log files
        .with_writer(file_writer)
        .with_filter(LevelFilter::TRACE);
    
    // Determine which console output layer to use based on debug mode
    let console_layer = if debug {
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
    
    // Return the guard so it stays alive for the program's duration
    guard
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

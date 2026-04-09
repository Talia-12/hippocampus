/// Integration tests for database creation and migration on startup
///
/// These tests verify that the server binary:
/// - Creates a new database file if one doesn't exist
/// - Runs embedded migrations against the database
/// - Serves requests successfully after setup
use diesel::prelude::*;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

const SERVER_ADDR: &str = "127.0.0.1:3001";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Waits for the server to be ready by polling the TCP port.
///
/// ### Arguments
///
/// * `addr` - The address to connect to
/// * `timeout` - Maximum time to wait
///
/// ### Returns
///
/// `true` if the server became ready, `false` if the timeout was reached
fn wait_for_server(addr: &str, timeout: Duration) -> bool {
	let start = Instant::now();
	while start.elapsed() < timeout {
		if TcpStream::connect(addr).is_ok() {
			return true;
		}
		thread::sleep(POLL_INTERVAL);
	}
	false
}

/// Sends a minimal HTTP GET request and returns the status code and body.
///
/// ### Arguments
///
/// * `addr` - The server address (host:port)
/// * `path` - The request path (e.g. "/item_types")
///
/// ### Returns
///
/// A tuple of (status_code, body_string)
fn http_get(addr: &str, path: &str) -> (u16, String) {
	let mut stream = TcpStream::connect(addr).expect("Failed to connect to server");
	stream
		.set_read_timeout(Some(Duration::from_secs(5)))
		.unwrap();

	let request = format!("GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", path, addr);
	stream.write_all(request.as_bytes()).unwrap();

	let mut response = String::new();
	stream.read_to_string(&mut response).unwrap_or(0);

	// Parse status code from first line (e.g. "HTTP/1.1 200 OK")
	let status = response
		.lines()
		.next()
		.and_then(|line| line.split_whitespace().nth(1))
		.and_then(|code| code.parse::<u16>().ok())
		.unwrap_or(0);

	// Body is after the blank line
	let body = response
		.split("\r\n\r\n")
		.nth(1)
		.unwrap_or("")
		.to_string();

	(status, body)
}

/// Helper to ensure the child process is killed on drop
struct ServerGuard(Child);

impl Drop for ServerGuard {
	fn drop(&mut self) {
		let _ = self.0.kill();
		let _ = self.0.wait();
	}
}

/// Tests that the server creates a new database and runs migrations when
/// pointed at a non-existent database file.
///
/// This test:
/// 1. Creates a temp directory and derives a database path that doesn't exist yet
/// 2. Launches the server binary with `--database-url` pointing to that path
/// 3. Waits for the server to start
/// 4. Verifies the database file was created
/// 5. Opens the database and verifies expected tables exist
/// 6. Makes an HTTP request to confirm the server is functional
/// 7. Kills the server process (cleanup is automatic via TempDir and ServerGuard)
#[test]
fn test_server_creates_database_and_runs_migrations() {
	let tmp_dir = tempfile::tempdir().expect("Failed to create temp directory");
	let db_path = tmp_dir.path().join("test_hippocampus.db");

	// Verify the database file does not exist yet
	assert!(
		!db_path.exists(),
		"Database file should not exist before server starts"
	);

	// Launch the server binary
	let bin_path = assert_cmd::cargo::cargo_bin!("hippocampus");
	let child = Command::new(bin_path)
		.arg("--database-url")
		.arg(&db_path)
		.spawn()
		.expect("Failed to spawn server process");

	let _guard = ServerGuard(child);

	// Wait for the server to be ready
	assert!(
		wait_for_server(SERVER_ADDR, STARTUP_TIMEOUT),
		"Server did not start within {:?}",
		STARTUP_TIMEOUT
	);

	// Verify the database file was created
	assert!(
		db_path.exists(),
		"Database file should have been created by the server"
	);

	// Open the database and verify expected tables exist
	let mut conn = SqliteConnection::establish(db_path.to_str().unwrap())
		.expect("Failed to connect to the created database");

	let expected_tables = vec!["items", "reviews", "item_types", "cards", "tags"];
	for table_name in &expected_tables {
		let result: Vec<TableName> = diesel::sql_query(format!(
			"SELECT name FROM sqlite_master WHERE type='table' AND name='{}'",
			table_name
		))
		.load::<TableName>(&mut conn)
		.unwrap_or_default();

		assert!(
			result.len() == 1,
			"Expected table '{}' to exist in the database, but it was not found",
			table_name
		);
	}

	// Make an HTTP request to verify the server is functional
	let (status, body) = http_get(SERVER_ADDR, "/item_types");

	assert_eq!(status, 200, "Expected 200 OK from /item_types endpoint");

	let parsed: serde_json::Value =
		serde_json::from_str(&body).expect("Failed to parse response as JSON");
	assert!(
		parsed.is_array(),
		"Expected JSON array response from /item_types"
	);
}

/// Helper struct for deserializing table name query results
#[derive(QueryableByName)]
#[allow(dead_code)]
struct TableName {
	#[diesel(sql_type = diesel::sql_types::Text)]
	name: String,
}

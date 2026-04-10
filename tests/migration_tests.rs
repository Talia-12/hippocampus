/// Integration tests for database creation and migration on startup
///
/// These tests verify that the server binary:
/// - Creates a new database file if one doesn't exist
/// - Runs embedded migrations against the database
/// - Serves requests successfully after setup
mod common;

use common::{SERVER_ADDR, STARTUP_TIMEOUT, ServerGuard, http_get, wait_for_server};
use diesel::prelude::*;
use std::process::Command;

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
		.env("HOME", tmp_dir.path())
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

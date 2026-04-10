use super::*;
use std::fs::File;
use std::io::Write;
use tempfile::{TempDir, tempdir};

/// Helper function to create a test configuration file
fn create_test_config_file(dir: &TempDir, content: &str) -> PathBuf {
	let config_path = dir.path().join("config.toml");
	let mut file = File::create(&config_path).unwrap();
	file.write_all(content.as_bytes()).unwrap();
	config_path
}

// ============================================================================
// ConfigBuilder::merge tests
// ============================================================================

#[test]
fn test_merge_with_all_values() {
	let base = ConfigBuilder {
		database_url: Some("original.db".to_string()),
		backup_interval_minutes: Some(30),
		backup_count: Some(5),
		..ConfigBuilder::default()
	};

	let other = ConfigBuilder {
		database_url: Some("updated.db".to_string()),
		backup_interval_minutes: Some(60),
		backup_count: Some(10),
		..ConfigBuilder::default()
	};

	let merged = base.merge(other);

	assert_eq!(merged.database_url, Some("updated.db".to_string()));
	assert_eq!(merged.backup_interval_minutes, Some(60));
	assert_eq!(merged.backup_count, Some(10));
}

#[test]
fn test_merge_with_partial_values() {
	let base = ConfigBuilder {
		database_url: Some("original.db".to_string()),
		backup_interval_minutes: Some(30),
		backup_count: Some(5),
		..ConfigBuilder::default()
	};

	let other = ConfigBuilder {
		database_url: Some("updated.db".to_string()),
		..ConfigBuilder::default()
	};

	let merged = base.merge(other);

	assert_eq!(merged.database_url, Some("updated.db".to_string()));
	assert_eq!(merged.backup_interval_minutes, Some(30)); // Preserved
	assert_eq!(merged.backup_count, Some(5)); // Preserved
}

#[test]
fn test_merge_with_no_values() {
	let base = ConfigBuilder {
		database_url: Some("original.db".to_string()),
		backup_interval_minutes: Some(30),
		backup_count: Some(5),
		..ConfigBuilder::default()
	};

	let merged = base.merge(ConfigBuilder::default());

	assert_eq!(merged.database_url, Some("original.db".to_string()));
	assert_eq!(merged.backup_interval_minutes, Some(30));
	assert_eq!(merged.backup_count, Some(5));
}

#[test]
fn test_merge_preserves_dir_fields() {
	let config_path = PathBuf::from("/some/config");
	let data_path = PathBuf::from("/some/data");
	let state_path = PathBuf::from("/some/state");
	let base = ConfigBuilder {
		config_dir: Some(config_path.clone()),
		data_dir: Some(data_path.clone()),
		state_dir: Some(state_path.clone()),
		..ConfigBuilder::default()
	};

	// Merge with empty builder should preserve existing values
	let merged = base.merge(ConfigBuilder::default());

	assert_eq!(merged.config_dir, Some(config_path));
	assert_eq!(merged.data_dir, Some(data_path));
	assert_eq!(merged.state_dir, Some(state_path));
}

#[test]
fn test_merge_overrides_dir_fields() {
	let base = ConfigBuilder {
		config_dir: Some(PathBuf::from("/old/config")),
		data_dir: Some(PathBuf::from("/old/data")),
		state_dir: Some(PathBuf::from("/old/state")),
		..ConfigBuilder::default()
	};

	let new_data = PathBuf::from("/new/data");
	let other = ConfigBuilder {
		data_dir: Some(new_data.clone()),
		..ConfigBuilder::default()
	};

	let merged = base.merge(other);

	assert_eq!(merged.config_dir, Some(PathBuf::from("/old/config")));
	assert_eq!(merged.data_dir, Some(new_data));
	assert_eq!(merged.state_dir, Some(PathBuf::from("/old/state")));
}

// ============================================================================
// Config::backup_interval tests
// ============================================================================

#[test]
fn test_backup_interval_conversion() {
	let config = Config {
		database_url: "test.db".to_string(),
		backup_interval_minutes: 30,
		backup_count: 5,
		config_dir: None,
		data_dir: None,
		state_dir: None,
	};

	let duration = config.backup_interval();

	assert_eq!(duration, Duration::from_secs(30 * 60));
}

// ============================================================================
// ConfigBuilder::build tests
// ============================================================================

#[test]
fn test_build_defaults() {
	let config = ConfigBuilder::default().build();

	assert_eq!(config.database_url, DEFAULT_DATABASE_FILENAME);
	assert_eq!(
		config.backup_interval_minutes,
		DEFAULT_BACKUP_INTERVAL_MINUTES
	);
	assert_eq!(config.backup_count, DEFAULT_BACKUP_COUNT);
	assert_eq!(config.config_dir, None);
	// In debug builds, data_dir and state_dir resolve to None (no override given)
	if cfg!(debug_assertions) {
		assert_eq!(config.data_dir, None);
		assert_eq!(config.state_dir, None);
	}
}

#[test]
fn test_build_with_data_dir() {
	let temp_dir = tempdir().unwrap();
	let data_path = temp_dir.path().to_path_buf();
	let config = ConfigBuilder {
		data_dir: Some(data_path.clone()),
		..ConfigBuilder::default()
	}
	.build();

	let expected_db_path = temp_dir
		.path()
		.join(DEFAULT_DATABASE_FILENAME)
		.to_string_lossy()
		.to_string();
	assert_eq!(config.database_url, expected_db_path);
	assert_eq!(config.data_dir, Some(data_path));
}

#[test]
fn test_build_with_state_dir() {
	let temp_dir = tempdir().unwrap();
	let state_path = temp_dir.path().to_path_buf();
	let config = ConfigBuilder {
		state_dir: Some(state_path.clone()),
		..ConfigBuilder::default()
	}
	.build();

	assert_eq!(config.database_url, DEFAULT_DATABASE_FILENAME);
	assert_eq!(config.state_dir, Some(state_path));
}

#[test]
fn test_build_with_both_dirs() {
	let data_dir = tempdir().unwrap();
	let state_dir = tempdir().unwrap();
	let data_path = data_dir.path().to_path_buf();
	let state_path = state_dir.path().to_path_buf();
	let config = ConfigBuilder {
		data_dir: Some(data_path.clone()),
		state_dir: Some(state_path.clone()),
		..ConfigBuilder::default()
	}
	.build();

	let expected_db_path = data_dir
		.path()
		.join(DEFAULT_DATABASE_FILENAME)
		.to_string_lossy()
		.to_string();
	assert_eq!(config.database_url, expected_db_path);
	assert_eq!(config.data_dir, Some(data_path));
	assert_eq!(config.state_dir, Some(state_path));
}

#[test]
fn test_build_with_config_dir() {
	let config_dir = tempdir().unwrap();
	let config_path = config_dir.path().to_path_buf();
	let config = ConfigBuilder {
		config_dir: Some(config_path.clone()),
		..ConfigBuilder::default()
	}
	.build();

	assert_eq!(config.database_url, DEFAULT_DATABASE_FILENAME);
	assert_eq!(config.config_dir, Some(config_path));
}

#[test]
fn test_build_explicit_database_url_not_overridden_by_data_dir() {
	let temp_dir = tempdir().unwrap();
	let data_path = temp_dir.path().to_path_buf();
	let config = ConfigBuilder {
		database_url: Some("explicit.db".to_string()),
		data_dir: Some(data_path.clone()),
		..ConfigBuilder::default()
	}
	.build();

	assert_eq!(config.database_url, "explicit.db");
	assert_eq!(config.data_dir, Some(data_path));
}

// ============================================================================
// config_from_args tests
//
// These tests construct `CliArgs` directly, which includes the
// `debug_allow_path_override` field that only exists in debug builds.
// ============================================================================

#[cfg(debug_assertions)]
#[test]
fn test_config_from_args_with_all_values() {
	let args = CliArgs {
		database_url: Some("args.db".to_string()),
		backup_interval_minutes: Some(45),
		backup_count: Some(15),
		debug: true,
		config_dir: None,
		data_dir: None,
		state_dir: None,
		debug_allow_path_override: false,
	};

	let builder = config_from_args(args);

	assert_eq!(builder.database_url, Some("args.db".to_string()));
	assert_eq!(builder.backup_interval_minutes, Some(45));
	assert_eq!(builder.backup_count, Some(15));
}

#[cfg(debug_assertions)]
#[test]
fn test_config_from_args_with_partial_values() {
	let args = CliArgs {
		database_url: Some("args.db".to_string()),
		backup_interval_minutes: None,
		backup_count: None,
		debug: false,
		config_dir: None,
		data_dir: None,
		state_dir: None,
		debug_allow_path_override: false,
	};

	let builder = config_from_args(args);

	assert_eq!(builder.database_url, Some("args.db".to_string()));
	assert_eq!(builder.backup_interval_minutes, None);
	assert_eq!(builder.backup_count, None);
}

#[cfg(debug_assertions)]
#[test]
fn test_config_from_args_with_no_values() {
	let args = CliArgs {
		database_url: None,
		backup_interval_minutes: None,
		backup_count: None,
		debug: false,
		config_dir: None,
		data_dir: None,
		state_dir: None,
		debug_allow_path_override: false,
	};

	let builder = config_from_args(args);

	assert_eq!(builder.database_url, None);
	assert_eq!(builder.backup_interval_minutes, None);
	assert_eq!(builder.backup_count, None);
}

#[cfg(debug_assertions)]
#[test]
fn test_config_from_args_forwards_dir_fields() {
	let config_path = PathBuf::from("/my/config");
	let data_path = PathBuf::from("/my/data");
	let state_path = PathBuf::from("/my/state");

	let args = CliArgs {
		database_url: None,
		backup_interval_minutes: None,
		backup_count: None,
		debug: false,
		config_dir: Some(config_path.clone()),
		data_dir: Some(data_path.clone()),
		state_dir: Some(state_path.clone()),
		debug_allow_path_override: false,
	};

	let builder = config_from_args(args);

	assert_eq!(builder.config_dir, Some(config_path));
	assert_eq!(builder.data_dir, Some(data_path));
	assert_eq!(builder.state_dir, Some(state_path));
}

// ============================================================================
// config_from_file tests — successful cases
// ============================================================================

#[test]
fn test_config_from_file_with_no_path() {
	let result = config_from_file(None);

	assert!(result.is_ok());
	let builder = result.unwrap();
	assert_eq!(builder.database_url, None);
	assert_eq!(builder.backup_interval_minutes, None);
	assert_eq!(builder.backup_count, None);
}

#[test]
fn test_config_from_file_with_valid_toml() {
	let temp_dir = tempdir().unwrap();
	let config_content = r#"
        database_url = "file.db"
        backup_interval_minutes = 40
        backup_count = 20
    "#;

	let config_path = create_test_config_file(&temp_dir, config_content);

	let result = config_from_file(Some(config_path));

	assert!(
		result.is_ok(),
		"Failed to parse config file: {}",
		result.err().unwrap()
	);
	let builder = result.unwrap();
	assert_eq!(builder.database_url, Some("file.db".to_string()));
	assert_eq!(builder.backup_interval_minutes, Some(40));
	assert_eq!(builder.backup_count, Some(20));
}

#[test]
fn test_config_from_file_with_partial_values() {
	let temp_dir = tempdir().unwrap();
	let config_content = r#"
        database_url = "file.db"
        # Intentionally missing other fields
    "#;

	let config_path = create_test_config_file(&temp_dir, config_content);

	let result = config_from_file(Some(config_path));

	assert!(
		result.is_ok(),
		"Failed to parse config file: {}",
		result.err().unwrap()
	);
	let builder = result.unwrap();
	assert_eq!(builder.database_url, Some("file.db".to_string()));
	assert_eq!(builder.backup_interval_minutes, None);
	assert_eq!(builder.backup_count, None);
}

// ============================================================================
// config_from_file tests — failure cases
// ============================================================================

#[test]
fn test_config_from_file_with_invalid_toml() {
	let temp_dir = tempdir().unwrap();
	let config_content = r#"
        database_url = "file.db"
        backup_interval_minutes = "not a number" # Type error
    "#;

	let config_path = create_test_config_file(&temp_dir, config_content);

	let result = config_from_file(Some(config_path));

	assert!(result.is_err());
}

#[test]
fn test_config_from_file_with_nonexistent_file() {
	let temp_dir = tempdir().unwrap();
	let nonexistent_path = temp_dir.path().join("nonexistent_config.toml");

	let result = config_from_file(Some(nonexistent_path));

	assert!(result.is_ok());
	// Should return default values when file doesn't exist
	let builder = result.unwrap();
	assert_eq!(builder.database_url, None);
	assert_eq!(builder.backup_interval_minutes, None);
	assert_eq!(builder.backup_count, None);
}

// ============================================================================
// get_*_dir_path tests
//
// The debug_assertions guard now only blocks when no override is given.
// When an override is provided, the function proceeds (validation that
// overrides are allowed in debug mode happens in get_config, not here).
// ============================================================================

#[test]
fn test_get_config_dir_path_with_override() {
	let temp_dir = tempdir().unwrap();
	let override_path = temp_dir.path().to_path_buf();

	let result = get_config_dir_path(Some(override_path.clone()));

	// Override is respected regardless of debug/release
	assert_eq!(result, Some(override_path));
}

#[test]
fn test_get_config_dir_path_with_nonexistent_override() {
	let override_path = PathBuf::from("/nonexistent/config/dir");

	let result = get_config_dir_path(Some(override_path));

	// Config dir is not created — returns None if it doesn't exist
	assert_eq!(result, None);
}

#[test]
fn test_get_config_dir_path_without_override() {
	let result = get_config_dir_path(None);

	if cfg!(debug_assertions) {
		assert_eq!(result, None);
	}
}

#[test]
fn test_get_data_dir_path_with_override() {
	let temp_dir = tempdir().unwrap();
	let override_path = temp_dir.path().to_path_buf();

	let result = get_data_dir_path(Some(override_path.clone()));

	assert_eq!(result, Some(override_path));
}

#[test]
fn test_get_data_dir_path_creates_nonexistent_override() {
	let temp_dir = tempdir().unwrap();
	let override_path = temp_dir.path().join("new_data_dir");

	assert!(!override_path.exists());
	let result = get_data_dir_path(Some(override_path.clone()));

	// Data dir should be created
	assert_eq!(result, Some(override_path.clone()));
	assert!(override_path.exists());
}

#[test]
fn test_get_data_dir_path_uncreatable_override() {
	// /dev/null is a file, so creating a subdirectory under it will fail
	let override_path = PathBuf::from("/dev/null/impossible");

	let result = get_data_dir_path(Some(override_path));

	assert_eq!(result, None);
}

#[test]
fn test_get_data_dir_path_without_override() {
	let result = get_data_dir_path(None);

	if cfg!(debug_assertions) {
		assert_eq!(result, None);
	}
}

#[test]
fn test_get_state_dir_path_with_override() {
	let temp_dir = tempdir().unwrap();
	let override_path = temp_dir.path().to_path_buf();

	let result = get_state_dir_path(Some(override_path.clone()));

	assert_eq!(result, Some(override_path));
}

#[test]
fn test_get_state_dir_path_creates_nonexistent_override() {
	let temp_dir = tempdir().unwrap();
	let override_path = temp_dir.path().join("new_state_dir");

	assert!(!override_path.exists());
	let result = get_state_dir_path(Some(override_path.clone()));

	assert_eq!(result, Some(override_path.clone()));
	assert!(override_path.exists());
}

#[test]
fn test_get_state_dir_path_uncreatable_override() {
	let override_path = PathBuf::from("/dev/null/impossible");

	let result = get_state_dir_path(Some(override_path));

	assert_eq!(result, None);
}

#[test]
fn test_get_state_dir_path_without_override() {
	let result = get_state_dir_path(None);

	if cfg!(debug_assertions) {
		assert_eq!(result, None);
	}
}

// ============================================================================
// get_config tests — path overrides
//
// These tests construct `CliArgs` directly, which includes the
// `debug_allow_path_override` field that only exists in debug builds.
// ============================================================================

#[cfg(debug_assertions)]
#[test]
fn test_get_config_with_config_dir_override() {
	let config_dir = tempdir().unwrap();
	let config_content = r#"
        database_url = "from_config_file.db"
        backup_interval_minutes = 99
    "#;
	create_test_config_file(&config_dir, config_content);

	let args = CliArgs {
		database_url: None,
		backup_interval_minutes: None,
		backup_count: None,
		debug: false,
		config_dir: Some(config_dir.path().to_path_buf()),
		data_dir: None,
		state_dir: None,
		debug_allow_path_override: true,
	};

	let config = get_config(args).unwrap();

	assert_eq!(config.database_url, "from_config_file.db");
	assert_eq!(config.backup_interval_minutes, 99);
	assert_eq!(config.config_dir, Some(config_dir.path().to_path_buf()));
}

#[cfg(debug_assertions)]
#[test]
fn test_get_config_with_data_dir_override() {
	let data_dir = tempdir().unwrap();

	let args = CliArgs {
		database_url: None,
		backup_interval_minutes: None,
		backup_count: None,
		debug: false,
		config_dir: None,
		data_dir: Some(data_dir.path().to_path_buf()),
		state_dir: None,
		debug_allow_path_override: true,
	};

	let config = get_config(args).unwrap();

	let expected_db_path = data_dir
		.path()
		.join(DEFAULT_DATABASE_FILENAME)
		.to_string_lossy()
		.to_string();
	assert_eq!(config.database_url, expected_db_path);
	assert_eq!(config.data_dir, Some(data_dir.path().to_path_buf()));
}

#[cfg(debug_assertions)]
#[test]
fn test_get_config_with_state_dir_override() {
	let state_dir = tempdir().unwrap();

	let args = CliArgs {
		database_url: None,
		backup_interval_minutes: None,
		backup_count: None,
		debug: false,
		config_dir: None,
		data_dir: None,
		state_dir: Some(state_dir.path().to_path_buf()),
		debug_allow_path_override: true,
	};

	let config = get_config(args).unwrap();

	assert_eq!(config.state_dir, Some(state_dir.path().to_path_buf()));
}

#[cfg(debug_assertions)]
#[test]
fn test_get_config_args_override_config_dir_file() {
	let config_dir = tempdir().unwrap();
	let config_content = r#"
        database_url = "from_file.db"
    "#;
	create_test_config_file(&config_dir, config_content);

	let args = CliArgs {
		database_url: Some("from_args.db".to_string()),
		backup_interval_minutes: None,
		backup_count: None,
		debug: false,
		config_dir: Some(config_dir.path().to_path_buf()),
		data_dir: None,
		state_dir: None,
		debug_allow_path_override: true,
	};

	let config = get_config(args).unwrap();

	// CLI args always take precedence
	assert_eq!(config.database_url, "from_args.db");
}

// ============================================================================
// get_config tests — debug-mode path override validation
// ============================================================================

#[cfg(debug_assertions)]
#[test]
fn test_get_config_rejects_overrides_without_flag() {
	let data_dir = tempdir().unwrap();

	let args = CliArgs {
		database_url: None,
		backup_interval_minutes: None,
		backup_count: None,
		debug: false,
		config_dir: None,
		data_dir: Some(data_dir.path().to_path_buf()),
		state_dir: None,
		debug_allow_path_override: false,
	};

	let result = get_config(args);

	assert!(result.is_err());
	let err = result.unwrap_err();
	assert!(
		err.contains("--debug-allow-path-override"),
		"Error should mention the flag: {}",
		err
	);
}

// ============================================================================
// get_config tests — precedence
//
// These tests construct `CliArgs` directly, which includes the
// `debug_allow_path_override` field that only exists in debug builds.
// ============================================================================

#[cfg(debug_assertions)]
#[test]
fn test_get_config_precedence() {
	// CLI args override file values, which override defaults
	let args = CliArgs {
		database_url: Some("args.db".to_string()),
		backup_interval_minutes: None,
		backup_count: None,
		debug: false,
		config_dir: None,
		data_dir: None,
		state_dir: None,
		debug_allow_path_override: false,
	};

	let test_config = ConfigBuilder {
		database_url: Some("file.db".to_string()),
		backup_interval_minutes: Some(50),
		..ConfigBuilder::default()
	};

	// Manually replicate the merge chain from get_config
	let config = test_config.merge(config_from_args(args)).build();

	assert_eq!(config.database_url, "args.db"); // From args
	assert_eq!(config.backup_interval_minutes, 50); // From file
	assert_eq!(config.backup_count, DEFAULT_BACKUP_COUNT); // Default
}

// ============================================================================
// Integration tests for full config loading
// ============================================================================

#[cfg(debug_assertions)]
#[test]
fn test_full_config_with_all_sources() {
	let args = CliArgs {
		database_url: Some("args.db".to_string()),
		backup_interval_minutes: None,
		backup_count: Some(25),
		debug: true,
		config_dir: None,
		data_dir: None,
		state_dir: None,
		debug_allow_path_override: false,
	};

	let file_config = ConfigBuilder {
		database_url: Some("file.db".to_string()),
		backup_interval_minutes: Some(40),
		..ConfigBuilder::default()
	};

	let final_config = file_config.merge(config_from_args(args)).build();

	assert_eq!(final_config.database_url, "args.db"); // From args (highest precedence)
	assert_eq!(final_config.backup_interval_minutes, 40); // From file
	assert_eq!(final_config.backup_count, 25); // From args
}

#[cfg(debug_assertions)]
#[test]
fn test_full_config_with_no_overrides() {
	let args = CliArgs {
		database_url: None,
		backup_interval_minutes: None,
		backup_count: None,
		debug: false,
		config_dir: None,
		data_dir: None,
		state_dir: None,
		debug_allow_path_override: false,
	};

	let final_config = ConfigBuilder::default()
		.merge(config_from_args(args))
		.build();

	// All values should be defaults
	assert_eq!(final_config.database_url, DEFAULT_DATABASE_FILENAME);
	assert_eq!(
		final_config.backup_interval_minutes,
		DEFAULT_BACKUP_INTERVAL_MINUTES
	);
	assert_eq!(final_config.backup_count, DEFAULT_BACKUP_COUNT);
}

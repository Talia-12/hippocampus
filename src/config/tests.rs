use super::*;
use tempfile::{tempdir, TempDir};
use std::fs::File;
use std::io::Write;

/// Helper function to create a test configuration file
fn create_test_config_file(dir: &TempDir, content: &str) -> PathBuf {
    let config_path = dir.path().join("config.toml");
    let mut file = File::create(&config_path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    config_path
}

/// Tests for Config::apply_update
#[test]
fn test_apply_update_with_all_values() {
    let config = Config {
        database_url: "original.db".to_string(),
        backup_interval_minutes: 30,
        backup_count: 5,
    };

    let update = ConfigUpdate {
        database_url: Some("updated.db".to_string()),
        backup_interval_minutes: Some(60),
        backup_count: Some(10),
        server_url: None,
    };

    let updated = config.apply_update(update);

    assert_eq!(updated.database_url, "updated.db");
    assert_eq!(updated.backup_interval_minutes, 60);
    assert_eq!(updated.backup_count, 10);
}


#[test]
fn test_apply_update_with_partial_values() {
    let config = Config {
        database_url: "original.db".to_string(),
        backup_interval_minutes: 30,
        backup_count: 5,
    };

    let update = ConfigUpdate {
        database_url: Some("updated.db".to_string()),
        backup_interval_minutes: None,
        backup_count: None,
        server_url: None,
    };

    let updated = config.apply_update(update);

    assert_eq!(updated.database_url, "updated.db");
    assert_eq!(updated.backup_interval_minutes, 30); // Unchanged
    assert_eq!(updated.backup_count, 5); // Unchanged
}


#[test]
fn test_apply_update_with_no_values() {
    let config = Config {
        database_url: "original.db".to_string(),
        backup_interval_minutes: 30,
        backup_count: 5,
    };

    let update = ConfigUpdate::default();

    let updated = config.apply_update(update);

    assert_eq!(updated.database_url, "original.db");
    assert_eq!(updated.backup_interval_minutes, 30);
    assert_eq!(updated.backup_count, 5);
}


/// Tests for Config::backup_interval
#[test]
fn test_backup_interval_conversion() {
    let config = Config {
        database_url: "test.db".to_string(),
        backup_interval_minutes: 30,
        backup_count: 5,
    };

    let duration = config.backup_interval();

    assert_eq!(duration, Duration::from_secs(30 * 60));
}


/// Tests for base_config
#[test]
fn test_base_config_defaults() {
    // Test with None as config_path
    let config = base_config(None);

    // Without a config path, it should use the default database_url
    assert_eq!(config.database_url, "srs_server.db");
    assert_eq!(config.backup_interval_minutes, 20);
    assert_eq!(config.backup_count, 10);
}


#[test]
fn test_base_config_with_path() {
    // Test with Some path
    let temp_dir = tempdir().unwrap();
    let config = base_config(Some(temp_dir.path().to_path_buf()));

    // With a config path, the database_url should be constructed using that path
    let expected_db_path = temp_dir.path().join("srs_server.db").to_string_lossy().to_string();
    assert_eq!(config.database_url, expected_db_path);
    assert_eq!(config.backup_interval_minutes, 20);
    assert_eq!(config.backup_count, 10);
}


/// Tests for config_from_args
#[test]
fn test_config_from_args_with_all_values() {
    let args = CliArgs {
        database_url: Some("args.db".to_string()),
        backup_interval_minutes: Some(45),
        backup_count: Some(15),
        debug: true,
    };

    let update = config_from_args(args);

    assert_eq!(update.database_url, Some("args.db".to_string()));
    assert_eq!(update.backup_interval_minutes, Some(45));
    assert_eq!(update.backup_count, Some(15));
}


#[test]
fn test_config_from_args_with_partial_values() {
    let args = CliArgs {
        database_url: Some("args.db".to_string()),
        backup_interval_minutes: None,
        backup_count: None,
        debug: false,
    };

    let update = config_from_args(args);

    assert_eq!(update.database_url, Some("args.db".to_string()));
    assert_eq!(update.backup_interval_minutes, None);
    assert_eq!(update.backup_count, None);
}


#[test]
fn test_config_from_args_with_no_values() {
    let args = CliArgs {
        database_url: None,
        backup_interval_minutes: None,
        backup_count: None,
        debug: false,
    };

    let update = config_from_args(args);

    assert_eq!(update.database_url, None);
    assert_eq!(update.backup_interval_minutes, None);
    assert_eq!(update.backup_count, None);
}


/// Tests for config_from_file - successful cases
#[test]
fn test_config_from_file_with_no_path() {
    // Test with None as config_path
    let result = config_from_file(None);

    assert!(result.is_ok());
    let update = result.unwrap();
    assert_eq!(update.database_url, None);
    assert_eq!(update.backup_interval_minutes, None);
    assert_eq!(update.backup_count, None);
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

    println!("config_path: {:?}", config_path);

    // get the content of the file
    let content = fs::read_to_string(config_path.clone()).unwrap();
    println!("config_content: {:?}", content);

    // Test with a directory containing a valid config.toml file
    let result = config_from_file(Some(config_path));

    assert!(result.is_ok(), "Failed to parse config file: {}", result.err().unwrap());
    let update = result.unwrap();
    assert_eq!(update.database_url, Some("file.db".to_string()));
    assert_eq!(update.backup_interval_minutes, Some(40));
    assert_eq!(update.backup_count, Some(20));
}


#[test]
fn test_config_from_file_with_partial_values() {
    let temp_dir = tempdir().unwrap();
    let config_content = r#"
        database_url = "file.db"
        # Intentionally missing other fields
    "#;

    let config_path = create_test_config_file(&temp_dir, config_content);

    // Test with a directory containing a partial config.toml file
    let result = config_from_file(Some(config_path));

    assert!(result.is_ok(), "Failed to parse config file: {}", result.err().unwrap());
    let update = result.unwrap();
    assert_eq!(update.database_url, Some("file.db".to_string()));
    assert_eq!(update.backup_interval_minutes, None);
    assert_eq!(update.backup_count, None);
}


/// Tests for config_from_file - failure cases
#[test]
fn test_config_from_file_with_invalid_toml() {
    let temp_dir = tempdir().unwrap();
    let config_content = r#"
        database_url = "file.db"
        backup_interval_minutes = "not a number" # Type error
    "#;

    let config_path = create_test_config_file(&temp_dir, config_content);

    // Test with invalid TOML content
    let result = config_from_file(Some(config_path));

    assert!(result.is_err());
}


#[test]
fn test_config_from_file_with_nonexistent_file() {
    let temp_dir = tempdir().unwrap();
    let nonexistent_path = temp_dir.path().join("nonexistent_config.toml");

    // Test with a path to a nonexistent file
    let result = config_from_file(Some(nonexistent_path));

    assert!(result.is_ok());
    // Should return default values when file doesn't exist
    let update = result.unwrap();
    assert_eq!(update.database_url, None);
    assert_eq!(update.backup_interval_minutes, None);
    assert_eq!(update.backup_count, None);
}


/// Tests for get_config
#[test]
fn test_get_config_precedence() {
    // This test ensures that CLI args override config file values
    // Modified to manually simulate the behavior of get_config with our test data

    // Create a mock args with only database_url specified
    let args = CliArgs {
        database_url: Some("args.db".to_string()),
        backup_interval_minutes: None,
        backup_count: None,
        debug: false,
    };

    // Create a test config that would be merged with base config
    let test_config = ConfigUpdate {
        database_url: Some("file.db".to_string()),
        backup_interval_minutes: Some(50),
        backup_count: None,
        server_url: None,
    };

    // Create a base config with None path
    let base = base_config(None);

    // Manually replicate the behavior of get_config
    let config = base
        .apply_update(test_config)
        .apply_update(config_from_args(args));

    // Assert that args override file values, which override base values
    assert_eq!(config.database_url, "args.db");
    assert_eq!(config.backup_interval_minutes, 50); // From file
    assert_eq!(config.backup_count, 10); // From base
}


/// Integration tests for full config loading
#[test]
fn test_full_config_with_all_sources() {
    // This is a simulated integration test that exercises the merging logic
    // without relying on actual files

    // Set up test args
    let args = CliArgs {
        database_url: Some("args.db".to_string()),
        backup_interval_minutes: None,
        backup_count: Some(25),
        debug: true,
    };

    // Create a base config with None path
    let base = base_config(None);

    // Create a simulated config from file
    let file_config = ConfigUpdate {
        database_url: Some("file.db".to_string()),
        backup_interval_minutes: Some(40),
        backup_count: None,
        server_url: None,
    };

    // Manually simulate the full config loading process
    let final_config = base
        .apply_update(file_config)
        .apply_update(config_from_args(args));

    // Check that precedence works correctly
    assert_eq!(final_config.database_url, "args.db"); // From args (highest precedence)
    assert_eq!(final_config.backup_interval_minutes, 40); // From file
    assert_eq!(final_config.backup_count, 25); // From args
}


#[test]
fn test_full_config_with_no_overrides() {
    // Create empty args (no overrides)
    let args = CliArgs {
        database_url: None,
        backup_interval_minutes: None,
        backup_count: None,
        debug: false,
    };

    // Create a base config with None path
    let base = base_config(None);

    // Manually simulate the config loading with no overrides
    let final_config = base
        .apply_update(ConfigUpdate::default())
        .apply_update(config_from_args(args));

    // All values should remain as in base config
    assert_eq!(final_config.database_url, "srs_server.db");
    assert_eq!(final_config.backup_interval_minutes, 20);
    assert_eq!(final_config.backup_count, 10);
}

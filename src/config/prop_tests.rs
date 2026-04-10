use super::*;
use crate::test_utils::arb_messy_string;
use proptest::prelude::*;

/// Generates an arbitrary ConfigBuilder with random optional fields
fn arb_config_builder() -> impl Strategy<Value = ConfigBuilder> {
	(
		prop::option::of(arb_messy_string()),
		prop::option::of(any::<u64>()),
		prop::option::of(any::<u32>()),
		prop::option::of(arb_messy_string()),
	)
		.prop_map(
			|(database_url, backup_interval_minutes, backup_count, server_url)| ConfigBuilder {
				database_url,
				backup_interval_minutes,
				backup_count,
				server_url,
				config_dir: None,
				data_dir: None,
				state_dir: None,
			},
		)
}

/// Generates a ConfigBuilder where all fields are Some
fn arb_full_config_builder() -> impl Strategy<Value = ConfigBuilder> {
	(
		arb_messy_string(),
		any::<u64>(),
		any::<u32>(),
		arb_messy_string(),
	)
		.prop_map(
			|(database_url, backup_interval_minutes, backup_count, server_url)| ConfigBuilder {
				database_url: Some(database_url),
				backup_interval_minutes: Some(backup_interval_minutes),
				backup_count: Some(backup_count),
				server_url: Some(server_url),
				config_dir: None,
				data_dir: None,
				state_dir: None,
			},
		)
}

// ============================================================================
// C1: merge Algebraic Properties
// ============================================================================

proptest! {
	/// C1.1: Identity: merge(default) == original builder
	#[test]
	fn prop_c1_1_identity(builder in arb_config_builder()) {
		let original = builder.clone();
		let merged = builder.merge(ConfigBuilder::default());

		prop_assert_eq!(merged.database_url, original.database_url);
		prop_assert_eq!(merged.backup_interval_minutes, original.backup_interval_minutes);
		prop_assert_eq!(merged.backup_count, original.backup_count);
	}

	/// C1.2: Full override: merge with all-Some replaces all fields
	#[test]
	fn prop_c1_2_full_override(builder in arb_config_builder(), other in arb_full_config_builder()) {
		let expected_url = other.database_url.clone();
		let expected_interval = other.backup_interval_minutes;
		let expected_count = other.backup_count;

		let merged = builder.merge(other);

		prop_assert_eq!(merged.database_url, expected_url);
		prop_assert_eq!(merged.backup_interval_minutes, expected_interval);
		prop_assert_eq!(merged.backup_count, expected_count);
	}

	/// C1.3: Partial override — None fields preserved
	#[test]
	fn prop_c1_3_none_fields_preserved(builder in arb_config_builder()) {
		let original = builder.clone();

		let merged = builder.merge(ConfigBuilder::default());

		prop_assert_eq!(merged.database_url, original.database_url);
		prop_assert_eq!(merged.backup_interval_minutes, original.backup_interval_minutes);
		prop_assert_eq!(merged.backup_count, original.backup_count);
	}

	/// C1.4: Partial override — Some fields replaced
	#[test]
	fn prop_c1_4_some_fields_replaced(
		builder in arb_config_builder(),
		new_url in arb_messy_string(),
	) {
		let original = builder.clone();

		let other = ConfigBuilder {
			database_url: Some(new_url.clone()),
			..ConfigBuilder::default()
		};

		let merged = builder.merge(other);

		prop_assert_eq!(merged.database_url, Some(new_url));
		prop_assert_eq!(merged.backup_interval_minutes, original.backup_interval_minutes);
		prop_assert_eq!(merged.backup_count, original.backup_count);
	}

	/// C1.5: Last-write-wins: b's Some fields override a's
	#[test]
	fn prop_c1_5_last_write_wins(
		base in arb_config_builder(),
		a in arb_config_builder(),
		b in arb_config_builder(),
	) {
		let expected_url = b.database_url.clone()
			.or(a.database_url.clone())
			.or(base.database_url.clone());
		let expected_interval = b.backup_interval_minutes
			.or(a.backup_interval_minutes)
			.or(base.backup_interval_minutes);
		let expected_count = b.backup_count
			.or(a.backup_count)
			.or(base.backup_count);

		let merged = base.merge(a).merge(b);

		prop_assert_eq!(merged.database_url, expected_url);
		prop_assert_eq!(merged.backup_interval_minutes, expected_interval);
		prop_assert_eq!(merged.backup_count, expected_count);
	}
}

// ============================================================================
// C2: backup_interval Conversion
// ============================================================================

proptest! {
	/// C2.1: backup_interval() == Duration::from_secs(minutes * 60)
	#[test]
	fn prop_c2_1_backup_interval_conversion(minutes in 0u64..=u64::MAX / 60) {
		let config = Config {
			database_url: String::new(),
			backup_interval_minutes: minutes,
			backup_count: 0,
			config_dir: None,
			data_dir: None,
			state_dir: None,
		};

		prop_assert_eq!(config.backup_interval(), Duration::from_secs(minutes * 60));
	}
}

// ============================================================================
// C3: config_from_args Mapping
//
// These tests construct `CliArgs` directly, which includes the
// `debug_allow_path_override` field that only exists in debug builds.
// ============================================================================

#[cfg(debug_assertions)]
proptest! {
	/// C3.1: config_from_args preserves all fields from CliArgs
	#[test]
	fn prop_c3_1_args_mapping(
		database_url in prop::option::of(arb_messy_string()),
		backup_interval_minutes in prop::option::of(any::<u64>()),
		backup_count in prop::option::of(any::<u32>()),
		debug in any::<bool>(),
	) {
		let args = CliArgs {
			database_url: database_url.clone(),
			backup_interval_minutes,
			backup_count,
			debug,
			config_dir: None,
			data_dir: None,
			state_dir: None,
			debug_allow_path_override: false,
		};

		let builder = config_from_args(args);

		prop_assert_eq!(builder.database_url, database_url);
		prop_assert_eq!(builder.backup_interval_minutes, backup_interval_minutes);
		prop_assert_eq!(builder.backup_count, backup_count);
		// server_url is always None from args
		prop_assert_eq!(builder.server_url, None);
	}
}

// ============================================================================
// C4: config_from_args preserves arbitrary fields
// ============================================================================

#[cfg(debug_assertions)]
proptest! {
	/// C4.1: config_from_args preserves all field values including None
	#[test]
	fn prop_c4_1_config_from_args_preserves_fields(
		database_url in prop::option::of("\\PC{0,50}"),
		backup_interval_minutes in prop::option::of(0u64..=1_000_000u64),
		backup_count in prop::option::of(0u32..=1000u32),
		debug in any::<bool>(),
	) {
		let args = CliArgs {
			database_url: database_url.clone(),
			backup_interval_minutes,
			backup_count,
			debug,
			config_dir: None,
			data_dir: None,
			state_dir: None,
			debug_allow_path_override: false,
		};

		let builder = config_from_args(args);

		prop_assert_eq!(builder.database_url, database_url);
		prop_assert_eq!(builder.backup_interval_minutes, backup_interval_minutes);
		prop_assert_eq!(builder.backup_count, backup_count);
		prop_assert_eq!(builder.server_url, None);
	}
}

// ============================================================================
// C5: config_from_file roundtrip
// ============================================================================

proptest! {
	/// C5.1: Write Config fields to TOML, read back, fields match
	#[test]
	fn prop_c5_1_config_from_file_roundtrip(
		database_url in "[a-zA-Z0-9_./]{1,50}",
		backup_interval_minutes in 1u64..=1_000_000u64,
		backup_count in 1u32..=1000u32,
	) {
		use std::io::Write;

		let toml_content = format!(
			"database_url = \"{}\"\nbackup_interval_minutes = {}\nbackup_count = {}\n",
			database_url, backup_interval_minutes, backup_count
		);

		let temp_dir = tempfile::tempdir().unwrap();
		let config_path = temp_dir.path().join("config.toml");
		let mut file = std::fs::File::create(&config_path).unwrap();
		file.write_all(toml_content.as_bytes()).unwrap();

		let result = config_from_file(Some(config_path));
		prop_assert!(result.is_ok(), "config_from_file should succeed: {:?}", result.err());

		let builder = result.unwrap();
		prop_assert_eq!(builder.database_url.as_deref(), Some(database_url.as_str()));
		prop_assert_eq!(builder.backup_interval_minutes, Some(backup_interval_minutes));
		prop_assert_eq!(builder.backup_count, Some(backup_count));
	}
}

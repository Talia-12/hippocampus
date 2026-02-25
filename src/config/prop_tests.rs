use super::*;
use crate::test_utils::arb_messy_string;
use proptest::prelude::*;

/// Generates an arbitrary Config
fn arb_config() -> impl Strategy<Value = Config> {
    (arb_messy_string(), any::<u64>(), any::<u32>()).prop_map(
        |(database_url, backup_interval_minutes, backup_count)| Config {
            database_url,
            backup_interval_minutes,
            backup_count,
        },
    )
}

/// Generates an arbitrary ConfigUpdate
fn arb_config_update() -> impl Strategy<Value = ConfigUpdate> {
    (
        prop::option::of(arb_messy_string()),
        prop::option::of(any::<u64>()),
        prop::option::of(any::<u32>()),
        prop::option::of(arb_messy_string()),
    )
        .prop_map(
            |(database_url, backup_interval_minutes, backup_count, server_url)| ConfigUpdate {
                database_url,
                backup_interval_minutes,
                backup_count,
                server_url,
            },
        )
}

/// Generates a ConfigUpdate where all fields are Some
fn arb_full_config_update() -> impl Strategy<Value = ConfigUpdate> {
    (arb_messy_string(), any::<u64>(), any::<u32>(), arb_messy_string()).prop_map(
        |(database_url, backup_interval_minutes, backup_count, server_url)| ConfigUpdate {
            database_url: Some(database_url),
            backup_interval_minutes: Some(backup_interval_minutes),
            backup_count: Some(backup_count),
            server_url: Some(server_url),
        },
    )
}

// ============================================================================
// C1: apply_update Algebraic Properties
// ============================================================================

proptest! {
    /// C1.1: Identity: apply_update(default) == original config
    #[test]
    fn prop_c1_1_identity(config in arb_config()) {
        let original_url = config.database_url.clone();
        let original_interval = config.backup_interval_minutes;
        let original_count = config.backup_count;

        let updated = config.apply_update(ConfigUpdate::default());

        prop_assert_eq!(updated.database_url, original_url);
        prop_assert_eq!(updated.backup_interval_minutes, original_interval);
        prop_assert_eq!(updated.backup_count, original_count);
    }

    /// C1.2: Full override: apply_update with all Some replaces all fields
    #[test]
    fn prop_c1_2_full_override(config in arb_config(), update in arb_full_config_update()) {
        let expected_url = update.database_url.clone().unwrap();
        let expected_interval = update.backup_interval_minutes.unwrap();
        let expected_count = update.backup_count.unwrap();

        let updated = config.apply_update(update);

        prop_assert_eq!(updated.database_url, expected_url);
        prop_assert_eq!(updated.backup_interval_minutes, expected_interval);
        prop_assert_eq!(updated.backup_count, expected_count);
    }

    /// C1.3: Partial override — None fields preserved
    #[test]
    fn prop_c1_3_none_fields_preserved(config in arb_config()) {
        let original_url = config.database_url.clone();
        let original_interval = config.backup_interval_minutes;
        let original_count = config.backup_count;

        // Update with all None
        let update = ConfigUpdate {
            database_url: None,
            backup_interval_minutes: None,
            backup_count: None,
            server_url: None,
        };

        let updated = config.apply_update(update);

        prop_assert_eq!(updated.database_url, original_url);
        prop_assert_eq!(updated.backup_interval_minutes, original_interval);
        prop_assert_eq!(updated.backup_count, original_count);
    }

    /// C1.4: Partial override — Some fields replaced
    #[test]
    fn prop_c1_4_some_fields_replaced(
        config in arb_config(),
        new_url in arb_messy_string(),
    ) {
        let original_interval = config.backup_interval_minutes;
        let original_count = config.backup_count;

        let update = ConfigUpdate {
            database_url: Some(new_url.clone()),
            backup_interval_minutes: None,
            backup_count: None,
            server_url: None,
        };

        let updated = config.apply_update(update);

        prop_assert_eq!(updated.database_url, new_url);
        prop_assert_eq!(updated.backup_interval_minutes, original_interval);
        prop_assert_eq!(updated.backup_count, original_count);
    }

    /// C1.5: Last-write-wins: b's Some fields override a's
    #[test]
    fn prop_c1_5_last_write_wins(
        config in arb_config(),
        a in arb_config_update(),
        b in arb_config_update(),
    ) {
        let after_a = config.clone().apply_update(a.clone());
        let after_ab = after_a.apply_update(b.clone());

        // For each field: if b has Some, result == b's value; else result == after_a's value
        let expected_url = b.database_url.unwrap_or_else(|| {
            a.database_url.unwrap_or(config.database_url.clone())
        });
        let expected_interval = b.backup_interval_minutes.unwrap_or_else(|| {
            a.backup_interval_minutes.unwrap_or(config.backup_interval_minutes)
        });
        let expected_count = b.backup_count.unwrap_or_else(|| {
            a.backup_count.unwrap_or(config.backup_count)
        });

        prop_assert_eq!(after_ab.database_url, expected_url);
        prop_assert_eq!(after_ab.backup_interval_minutes, expected_interval);
        prop_assert_eq!(after_ab.backup_count, expected_count);
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
        };

        prop_assert_eq!(config.backup_interval(), Duration::from_secs(minutes * 60));
    }
}

// ============================================================================
// C3: config_from_args Mapping
// ============================================================================

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
        };

        let update = config_from_args(args);

        prop_assert_eq!(update.database_url, database_url);
        prop_assert_eq!(update.backup_interval_minutes, backup_interval_minutes);
        prop_assert_eq!(update.backup_count, backup_count);
        // server_url is always None from args
        prop_assert_eq!(update.server_url, None);
    }
}

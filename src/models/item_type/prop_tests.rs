use super::*;
use crate::test_utils::{arb_datetime_utc, arb_messy_string};
use proptest::prelude::*;

// ============================================================================
// IT1: Constructor Properties
// ============================================================================

proptest! {
    /// IT1.1: ItemType::new produces a valid UUID
    #[test]
    fn prop_it1_1_new_produces_valid_uuid(name in "\\PC+") {
        let item_type = ItemType::new(name);
        prop_assert!(Uuid::parse_str(&item_type.get_id()).is_ok(),
            "get_id() should be a valid UUID, got: {}", item_type.get_id());
    }

    /// IT1.2: ItemType::new preserves name
    #[test]
    fn prop_it1_2_new_preserves_name(name in "\\PC+") {
        let item_type = ItemType::new(name.clone());
        prop_assert_eq!(item_type.get_name(), name);
    }

    /// IT1.3: ItemType::new timestamp is recent
    #[test]
    fn prop_it1_3_new_timestamp_recent(name in "\\PC+") {
        let item_type = ItemType::new(name);
        let diff = (Utc::now() - item_type.get_created_at()).num_seconds();
        prop_assert!(diff < 2, "created_at should be recent, diff: {}s", diff);
    }

    /// IT1.4: ItemType::new_with_fields preserves all fields roundtrip
    #[test]
    fn prop_it1_4_new_with_fields_roundtrip(
        id in "\\PC+",
        name in "\\PC+",
        created_at in arb_datetime_utc(),
    ) {
        let item_type = ItemType::new_with_fields(id.clone(), name.clone(), created_at);
        prop_assert_eq!(item_type.get_id(), id);
        prop_assert_eq!(item_type.get_name(), name);
        let diff = (item_type.get_created_at() - created_at).num_seconds().abs();
        prop_assert!(diff == 0, "created_at should match, diff: {}s", diff);
    }
}

// ============================================================================
// IT1r: Constructor Robustness
// ============================================================================

proptest! {
    /// IT1r.1: ItemType::new does not panic for any messy string
    #[test]
    fn prop_it1r_1_new_does_not_panic(name in arb_messy_string()) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ItemType::new(name.clone())
        }));
        prop_assert!(result.is_ok(),
            "ItemType::new should not panic for name={:?}", name);
    }

    /// IT1r.2: ItemType::new_with_fields does not panic for arbitrary inputs
    #[test]
    fn prop_it1r_2_new_with_fields_does_not_panic(
        id in arb_messy_string(),
        name in arb_messy_string(),
        created_at in arb_datetime_utc(),
    ) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ItemType::new_with_fields(id.clone(), name.clone(), created_at)
        }));
        prop_assert!(result.is_ok(),
            "new_with_fields should not panic for id={:?}, name={:?}", id, name);
    }
}

// ============================================================================
// IT2: Getter/Setter Roundtrips
// ============================================================================

proptest! {
    /// IT2.1: set_name / get_name roundtrip
    #[test]
    fn prop_it2_1_name_roundtrip(name in arb_messy_string()) {
        let mut item_type = ItemType::new("initial".to_string());
        item_type.set_name(name.clone());
        prop_assert_eq!(item_type.get_name(), name);
    }
}

// ============================================================================
// IT3: Serialization
// ============================================================================

proptest! {
    /// IT3.1: Serde roundtrip preserves all fields
    #[test]
    fn prop_it3_1_serde_roundtrip(name in "\\PC+") {
        let item_type = ItemType::new(name);
        let json = serde_json::to_string(&item_type).unwrap();
        let deserialized: ItemType = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(item_type.get_id(), deserialized.get_id());
        prop_assert_eq!(item_type.get_name(), deserialized.get_name());
        prop_assert_eq!(item_type.get_created_at_raw(), deserialized.get_created_at_raw());
    }
}

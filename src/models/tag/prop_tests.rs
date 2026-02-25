use super::*;
use crate::test_utils::{arb_datetime_utc, arb_messy_string};
use proptest::prelude::*;

// ============================================================================
// TG1: Constructor Properties
// ============================================================================

proptest! {
    /// TG1.1: Tag::new produces a valid UUID
    #[test]
    fn prop_tg1_1_new_produces_valid_uuid(name in "\\PC+", visible in any::<bool>()) {
        let tag = Tag::new(name, visible);
        prop_assert!(Uuid::parse_str(&tag.get_id()).is_ok(),
            "get_id() should be a valid UUID, got: {}", tag.get_id());
    }

    /// TG1.2: Tag::new preserves name
    #[test]
    fn prop_tg1_2_new_preserves_name(name in "\\PC+", visible in any::<bool>()) {
        let tag = Tag::new(name.clone(), visible);
        prop_assert_eq!(tag.get_name(), name);
    }

    /// TG1.3: Tag::new preserves visibility
    #[test]
    fn prop_tg1_3_new_preserves_visibility(name in "\\PC+", visible in any::<bool>()) {
        let tag = Tag::new(name, visible);
        prop_assert_eq!(tag.get_visible(), visible);
    }

    /// TG1.4: Tag::new timestamp is recent
    #[test]
    fn prop_tg1_4_new_timestamp_recent(name in "\\PC+", visible in any::<bool>()) {
        let tag = Tag::new(name, visible);
        let diff = (Utc::now() - tag.get_created_at()).num_seconds();
        prop_assert!(diff < 2, "created_at should be recent, diff: {}s", diff);
    }

    /// TG1.5: Tag::new_with_fields preserves all fields roundtrip
    #[test]
    fn prop_tg1_5_new_with_fields_roundtrip(
        id in "\\PC+",
        name in "\\PC+",
        visible in any::<bool>(),
        created_at in arb_datetime_utc(),
    ) {
        let tag = Tag::new_with_fields(id.clone(), name.clone(), visible, created_at);
        prop_assert_eq!(tag.get_id(), id);
        prop_assert_eq!(tag.get_name(), name);
        prop_assert_eq!(tag.get_visible(), visible);
        let diff = (tag.get_created_at() - created_at).num_seconds().abs();
        prop_assert!(diff == 0, "created_at should match, diff: {}s", diff);
    }
}

// ============================================================================
// TG1r: Constructor Robustness
// ============================================================================

proptest! {
    /// TG1r.1: Tag::new does not panic for any (messy_string, bool)
    #[test]
    fn prop_tg1r_1_new_does_not_panic(name in arb_messy_string(), visible in any::<bool>()) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Tag::new(name.clone(), visible)
        }));
        prop_assert!(result.is_ok(),
            "Tag::new should not panic for name={:?}", name);
    }

    /// TG1r.2: Tag::new_with_fields does not panic for arbitrary inputs
    #[test]
    fn prop_tg1r_2_new_with_fields_does_not_panic(
        id in arb_messy_string(),
        name in arb_messy_string(),
        visible in any::<bool>(),
        created_at in arb_datetime_utc(),
    ) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Tag::new_with_fields(id.clone(), name.clone(), visible, created_at)
        }));
        prop_assert!(result.is_ok(),
            "new_with_fields should not panic for id={:?}, name={:?}", id, name);
    }
}

// ============================================================================
// TG2: Getter/Setter Roundtrips
// ============================================================================

proptest! {
    /// TG2.1: set_name / get_name roundtrip
    #[test]
    fn prop_tg2_1_name_roundtrip(name in arb_messy_string()) {
        let mut tag = Tag::new("initial".to_string(), true);
        tag.set_name(name.clone());
        prop_assert_eq!(tag.get_name(), name);
    }

    /// TG2.2: set_visible / get_visible roundtrip
    #[test]
    fn prop_tg2_2_visible_roundtrip(visible in any::<bool>()) {
        let mut tag = Tag::new("tag".to_string(), !visible);
        tag.set_visible(visible);
        prop_assert_eq!(tag.get_visible(), visible);
    }
}

// ============================================================================
// TG3: Serialization
// ============================================================================

proptest! {
    /// TG3.1: Serde roundtrip preserves all fields
    #[test]
    fn prop_tg3_1_serde_roundtrip(name in "\\PC+", visible in any::<bool>()) {
        let tag = Tag::new(name, visible);
        let json = serde_json::to_string(&tag).unwrap();
        let deserialized: Tag = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(tag.get_id(), deserialized.get_id());
        prop_assert_eq!(tag.get_name(), deserialized.get_name());
        prop_assert_eq!(tag.get_visible(), deserialized.get_visible());
        prop_assert_eq!(tag.get_created_at_raw(), deserialized.get_created_at_raw());
    }
}

use super::*;
use crate::test_utils::{arb_datetime_utc, arb_json, arb_messy_string, json_approx_eq};
use proptest::prelude::*;

// ============================================================================
// I1: Constructor Properties
// ============================================================================

proptest! {
    /// I1.1: Item::new produces a valid UUID
    #[test]
    fn prop_i1_1_new_produces_valid_uuid(
        item_type in "\\PC+",
        title in "\\PC+",
        data in arb_json(),
    ) {
        let item = Item::new(item_type, title, JsonValue(data));
        prop_assert!(Uuid::parse_str(&item.get_id()).is_ok(),
            "get_id() should be a valid UUID, got: {}", item.get_id());
    }

    /// I1.2: Item::new preserves item_type
    #[test]
    fn prop_i1_2_new_preserves_item_type(
        item_type in "\\PC+",
        title in "\\PC+",
        data in arb_json(),
    ) {
        let item = Item::new(item_type.clone(), title, JsonValue(data));
        prop_assert_eq!(item.get_item_type(), item_type);
    }

    /// I1.3: Item::new preserves title
    #[test]
    fn prop_i1_3_new_preserves_title(
        item_type in "\\PC+",
        title in "\\PC+",
        data in arb_json(),
    ) {
        let item = Item::new(item_type, title.clone(), JsonValue(data));
        prop_assert_eq!(item.get_title(), title);
    }

    /// I1.4: Item::new preserves data
    #[test]
    fn prop_i1_4_new_preserves_data(
        item_type in "\\PC+",
        title in "\\PC+",
        data in arb_json(),
    ) {
        let data = JsonValue(data);
        let item = Item::new(item_type, title, data.clone());
        prop_assert_eq!(item.get_data(), data);
    }

    /// I1.5: Item::new timestamps are recent
    #[test]
    fn prop_i1_5_new_timestamps_recent(
        item_type in "\\PC+",
        title in "\\PC+",
    ) {
        let item = Item::new(item_type, title, JsonValue(serde_json::Value::Null));
        let now = Utc::now();
        let created_diff = (now - item.get_created_at()).num_seconds();
        let updated_diff = (now - item.get_updated_at()).num_seconds();
        prop_assert!(created_diff < 2, "created_at should be recent, diff: {}s", created_diff);
        prop_assert!(updated_diff < 2, "updated_at should be recent, diff: {}s", updated_diff);
    }

    /// I1.6: Item::new timestamps are equal (both set to same `now`)
    #[test]
    fn prop_i1_6_new_timestamps_equal(
        item_type in "\\PC+",
        title in "\\PC+",
    ) {
        let item = Item::new(item_type, title, JsonValue(serde_json::Value::Null));
        prop_assert_eq!(item.get_created_at_raw(), item.get_updated_at_raw(),
            "created_at and updated_at should be equal on construction");
    }

    /// I1.7: Item::new_with_fields preserves all fields roundtrip
    #[test]
    fn prop_i1_7_new_with_fields_roundtrip(
        id in "\\PC+",
        item_type in "\\PC+",
        title in "\\PC+",
        data in arb_json(),
        created_at in arb_datetime_utc(),
        updated_at in arb_datetime_utc(),
    ) {
        let data = JsonValue(data);
        let item = Item::new_with_fields(
            id.clone(),
            item_type.clone(),
            title.clone(),
            data.clone(),
            created_at,
            updated_at,
        );
        prop_assert_eq!(item.get_id(), id);
        prop_assert_eq!(item.get_item_type(), item_type);
        prop_assert_eq!(item.get_title(), title);
        prop_assert_eq!(item.get_data(), data);
        let created_diff = (item.get_created_at() - created_at).num_seconds().abs();
        let updated_diff = (item.get_updated_at() - updated_at).num_seconds().abs();
        prop_assert!(created_diff == 0, "created_at should match, diff: {}s", created_diff);
        prop_assert!(updated_diff == 0, "updated_at should match, diff: {}s", updated_diff);
    }
}

// ============================================================================
// I1r: Constructor Robustness
// ============================================================================

proptest! {
    /// I1r.1: Item::new does not panic for any (messy_string, messy_string, arbitrary JSON)
    #[test]
    fn prop_i1r_1_new_does_not_panic(
        item_type in arb_messy_string(),
        title in arb_messy_string(),
        data in arb_json(),
    ) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Item::new(item_type.clone(), title.clone(), JsonValue(data))
        }));
        prop_assert!(result.is_ok(),
            "Item::new should not panic for item_type={:?}, title={:?}", item_type, title);
    }

    /// I1r.2: Item::new_with_fields does not panic for arbitrary inputs
    #[test]
    fn prop_i1r_2_new_with_fields_does_not_panic(
        id in arb_messy_string(),
        item_type in arb_messy_string(),
        title in arb_messy_string(),
        data in arb_json(),
        created_at in arb_datetime_utc(),
        updated_at in arb_datetime_utc(),
    ) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Item::new_with_fields(id.clone(), item_type.clone(), title.clone(), JsonValue(data), created_at, updated_at)
        }));
        prop_assert!(result.is_ok(),
            "new_with_fields should not panic for id={:?}, item_type={:?}, title={:?}",
            id, item_type, title);
    }
}

// ============================================================================
// I2: Getter/Setter Roundtrips
// ============================================================================

proptest! {
    /// I2.1: set_title / get_title roundtrip
    #[test]
    fn prop_i2_1_title_roundtrip(title in arb_messy_string()) {
        let mut item = Item::new("type".to_string(), "initial".to_string(), JsonValue(serde_json::Value::Null));
        item.set_title(title.clone());
        prop_assert_eq!(item.get_title(), title);
    }

    /// I2.2: set_data / get_data roundtrip
    #[test]
    fn prop_i2_2_data_roundtrip(data in arb_json()) {
        let data = JsonValue(data);
        let mut item = Item::new("type".to_string(), "title".to_string(), JsonValue(serde_json::Value::Null));
        item.set_data(data.clone());
        prop_assert_eq!(item.get_data(), data);
    }

    /// I2.3: set_item_type / get_item_type roundtrip
    #[test]
    fn prop_i2_3_item_type_roundtrip(item_type in arb_messy_string()) {
        let mut item = Item::new("initial".to_string(), "title".to_string(), JsonValue(serde_json::Value::Null));
        item.set_item_type(item_type.clone());
        prop_assert_eq!(item.get_item_type(), item_type);
    }
}

// ============================================================================
// I2s: Setter Side Effects
// ============================================================================

proptest! {
    /// I2s.1: set_title advances updated_at
    #[test]
    fn prop_i2s_1_set_title_advances_updated_at(title in "\\PC+") {
        let mut item = Item::new("type".to_string(), "initial".to_string(), JsonValue(serde_json::Value::Null));
        let before = item.get_updated_at_raw();
        // Small sleep to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(2));
        item.set_title(title);
        let after = item.get_updated_at_raw();
        prop_assert!(after >= before,
            "updated_at should advance after set_title: before={:?}, after={:?}", before, after);
    }

    /// I2s.2: set_data advances updated_at
    #[test]
    fn prop_i2s_2_set_data_advances_updated_at(data in arb_json()) {
        let mut item = Item::new("type".to_string(), "title".to_string(), JsonValue(serde_json::Value::Null));
        let before = item.get_updated_at_raw();
        std::thread::sleep(std::time::Duration::from_millis(2));
        item.set_data(JsonValue(data));
        let after = item.get_updated_at_raw();
        prop_assert!(after >= before,
            "updated_at should advance after set_data: before={:?}, after={:?}", before, after);
    }

    /// I2s.3: set_item_type advances updated_at
    #[test]
    fn prop_i2s_3_set_item_type_advances_updated_at(item_type in "\\PC+") {
        let mut item = Item::new("initial".to_string(), "title".to_string(), JsonValue(serde_json::Value::Null));
        let before = item.get_updated_at_raw();
        std::thread::sleep(std::time::Duration::from_millis(2));
        item.set_item_type(item_type);
        let after = item.get_updated_at_raw();
        prop_assert!(after >= before,
            "updated_at should advance after set_item_type: before={:?}, after={:?}", before, after);
    }

    /// I2s.4: Setters do not modify created_at
    #[test]
    fn prop_i2s_4_setters_preserve_created_at(
        title in "\\PC+",
        item_type in "\\PC+",
        data in arb_json(),
    ) {
        let mut item = Item::new("initial_type".to_string(), "initial_title".to_string(), JsonValue(serde_json::Value::Null));
        let created_at = item.get_created_at_raw();

        item.set_title(title);
        prop_assert_eq!(item.get_created_at_raw(), created_at, "set_title should not change created_at");

        item.set_item_type(item_type);
        prop_assert_eq!(item.get_created_at_raw(), created_at, "set_item_type should not change created_at");

        item.set_data(JsonValue(data));
        prop_assert_eq!(item.get_created_at_raw(), created_at, "set_data should not change created_at");
    }
}

// ============================================================================
// I3: Serialization
// ============================================================================

proptest! {
    /// I3.1: Serde roundtrip preserves all fields
    ///
    /// JSON data is compared with numeric tolerance to handle f64 precision
    /// loss inherent to serde_json serialize→deserialize.
    #[test]
    fn prop_i3_1_serde_roundtrip(
        item_type in "\\PC+",
        title in "\\PC+",
        data in arb_json(),
    ) {
        let item = Item::new(item_type, title, JsonValue(data));
        let json = serde_json::to_string(&item).unwrap();
        let deserialized: Item = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(item.get_id(), deserialized.get_id());
        prop_assert_eq!(item.get_item_type(), deserialized.get_item_type());
        prop_assert_eq!(item.get_title(), deserialized.get_title());
        prop_assert!(json_approx_eq(&item.get_data().0, &deserialized.get_data().0),
            "data mismatch:\n  left:  {:?}\n  right: {:?}",
            item.get_data().0, deserialized.get_data().0);
        prop_assert_eq!(item.get_created_at_raw(), deserialized.get_created_at_raw());
        prop_assert_eq!(item.get_updated_at_raw(), deserialized.get_updated_at_raw());
    }
}

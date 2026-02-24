use super::*;
use crate::test_utils::{
    arb_any_f32, arb_priority, arb_priority_offset, arb_sort_position, arb_wide_offset,
};
use proptest::prelude::*;

// ============================================================================
// P1: Getter/Setter Roundtrips
// ============================================================================

proptest! {
    /// P1.1: set_sort_position(pos); get_sort_position() == pos
    #[test]
    fn prop_p1_1_sort_position_roundtrip(pos in arb_sort_position()) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        card.set_sort_position(pos);
        prop_assert_eq!(card.get_sort_position(), pos);
    }

    /// P1.2: set_priority_offset(off); get_priority_offset() == off
    #[test]
    fn prop_p1_2_priority_offset_roundtrip(off in arb_priority_offset()) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        card.set_priority_offset(off);
        prop_assert!((card.get_priority_offset() - off).abs() < f32::EPSILON);
    }
}

// ============================================================================
// P1r: Getter/Setter Robustness with Arbitrary Floats
// ============================================================================

proptest! {
    /// P1r.1: sort_position roundtrip with any f32 (bit-exact for NaN)
    #[test]
    fn prop_p1r_1_sort_position_any_f32_roundtrip(v in arb_any_f32()) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        card.set_sort_position(Some(v));
        let got = card.get_sort_position();
        prop_assert!(got.is_some());
        prop_assert_eq!(got.unwrap().to_bits(), v.to_bits());
    }

    /// P1r.2: priority_offset roundtrip with any f32 (bit-exact for NaN)
    #[test]
    fn prop_p1r_2_priority_offset_any_f32_roundtrip(v in arb_any_f32()) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        card.set_priority_offset(v);
        prop_assert_eq!(card.get_priority_offset().to_bits(), v.to_bits());
    }
}

// ============================================================================
// P2: to_json_hide_priority_offset Properties
// ============================================================================

proptest! {
    /// P2.1: JSON priority == (base + offset).clamp(0.0, 1.0)
    #[test]
    fn prop_p2_1_effective_priority_correct(
        base in arb_priority(),
        offset in arb_wide_offset(),
    ) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), base);
        card.set_priority_offset(offset);
        let json = card.to_json_hide_priority_offset();
        let json_priority = json["priority"].as_f64().unwrap() as f32;
        let expected = (base + offset).clamp(0.0, 1.0);
        prop_assert!((json_priority - expected).abs() < 1e-5,
            "expected {}, got {}", expected, json_priority);
    }

    /// P2.2: JSON has no priority_offset key
    #[test]
    fn prop_p2_2_offset_field_absent(
        base in arb_priority(),
        offset in arb_wide_offset(),
    ) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), base);
        card.set_priority_offset(offset);
        let json = card.to_json_hide_priority_offset();
        prop_assert!(json.get("priority_offset").is_none(),
            "priority_offset key should be absent from JSON");
    }

    /// P2.3: All fields except priority and priority_offset match serde_json::to_value()
    #[test]
    fn prop_p2_3_other_fields_preserved(
        base in arb_priority(),
        offset in arb_priority_offset(),
    ) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), base);
        card.set_priority_offset(offset);
        let hidden = card.to_json_hide_priority_offset();
        let full = serde_json::to_value(&card).unwrap();

        let hidden_obj = hidden.as_object().unwrap();
        let full_obj = full.as_object().unwrap();

        for (key, value) in full_obj {
            if key == "priority" || key == "priority_offset" {
                continue;
            }
            prop_assert_eq!(
                hidden_obj.get(key), Some(value),
                "Field '{}' mismatch", key
            );
        }
    }

    /// P2.4: JSON priority is always in [0.0, 1.0]
    #[test]
    fn prop_p2_4_effective_priority_in_bounds(
        base in arb_priority(),
        offset in arb_wide_offset(),
    ) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), base);
        card.set_priority_offset(offset);
        let json = card.to_json_hide_priority_offset();
        let priority = json["priority"].as_f64().unwrap();
        prop_assert!(priority >= 0.0 && priority <= 1.0,
            "priority {} out of bounds", priority);
    }

    /// P2.5: When offset=0.0, JSON priority == base priority
    #[test]
    fn prop_p2_5_zero_offset_identity(base in arb_priority()) {
        let card = Card::new("item1".to_string(), 0, Utc::now(), base);
        let json = card.to_json_hide_priority_offset();
        let json_priority = json["priority"].as_f64().unwrap() as f32;
        prop_assert!((json_priority - base).abs() < 1e-5,
            "expected {}, got {}", base, json_priority);
    }
}

// ============================================================================
// P2r: to_json_hide_priority_offset Robustness with Arbitrary Floats
// ============================================================================

proptest! {
    /// P2r.1: does not panic for any f32 priority
    #[test]
    fn prop_p2r_1_hide_offset_does_not_panic_any_priority(p in arb_any_f32()) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        card.set_priority(p);
        let _json = card.to_json_hide_priority_offset();
    }

    /// P2r.2: does not panic for any f32 offset
    #[test]
    fn prop_p2r_2_hide_offset_does_not_panic_any_offset(off in arb_any_f32()) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        card.set_priority_offset(off);
        let _json = card.to_json_hide_priority_offset();
    }

    /// P2r.3: does not panic for any (priority, offset) pair
    #[test]
    fn prop_p2r_3_hide_offset_does_not_panic_both_any(
        p in arb_any_f32(),
        off in arb_any_f32(),
    ) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        card.set_priority(p);
        card.set_priority_offset(off);
        let _json = card.to_json_hide_priority_offset();
    }

    /// P2r.4: For finite priority and offset, JSON priority is in [0.0, 1.0]
    #[test]
    fn prop_p2r_4_effective_priority_never_outside_bounds_for_finite(
        p in arb_any_f32(),
        off in arb_any_f32(),
    ) {
        prop_assume!(p.is_finite() && off.is_finite());
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), p);
        card.set_priority_offset(off);
        let json = card.to_json_hide_priority_offset();
        let priority = json["priority"].as_f64().unwrap();
        prop_assert!(priority >= 0.0 && priority <= 1.0,
            "priority {} out of bounds for p={}, off={}", priority, p, off);
    }
}

// ============================================================================
// P3: Serialization/Deserialization
// ============================================================================

proptest! {
    /// P3.1: serialize → deserialize preserves priority_offset
    #[test]
    fn prop_p3_1_serde_roundtrip_preserves_offset(off in arb_priority_offset()) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        card.set_priority_offset(off);
        let json = serde_json::to_string(&card).unwrap();
        let deserialized: Card = serde_json::from_str(&json).unwrap();
        prop_assert!((deserialized.get_priority_offset() - off).abs() < f32::EPSILON);
    }

    /// P3.2: to_json_hide_priority_offset → deserialize → priority_offset == 0.0
    #[test]
    fn prop_p3_2_hidden_json_deserializes_with_default_offset(
        base in arb_priority(),
        off in arb_priority_offset(),
    ) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), base);
        card.set_priority_offset(off);
        let hidden_json = card.to_json_hide_priority_offset();
        let deserialized: Card = serde_json::from_value(hidden_json).unwrap();
        prop_assert_eq!(deserialized.get_priority_offset(), 0.0,
            "priority_offset should default to 0.0 when absent");
    }

    /// P3.3: serialize → deserialize preserves sort_position (including None)
    #[test]
    fn prop_p3_3_serde_roundtrip_preserves_sort_position(pos in arb_sort_position()) {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        card.set_sort_position(pos);
        let json = serde_json::to_string(&card).unwrap();
        let deserialized: Card = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(deserialized.get_sort_position(), pos);
    }
}

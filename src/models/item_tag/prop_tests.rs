use super::*;
use crate::test_utils::arb_messy_string;
use proptest::prelude::*;

proptest! {
    /// IT1.1: Arbitrary strings are preserved through constructor
    #[test]
    fn prop_it_tag_1_1_new_preserves_ids(
        item_id in arb_messy_string(),
        tag_id in arb_messy_string(),
    ) {
        let item_tag = ItemTag::new(item_id.clone(), tag_id.clone());
        prop_assert_eq!(item_tag.get_item_id(), item_id);
        prop_assert_eq!(item_tag.get_tag_id(), tag_id);
    }

    /// IT1r.1: Messy strings don't cause panics
    #[test]
    fn prop_it_tag_1r_1_new_does_not_panic(
        item_id in arb_messy_string(),
        tag_id in arb_messy_string(),
    ) {
        let item_tag = ItemTag::new(item_id, tag_id);
        let _ = item_tag.get_item_id();
        let _ = item_tag.get_tag_id();
        let _ = item_tag.get_created_at();
    }

    /// IT2.1: JSON serde roundtrip preserves all fields
    #[test]
    fn prop_it_tag_2_1_serde_roundtrip(
        item_id in arb_messy_string(),
        tag_id in arb_messy_string(),
    ) {
        let item_tag = ItemTag::new(item_id.clone(), tag_id.clone());
        let json_str = serde_json::to_string(&item_tag).unwrap();
        let deserialized: ItemTag = serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(deserialized.get_item_id(), item_id);
        prop_assert_eq!(deserialized.get_tag_id(), tag_id);
        // created_at should roundtrip within 1 second tolerance due to NaiveDateTime precision
        let diff = (item_tag.get_created_at() - deserialized.get_created_at()).num_seconds().abs();
        prop_assert!(diff <= 1, "created_at roundtrip diff too large: {} seconds", diff);
    }
}

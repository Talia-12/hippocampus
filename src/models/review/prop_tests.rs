use super::*;
use crate::test_utils::{arb_datetime_utc, arb_messy_string, arb_rating, arb_invalid_rating};
use proptest::prelude::*;

// ============================================================================
// R1: Constructor Properties
// ============================================================================

proptest! {
    /// R1.1: Review::new produces a valid UUID
    #[test]
    fn prop_r1_1_new_produces_valid_uuid(
        card_id in "\\PC+",
        rating in arb_rating(),
    ) {
        let review = Review::new(&card_id, rating);
        prop_assert!(Uuid::parse_str(&review.get_id()).is_ok(),
            "get_id() should be a valid UUID, got: {}", review.get_id());
    }

    /// R1.2: Review::new preserves card_id
    #[test]
    fn prop_r1_2_new_preserves_card_id(
        card_id in "\\PC+",
        rating in arb_rating(),
    ) {
        let review = Review::new(&card_id, rating);
        prop_assert_eq!(review.get_card_id(), card_id);
    }

    /// R1.3: Review::new preserves rating
    #[test]
    fn prop_r1_3_new_preserves_rating(
        card_id in "\\PC+",
        rating in arb_rating(),
    ) {
        let review = Review::new(&card_id, rating);
        prop_assert_eq!(review.get_rating(), rating);
    }

    /// R1.4: Review::new timestamp is recent
    #[test]
    fn prop_r1_4_new_timestamp_is_recent(
        card_id in "\\PC+",
        rating in arb_rating(),
    ) {
        let review = Review::new(&card_id, rating);
        let diff = (Utc::now() - review.get_review_timestamp()).num_seconds();
        prop_assert!(diff < 2, "Timestamp should be within 2 seconds, got {} seconds", diff);
    }

    /// R1.5: Review::new_with_fields preserves all fields roundtrip
    #[test]
    fn prop_r1_5_new_with_fields_roundtrip(
        id in "\\PC+",
        card_id in "\\PC+",
        rating in any::<i32>(),
        ts in arb_datetime_utc(),
    ) {
        let review = Review::new_with_fields(
            id.clone(),
            card_id.clone(),
            rating,
            ts,
        );
        prop_assert_eq!(review.get_id(), id);
        prop_assert_eq!(review.get_card_id(), card_id);
        prop_assert_eq!(review.get_rating(), rating);
        // DateTime comparison: new_with_fields converts to NaiveDateTime then back,
        // so sub-second precision should be preserved
        let diff = (review.get_review_timestamp() - ts).num_seconds().abs();
        prop_assert!(diff == 0, "Timestamps should match, diff: {} seconds", diff);
    }
}

// ============================================================================
// R1p: Constructor Panic Boundaries
// ============================================================================

#[test]
#[should_panic(expected = "Rating must be between 1 and 4")]
fn prop_r1p_1_new_panics_for_rating_0() {
    let _ = Review::new("card-id", 0);
}

#[test]
#[should_panic(expected = "Rating must be between 1 and 4")]
fn prop_r1p_2_new_panics_for_rating_5() {
    let _ = Review::new("card-id", 5);
}

proptest! {
    /// R1p.3: Review::new panics for any rating outside [1, 4]
    #[test]
    fn prop_r1p_3_new_panics_for_invalid_rating(rating in arb_invalid_rating()) {
        let result = std::panic::catch_unwind(|| {
            Review::new("card-id", rating)
        });
        prop_assert!(result.is_err(),
            "Review::new should panic for rating {}", rating);
    }
}

// ============================================================================
// R1r: Constructor Robustness
// ============================================================================

proptest! {
    /// R1r.1: Review::new does not panic for any string card_id
    #[test]
    fn prop_r1r_1_new_does_not_panic_any_card_id(card_id in arb_messy_string()) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Review::new(&card_id, 2)
        }));
        prop_assert!(result.is_ok(),
            "Review::new should not panic for card_id: {:?}", card_id);
    }

    /// R1r.2: Review::new_with_fields does not panic for arbitrary strings and any i32 rating
    #[test]
    fn prop_r1r_2_new_with_fields_does_not_panic(
        id in arb_messy_string(),
        card_id in arb_messy_string(),
        rating in any::<i32>(),
        ts in arb_datetime_utc(),
    ) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Review::new_with_fields(id.clone(), card_id.clone(), rating, ts)
        }));
        prop_assert!(result.is_ok(),
            "new_with_fields should not panic for id={:?}, card_id={:?}, rating={}",
            id, card_id, rating);
    }
}

// ============================================================================
// R2: Getter/Setter Roundtrips
// ============================================================================

proptest! {
    /// R2.1: set_card_id / get_card_id roundtrip
    #[test]
    fn prop_r2_1_card_id_roundtrip(card_id in arb_messy_string()) {
        let mut review = Review::new("initial", 2);
        review.set_card_id(card_id.clone());
        prop_assert_eq!(review.get_card_id(), card_id);
    }

    /// R2.2: set_rating / get_rating roundtrip
    #[test]
    fn prop_r2_2_rating_roundtrip(rating in any::<i32>()) {
        let mut review = Review::new("card-id", 2);
        review.set_rating(rating);
        prop_assert_eq!(review.get_rating(), rating);
    }

    /// R2.3: set_review_timestamp / get_review_timestamp roundtrip
    #[test]
    fn prop_r2_3_timestamp_roundtrip(ts in arb_datetime_utc()) {
        let mut review = Review::new("card-id", 2);
        review.set_review_timestamp(ts);
        let diff = (review.get_review_timestamp() - ts).num_seconds().abs();
        prop_assert!(diff == 0, "Timestamps should match, diff: {} seconds", diff);
    }
}

// ============================================================================
// R3: Serialization
// ============================================================================

proptest! {
    /// R3.1: serde roundtrip preserves all fields
    #[test]
    fn prop_r3_1_serde_roundtrip(
        card_id in "\\PC+",
        rating in arb_rating(),
    ) {
        let review = Review::new(&card_id, rating);
        let json = serde_json::to_string(&review).unwrap();
        let deserialized: Review = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(review.get_id(), deserialized.get_id());
        prop_assert_eq!(review.get_card_id(), deserialized.get_card_id());
        prop_assert_eq!(review.get_rating(), deserialized.get_rating());
        prop_assert_eq!(
            review.get_review_timestamp_raw(),
            deserialized.get_review_timestamp_raw()
        );
    }
}

// ============================================================================
// R3r: Serialization Robustness
// ============================================================================

proptest! {
    /// R3r.1: Serialization does not panic for any i32 rating stored via set_rating
    #[test]
    fn prop_r3r_1_serialize_does_not_panic_any_rating(rating in any::<i32>()) {
        let mut review = Review::new("card-id", 2);
        review.set_rating(rating);
        let result = serde_json::to_string(&review);
        prop_assert!(result.is_ok(),
            "Serialization should not fail for rating {}", rating);
    }
}

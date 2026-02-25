use super::*;
use super::tests::{card_with_fsrs_data, interval_days_for};
use crate::repo::tests::setup_test_db;
use crate::repo::{create_item, create_item_type};
use crate::test_utils::{
    arb_difficulty, arb_invalid_rating, arb_json, arb_messy_string, arb_rating, arb_stability,
};
use proptest::prelude::*;
use serde_json::json;

// ============================================================================
// Helper: create a card in the database for property tests
// ============================================================================

/// Creates an item type, item, and returns the card for property testing
async fn setup_card(pool: &crate::db::DbPool) -> Card {
    let item_type = create_item_type(pool, "Prop Test Type".to_string(), "fsrs".to_string())
        .await
        .unwrap();
    let item = create_item(
        pool,
        &item_type.get_id(),
        "Prop Test Item".to_string(),
        json!({"front": "Q", "back": "A"}),
    )
    .await
    .unwrap();
    crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap()
}

// ============================================================================
// T1: calculate_next_review Pure-Logic Properties
// ============================================================================

proptest! {
    /// T1.1: Monotonicity — for stability ≥ 5.0, interval(r1) < interval(r2) when r1 < r2
    #[test]
    fn prop_t1_1_monotonicity(
        stability in arb_stability().prop_filter("stability >= 5.0", |s| *s >= 5.0),
        difficulty in arb_difficulty(),
        r1 in arb_rating(),
        r2 in arb_rating(),
    ) {
        prop_assume!(r1 != r2);
        let (lo, hi) = if r1 < r2 { (r1, r2) } else { (r2, r1) };

        let card = card_with_fsrs_data(stability, difficulty);
        let interval_lo = interval_days_for(&card, lo);
        let interval_hi = interval_days_for(&card, hi);

        prop_assert!(
            interval_lo < interval_hi,
            "rating {} interval ({:.2}) should be < rating {} interval ({:.2}), \
             stability={}, difficulty={}",
            lo, interval_lo, hi, interval_hi,
            stability, difficulty,
        );
    }

    /// T1.2: Next review is in the future (allowing small clock drift)
    #[test]
    fn prop_t1_2_next_review_in_future(
        stability in arb_stability(),
        difficulty in arb_difficulty(),
        rating in arb_rating(),
    ) {
        let card = card_with_fsrs_data(stability, difficulty);
        let (next_review, _) = calculate_next_fsrs_review(&card, rating).unwrap();
        let threshold = Utc::now() - Duration::hours(2);
        prop_assert!(
            next_review > threshold,
            "next_review {} should be after {} (now - 2h)",
            next_review, threshold,
        );
    }

    /// T1.3: Scheduler data has exactly stability and difficulty keys
    #[test]
    fn prop_t1_3_scheduler_data_keys(
        stability in arb_stability(),
        difficulty in arb_difficulty(),
        rating in arb_rating(),
    ) {
        let card = card_with_fsrs_data(stability, difficulty);
        let (_, scheduler_data) = calculate_next_fsrs_review(&card, rating).unwrap();
        let obj = scheduler_data.0.as_object().unwrap();
        prop_assert_eq!(obj.len(), 2, "Should have exactly 2 keys, got: {:?}", obj.keys().collect::<Vec<_>>());
        prop_assert!(obj.contains_key("stability"), "Missing stability key");
        prop_assert!(obj.contains_key("difficulty"), "Missing difficulty key");
    }

    /// T1.4: Stability is always positive
    #[test]
    fn prop_t1_4_stability_positive(
        stability in arb_stability(),
        difficulty in arb_difficulty(),
        rating in arb_rating(),
    ) {
        let card = card_with_fsrs_data(stability, difficulty);
        let (_, scheduler_data) = calculate_next_fsrs_review(&card, rating).unwrap();
        let s = scheduler_data.0["stability"].as_f64().unwrap();
        prop_assert!(s > 0.0, "Stability should be positive, got {}", s);
    }

    /// T1.5: Difficulty is always positive
    #[test]
    fn prop_t1_5_difficulty_positive(
        stability in arb_stability(),
        difficulty in arb_difficulty(),
        rating in arb_rating(),
    ) {
        let card = card_with_fsrs_data(stability, difficulty);
        let (_, scheduler_data) = calculate_next_fsrs_review(&card, rating).unwrap();
        let d = scheduler_data.0["difficulty"].as_f64().unwrap();
        prop_assert!(d > 0.0, "Difficulty should be positive, got {}", d);
    }

    /// T1.6: Invalid rating returns Err
    #[test]
    fn prop_t1_6_invalid_rating_returns_err(
        stability in arb_stability(),
        difficulty in arb_difficulty(),
        rating in arb_invalid_rating(),
    ) {
        let card = card_with_fsrs_data(stability, difficulty);
        let result = calculate_next_fsrs_review(&card, rating);
        prop_assert!(result.is_err(),
            "calculate_next_review should return Err for rating {}", rating);
    }

    /// T1.7: Fresh card (no scheduler_data, no last_review) succeeds for all valid ratings
    #[test]
    fn prop_t1_7_fresh_card_succeeds(rating in arb_rating()) {
        let card = Card::new_with_fields(
            "test-id".to_string(),
            "item-id".to_string(),
            0,
            Utc::now(),
            None,
            None,
            0.5,
            None,
        );
        let result = calculate_next_fsrs_review(&card, rating);
        prop_assert!(result.is_ok(),
            "Fresh card should succeed for rating {}, got: {:?}", rating, result.err());
    }
}

// ============================================================================
// T1r: calculate_next_review Robustness
// ============================================================================

proptest! {
    /// T1r.1: Does not panic for arbitrary scheduler_data JSON
    #[test]
    fn prop_t1r_1_arbitrary_json_no_panic(
        json_val in arb_json(),
        rating in arb_rating(),
    ) {
        let card = Card::new_with_fields(
            "test-id".to_string(),
            "item-id".to_string(),
            0,
            Utc::now(),
            Some(Utc::now()),
            Some(JsonValue(json_val)),
            0.5,
            None,
        );
        // May return Err but must not panic
        let _ = calculate_next_fsrs_review(&card, rating);
    }

    /// T1r.2: Does not panic for any i32 rating with valid card
    #[test]
    fn prop_t1r_2_any_rating_no_panic(rating in any::<i32>()) {
        let card = card_with_fsrs_data(10.0, 5.0);
        // May return Err but must not panic
        let _ = calculate_next_fsrs_review(&card, rating);
    }
}

// ============================================================================
// T2: record_review Database Properties
// ============================================================================

// Note: proptest doesn't work with async, so we use the tokio runtime directly
// inside proptest closures.

proptest! {
    /// T2.1: Review count increases by 1 after record_review
    #[test]
    fn prop_t2_1_review_count_increases(rating in arb_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            let before = get_reviews_for_card(&pool, &card.get_id()).unwrap().len();
            record_review(&pool, &card.get_id(), rating).await.unwrap();
            let after = get_reviews_for_card(&pool, &card.get_id()).unwrap().len();

            assert_eq!(after, before + 1, "Review count should increase by 1");
        });
    }

    /// T2.2: Review references correct card
    #[test]
    fn prop_t2_2_review_references_correct_card(rating in arb_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            let review = record_review(&pool, &card.get_id(), rating).await.unwrap();
            assert_eq!(review.get_card_id(), card.get_id());
        });
    }

    /// T2.3: Review has correct rating
    #[test]
    fn prop_t2_3_review_has_correct_rating(rating in arb_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            let review = record_review(&pool, &card.get_id(), rating).await.unwrap();
            assert_eq!(review.get_rating(), rating);
        });
    }

    /// T2.4: Card's scheduler_data is set after review
    #[test]
    fn prop_t2_4_scheduler_data_set(rating in arb_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            record_review(&pool, &card.get_id(), rating).await.unwrap();

            let updated = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            let data = updated.get_scheduler_data();
            assert!(data.is_some(), "scheduler_data should be set after review");
            let obj = data.unwrap().0;
            assert!(obj["stability"].as_f64().is_some(), "Should have stability");
            assert!(obj["difficulty"].as_f64().is_some(), "Should have difficulty");
        });
    }

    /// T2.5: Card's last_review is set after review
    #[test]
    fn prop_t2_5_last_review_set(rating in arb_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            record_review(&pool, &card.get_id(), rating).await.unwrap();

            let updated = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            assert!(updated.get_last_review().is_some(), "last_review should be set");
            let diff = (Utc::now() - updated.get_last_review().unwrap()).num_seconds();
            assert!(diff < 5, "last_review should be recent, diff: {} seconds", diff);
        });
    }

    /// T2.6: Card's next_review moves forward after review with rating >= 2
    #[test]
    fn prop_t2_6_next_review_moves_forward(rating in 2i32..=4i32) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            record_review(&pool, &card.get_id(), rating).await.unwrap();

            let updated = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            let now = Utc::now();
            assert!(updated.get_next_review() > now,
                "next_review {} should be after now {} for rating {}",
                updated.get_next_review(), now, rating);
        });
    }

    /// T2.7: Invalid rating returns Err without side effects
    #[test]
    fn prop_t2_7_invalid_rating_no_side_effects(rating in arb_invalid_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            let before_reviews = get_reviews_for_card(&pool, &card.get_id()).unwrap().len();
            let before_card = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            let result = record_review(&pool, &card.get_id(), rating).await;
            assert!(result.is_err(), "Should fail for rating {}", rating);

            let after_reviews = get_reviews_for_card(&pool, &card.get_id()).unwrap().len();
            let after_card = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            assert_eq!(before_reviews, after_reviews, "Review count should not change");
            assert_eq!(before_card.get_scheduler_data(), after_card.get_scheduler_data(),
                "scheduler_data should not change");
            assert_eq!(before_card.get_last_review(), after_card.get_last_review(),
                "last_review should not change");
        });
    }

    /// T2.8: Nonexistent card_id returns Err
    #[test]
    fn prop_t2_8_nonexistent_card_returns_err(
        card_id in arb_messy_string(),
        rating in arb_rating(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let result = record_review(&pool, &card_id, rating).await;
            assert!(result.is_err(), "Should fail for nonexistent card_id: {:?}", card_id);
        });
    }
}

// ============================================================================
// T3: get_all_next_reviews_for_card Properties
// ============================================================================

proptest! {
    /// T3.1: Returns exactly 4 results (one per rating)
    #[test]
    fn prop_t3_1_returns_four_results(rating in arb_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            // Optionally review first to give the card some state
            if rating > 1 {
                record_review(&pool, &card.get_id(), rating).await.unwrap();
            }

            let results = get_all_next_reviews_for_card(&pool, &card.get_id()).await.unwrap();
            assert_eq!(results.len(), 4, "Should return exactly 4 results");
        });
    }

    /// T3.2: Results are monotonically increasing in next_review time
    #[test]
    fn prop_t3_2_monotonically_increasing(rating in arb_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            if rating > 1 {
                record_review(&pool, &card.get_id(), rating).await.unwrap();
            }

            let results = get_all_next_reviews_for_card(&pool, &card.get_id()).await.unwrap();
            for i in 0..3 {
                assert!(
                    results[i].0 <= results[i + 1].0,
                    "next_review for rating {} ({}) should be <= rating {} ({})",
                    i + 1, results[i].0, i + 2, results[i + 1].0,
                );
            }
        });
    }

    /// T3.3: Read-only: calling get_all_next_reviews_for_card does not modify card state
    #[test]
    fn prop_t3_3_read_only(rating in arb_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            // Record a review to give the card some state
            record_review(&pool, &card.get_id(), rating).await.unwrap();

            let card_before = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            let _ = get_all_next_reviews_for_card(&pool, &card.get_id()).await.unwrap();

            let card_after = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            assert_eq!(card_before, card_after, "Card state should not change");
        });
    }

    /// T3.4: Each result contains valid FSRS data (stability > 0, difficulty > 0)
    #[test]
    fn prop_t3_4_valid_fsrs_data(rating in arb_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            if rating > 1 {
                record_review(&pool, &card.get_id(), rating).await.unwrap();
            }

            let results = get_all_next_reviews_for_card(&pool, &card.get_id()).await.unwrap();
            for (i, (_, data)) in results.iter().enumerate() {
                let obj = data.0.as_object().unwrap();
                let s = obj["stability"].as_f64().unwrap();
                let d = obj["difficulty"].as_f64().unwrap();
                assert!(s > 0.0, "Rating {} stability should be positive, got {}", i + 1, s);
                assert!(d > 0.0, "Rating {} difficulty should be positive, got {}", i + 1, d);
            }
        });
    }
}

// ============================================================================
// T4: get_reviews_for_card Properties
// ============================================================================

proptest! {
    /// T4.1: Returns reviews in descending timestamp order
    #[test]
    fn prop_t4_1_descending_order(num_reviews in 2usize..=5usize) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            for _ in 0..num_reviews {
                record_review(&pool, &card.get_id(), 3).await.unwrap();
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            let reviews = get_reviews_for_card(&pool, &card.get_id()).unwrap();
            for i in 0..reviews.len() - 1 {
                assert!(
                    reviews[i].get_review_timestamp() >= reviews[i + 1].get_review_timestamp(),
                    "Reviews should be in descending order"
                );
            }
        });
    }

    /// T4.2: All returned reviews belong to the queried card
    #[test]
    fn prop_t4_2_reviews_belong_to_card(rating in arb_rating()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            record_review(&pool, &card.get_id(), rating).await.unwrap();

            let reviews = get_reviews_for_card(&pool, &card.get_id()).unwrap();
            for review in &reviews {
                assert_eq!(review.get_card_id(), card.get_id(),
                    "Review should belong to queried card");
            }
        });
    }

    /// T4.3: Returns empty vec for card with no reviews (not an error)
    #[test]
    fn prop_t4_3_empty_for_unreviewed_card(_dummy in 0..1i32) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            let reviews = get_reviews_for_card(&pool, &card.get_id()).unwrap();
            assert!(reviews.is_empty(), "Should return empty vec for unreviewed card");
        });
    }
}

// ============================================================================
// T5: migrate_scheduler_data Properties
// ============================================================================

proptest! {
    /// T5.1: Idempotency — calling migrate twice produces same result as once
    #[test]
    fn prop_t5_1_idempotency(_dummy in 0..1i32) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            // Set SM-2 data
            let sm2_data = JsonValue(json!({
                "ease_factor": 2.5,
                "interval": 10.0,
            }));
            diesel::update(cards::table.find(card.get_id()))
                .set(cards::scheduler_data.eq(Some(sm2_data)))
                .execute(&mut pool.get().unwrap())
                .unwrap();

            // Migrate once
            migrate_scheduler_data(&pool).await.unwrap();
            let after_first = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            // Migrate again
            migrate_scheduler_data(&pool).await.unwrap();
            let after_second = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            assert_eq!(
                after_first.get_scheduler_data(),
                after_second.get_scheduler_data(),
                "Scheduler data should be same after second migration"
            );
        });
    }

    /// T5.2: SM-2 data converted — cards with ease_factor+interval get stability+difficulty
    #[test]
    fn prop_t5_2_sm2_converted(
        ease in 1.3f64..=4.0f64,
        interval in 1.0f64..=365.0f64,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            let sm2_data = JsonValue(json!({
                "ease_factor": ease,
                "interval": interval,
            }));
            diesel::update(cards::table.find(card.get_id()))
                .set(cards::scheduler_data.eq(Some(sm2_data)))
                .execute(&mut pool.get().unwrap())
                .unwrap();

            migrate_scheduler_data(&pool).await.unwrap();

            let updated = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            let data = updated.get_scheduler_data().unwrap().0;
            assert!(data["stability"].as_f64().is_some(), "Should have stability");
            assert!(data["difficulty"].as_f64().is_some(), "Should have difficulty");
            assert!(data.get("ease_factor").is_none(), "Should not have ease_factor");
            assert!(data.get("interval").is_none(), "Should not have interval");
        });
    }

    /// T5.3: Non-SM-2 data untouched — cards with FSRS-format data are not modified
    #[test]
    fn prop_t5_3_fsrs_data_untouched(
        stability in arb_stability(),
        difficulty in arb_difficulty(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            let fsrs_data = JsonValue(json!({
                "stability": stability,
                "difficulty": difficulty,
            }));
            diesel::update(cards::table.find(card.get_id()))
                .set(cards::scheduler_data.eq(Some(fsrs_data.clone())))
                .execute(&mut pool.get().unwrap())
                .unwrap();

            migrate_scheduler_data(&pool).await.unwrap();

            let updated = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            // Compare with tolerance due to f32→JSON→SQLite→JSON→f64 roundtrip precision
            let updated_data = updated.get_scheduler_data().unwrap().0;
            let updated_s = updated_data["stability"].as_f64().unwrap();
            let updated_d = updated_data["difficulty"].as_f64().unwrap();
            assert!((updated_s - stability as f64).abs() < 1e-3,
                "Stability should be approximately unchanged: {} vs {}", updated_s, stability);
            assert!((updated_d - difficulty as f64).abs() < 1e-3,
                "Difficulty should be approximately unchanged: {} vs {}", updated_d, difficulty);
        });
    }

    /// T5.4: Null scheduler_data untouched — cards with null remain null
    #[test]
    fn prop_t5_4_null_data_untouched(_dummy in 0..1i32) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let card = setup_card(&pool).await;

            // Card starts with null scheduler_data
            assert!(card.get_scheduler_data().is_none());

            migrate_scheduler_data(&pool).await.unwrap();

            let updated = crate::schema::cards::table
                .find(card.get_id())
                .first::<Card>(&mut pool.get().unwrap())
                .unwrap();

            assert!(updated.get_scheduler_data().is_none(),
                "Null scheduler_data should remain null after migration");
        });
    }

    /// T5.5: Metadata marker set after migration
    #[test]
    fn prop_t5_5_metadata_marker_set(_dummy in 0..1i32) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();

            migrate_scheduler_data(&pool).await.unwrap();

            let marker: String = metadata::table
                .find("sr-scheduler")
                .select(metadata::value)
                .first::<String>(&mut pool.get().unwrap())
                .unwrap();

            assert_eq!(marker, "fsrs-1", "Metadata marker should be 'fsrs-1'");
        });
    }
}

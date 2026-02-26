use super::*;
use crate::repo::tests::setup_test_db;
use crate::repo::{create_item, create_item_type};
use serde_json::json;

#[tokio::test]
async fn test_record_review() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item
    let item = create_item(
        &pool,
        &item_type.get_id(),
        "Test Item".to_string(),
        json!({"front": "Hello", "back": "World"})
    ).await.unwrap();

    // Get the card created for the item
    let card = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    // Test recording a review
    let rating = 2;
    let review = record_review(&pool, &card.get_id(), rating).await.unwrap();

    assert_eq!(review.get_card_id(), card.get_id());
    assert_eq!(review.get_rating(), rating);

    // Verify that the card was updated
    let updated_card = crate::schema::cards::table
        .find(card.get_id())
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    assert!(updated_card.get_last_review().is_some());
    assert!(updated_card.get_scheduler_data().is_some());

    // The next review should be in the future
    let now = Utc::now();
    let next_review = updated_card.get_next_review();
    assert!(next_review > now);
}


#[tokio::test]
async fn test_get_reviews_for_card() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item
    let item = create_item(
        &pool,
        &item_type.get_id(),
        "Test Item".to_string(),
        json!({"front": "Hello", "back": "World"})
    ).await.unwrap();

    // Get the card created for the item
    let card = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    // Record some reviews
    let review1 = record_review(&pool, &card.get_id(), 2).await.unwrap();

    // We need to wait a moment to ensure the timestamps are different
    std::thread::sleep(std::time::Duration::from_millis(10));

    let review2 = record_review(&pool, &card.get_id(), 3).await.unwrap();

    // Get reviews for the card
    let reviews = get_reviews_for_card(&pool, &card.get_id()).unwrap();

    // Should have 2 reviews, with the most recent first
    assert_eq!(reviews.len(), 2);
    assert_eq!(reviews[0].get_id(), review2.get_id());
    assert_eq!(reviews[1].get_id(), review1.get_id());
}


#[tokio::test]
async fn test_record_review_edge_cases() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item
    let item = create_item(
        &pool,
        &item_type.get_id(),
        "Test Item".to_string(),
        json!({"front": "Hello", "back": "World"})
    ).await.unwrap();

    // Get the card created for the item
    let card = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    // Try an invalid rating
    let result = record_review(&pool, &card.get_id(), 0).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Rating must be between 1 and 4"));

    let result = record_review(&pool, &card.get_id(), 5).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Rating must be between 1 and 4"));

    // Try a non-existent card
    let result = record_review(&pool, "nonexistent-id", 2).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Card not found"));

    // Test different ratings affect the scheduler data correctly

    // First, record a review with rating 1 (again)
    let _review1 = record_review(&pool, &card.get_id(), 1).await.unwrap();
    let card1 = crate::schema::cards::table
        .find(card.get_id())
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    let data1 = card1.get_scheduler_data().unwrap().0;
    assert!(data1["stability"].as_f64().is_some(), "Should have stability");
    assert!(data1["difficulty"].as_f64().is_some(), "Should have difficulty");

    // Create another card
    let item2 = create_item(
        &pool,
        &item_type.get_id(),
        "Test Item 2".to_string(),
        json!({"front": "Hello2", "back": "World2"})
    ).await.unwrap();

    let card2 = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item2.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    // Record a review with rating 4 (easy)
    let _review2 = record_review(&pool, &card2.get_id(), 4).await.unwrap();
    let card2_updated = crate::schema::cards::table
        .find(card2.get_id())
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    let data2 = card2_updated.get_scheduler_data().unwrap().0;
    assert!(data2["stability"].as_f64().unwrap() > 0.0, "Stability should be positive");
    assert!(data2["difficulty"].as_f64().unwrap() > 0.0, "Difficulty should be positive");

    // Create a third card and do multiple reviews
    let item3 = create_item(
        &pool,
        &item_type.get_id(),
        "Test Item 3".to_string(),
        json!({"front": "Hello3", "back": "World3"})
    ).await.unwrap();

    let card3 = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item3.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    // Do a series of "good" reviews
    record_review(&pool, &card3.get_id(), 3).await.unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    record_review(&pool, &card3.get_id(), 3).await.unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    record_review(&pool, &card3.get_id(), 3).await.unwrap();

    let card3_updated = crate::schema::cards::table
        .find(card3.get_id())
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    let data3 = card3_updated.get_scheduler_data().unwrap().0;
    assert!(data3["stability"].as_f64().is_some(), "Should have stability after multiple reviews");
    assert!(data3["difficulty"].as_f64().is_some(), "Should have difficulty after multiple reviews");
}

/// Helper: build a Card with FSRS scheduler data for pure-logic tests
pub(super) fn card_with_fsrs_data(stability: f32, difficulty: f32) -> Card {
    Card::new_with_fields(
        "test-id".to_string(),
        "item-id".to_string(),
        0,
        Utc::now(),
        Some(Utc::now()),
        Some(JsonValue(json!({
            "stability": stability,
            "difficulty": difficulty,
        }))),
        0.5,
        None,
    )
}

/// Extract the interval in days from calculate_next_review's next_review datetime
pub(super) fn interval_days_for(card: &Card, rating: i32) -> f64 {
    let (next_review, _) = calculate_next_fsrs_review(card, rating).unwrap();
    let diff = next_review - Utc::now();
    diff.num_hours() as f64 / 24.0
}

#[test]
fn test_intervals_monotonic_fresh_card() {
    // Card with no scheduler data (first review)
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

    let intervals: Vec<f64> = (1..=4).map(|r| interval_days_for(&card, r)).collect();
    for i in 0..3 {
        assert!(
            intervals[i] < intervals[i + 1],
            "rating {} interval ({:.2}) should be < rating {} interval ({:.2})",
            i + 1, intervals[i], i + 2, intervals[i + 1],
        );
    }
}

#[test]
fn test_intervals_monotonic_various_states() {
    let cases = vec![
        (5.0, 3.0),    // medium stability, low difficulty
        (10.0, 5.0),   // higher stability, medium difficulty
        (10.0, 7.0),   // higher stability, high difficulty
        (30.0, 5.0),   // high stability, medium difficulty
        (50.0, 3.0),   // very high stability, low difficulty
        (100.0, 1.0),  // very high stability, very low difficulty
    ];

    for (stability, difficulty) in cases {
        let card = card_with_fsrs_data(stability, difficulty);
        let intervals: Vec<f64> = (1..=4).map(|r| interval_days_for(&card, r)).collect();

        for i in 0..3 {
            assert!(
                intervals[i] < intervals[i + 1],
                "s={}, d={}: rating {} interval ({:.2}) should be < rating {} interval ({:.2})",
                stability, difficulty,
                i + 1, intervals[i], i + 2, intervals[i + 1],
            );
        }
    }
}

#[tokio::test]
async fn test_migrate_scheduler_data() {
    let pool = setup_test_db();

    // Create an item type and item
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();
    let item = create_item(
        &pool,
        &item_type.get_id(),
        "Test Item".to_string(),
        json!({"front": "Hello", "back": "World"})
    ).await.unwrap();

    // Get the card
    let card = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    // Manually set SM-2 format scheduler data
    let sm2_data = JsonValue(json!({
        "ease_factor": 2.5,
        "interval": 10.0,
        "repetitions": 3,
    }));
    diesel::update(cards::table.find(card.get_id()))
        .set(cards::scheduler_data.eq(Some(sm2_data)))
        .execute(&mut pool.get().unwrap())
        .unwrap();

    // Run migration
    migrate_scheduler_data(&pool).await.unwrap();

    // Check the card was migrated
    let updated_card = crate::schema::cards::table
        .find(card.get_id())
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    let data = updated_card.get_scheduler_data().unwrap().0;
    assert!(data["stability"].as_f64().is_some(), "Should have stability after migration");
    assert!(data["difficulty"].as_f64().is_some(), "Should have difficulty after migration");
    assert!(data.get("ease_factor").is_none(), "Should not have ease_factor after migration");

    // Check metadata marker was set
    let marker: String = metadata::table
        .find("sr-scheduler")
        .select(metadata::value)
        .first::<String>(&mut pool.get().unwrap())
        .unwrap();
    assert_eq!(marker, "fsrs-1");

    // Run migration again - should be a no-op
    migrate_scheduler_data(&pool).await.unwrap();
}

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn intervals_monotonic_for_any_card_state(
            stability in 5.0f32..=365.0,
            difficulty in 1.0f32..=10.0,
            r1 in 1i32..=4i32,
            r2 in 1i32..=4i32,
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
    }
}

// ============================================================================
// Incremental Queue tests
// ============================================================================

/// Helper: build a Card with incremental queue scheduler data for pure-logic tests
pub(super) fn card_with_iq_data(interval: f64, priority: f32) -> Card {
    Card::new_with_fields(
        "test-id".to_string(),
        "item-id".to_string(),
        0,
        Utc::now(),
        Some(Utc::now()),
        Some(JsonValue(json!({ "interval": interval }))),
        priority,
        None,
    )
}

#[tokio::test]
async fn test_record_review_incremental_queue() {
    let pool = setup_test_db();

    // Create an item type with incremental_queue review function
    let item_type = create_item_type(&pool, "IQ Test Type".to_string(), "incremental_queue".to_string())
        .await
        .unwrap();

    // Create an item
    let item = create_item(
        &pool,
        &item_type.get_id(),
        "IQ Item".to_string(),
        json!({"content": "Some content"}),
    )
    .await
    .unwrap();

    // Get the card
    let card = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    // Record a review
    let review = record_review(&pool, &card.get_id(), 3).await.unwrap();
    assert_eq!(review.get_card_id(), card.get_id());
    assert_eq!(review.get_rating(), 3);

    // Verify the card was updated with incremental queue scheduler data
    let updated_card = crate::schema::cards::table
        .find(card.get_id())
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    let data = updated_card.get_scheduler_data().unwrap().0;
    assert!(
        data["interval"].as_f64().is_some(),
        "Should have 'interval' key in scheduler_data"
    );
    assert!(
        data.get("stability").is_none(),
        "Should not have FSRS 'stability' key"
    );
}

#[test]
fn test_incremental_queue_rating_1_resets() {
    let card = card_with_iq_data(30.0, 0.5);
    let (_, scheduler_data) = calculate_next_incremental_queue_review(&card, 1).unwrap();
    let interval = scheduler_data.0["interval"].as_f64().unwrap();
    assert!(
        (interval - 1.0).abs() < f64::EPSILON,
        "Rating 1 should reset interval to 1.0, got {}",
        interval
    );
}

#[test]
fn test_incremental_queue_intervals_bounded() {
    let card = card_with_iq_data(1.0, 0.5);

    // Rating 2: min 2 days
    let (_, data2) = calculate_next_incremental_queue_review(&card, 2).unwrap();
    let interval2 = data2.0["interval"].as_f64().unwrap();
    assert!(
        interval2 >= 2.0,
        "Rating 2 interval should be >= 2.0, got {}",
        interval2
    );

    // Rating 3: min 4 days
    let (_, data3) = calculate_next_incremental_queue_review(&card, 3).unwrap();
    let interval3 = data3.0["interval"].as_f64().unwrap();
    assert!(
        interval3 >= 4.0,
        "Rating 3 interval should be >= 4.0, got {}",
        interval3
    );

    // Rating 4: min 7 days
    let (_, data4) = calculate_next_incremental_queue_review(&card, 4).unwrap();
    let interval4 = data4.0["interval"].as_f64().unwrap();
    assert!(
        interval4 >= 7.0,
        "Rating 4 interval should be >= 7.0, got {}",
        interval4
    );
}

// ============================================================================
// Edge-case / error-path tests
// ============================================================================

use diesel::connection::SimpleConnection;

/// Helper: insert a card via raw SQL with FK constraints disabled,
/// so the card references a nonexistent item_id.
fn insert_orphaned_card(pool: &crate::db::DbPool) -> String {
    let mut conn = pool.get().unwrap();
    conn.batch_execute("PRAGMA foreign_keys = OFF").unwrap();

    let card_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().naive_utc().format("%Y-%m-%d %H:%M:%S").to_string();
    diesel::sql_query(format!(
        "INSERT INTO cards (id, item_id, card_index, next_review, priority, priority_offset) \
         VALUES ('{}', 'nonexistent-item', 0, '{}', 0.5, 0.0)",
        card_id, now,
    ))
    .execute(&mut conn)
    .unwrap();

    conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
    card_id
}

/// record_review returns error when the item→item_type join fails
/// (i.e. the card's item_id doesn't map to an item with a valid item_type)
#[tokio::test]
async fn test_record_review_review_function_lookup_fails() {
    let pool = setup_test_db();
    let card_id = insert_orphaned_card(&pool);

    let result = record_review(&pool, &card_id, 3).await;
    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("Failed to look up review function"),
        "Expected 'Failed to look up review function' error"
    );
}

/// calculate_next_review returns error for an unknown review function
#[test]
fn test_calculate_next_review_unknown_function() {
    let card = card_with_fsrs_data(5.0, 3.0);
    let result = calculate_next_review(&card, "unknown_function", 3);
    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("Unknown review function: unknown_function"),
        "Expected 'Unknown review function' error"
    );
}

/// get_all_next_reviews_for_card returns error for a nonexistent card_id
#[tokio::test]
async fn test_get_all_next_reviews_nonexistent_card() {
    let pool = setup_test_db();
    let result = get_all_next_reviews_for_card(&pool, "nonexistent-card-id").await;
    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("Card not found"),
        "Expected 'Card not found' error"
    );
}

/// get_all_next_reviews_for_card returns error when the review function lookup fails
#[tokio::test]
async fn test_get_all_next_reviews_review_function_lookup_fails() {
    let pool = setup_test_db();
    let card_id = insert_orphaned_card(&pool);

    let result = get_all_next_reviews_for_card(&pool, &card_id).await;
    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("Failed to look up review function"),
        "Expected 'Failed to look up review function' error"
    );
}

/// migrate_scheduler_data_none_fsrs_0 propagates error when memory_state_from_sm2 fails.
///
/// Triggers the error by setting ease_factor to a value that overflows f32 (producing
/// f32::INFINITY), which causes FSRS to return InvalidInput.
#[tokio::test]
async fn test_migrate_none_fsrs_0_memory_state_from_sm2_err() {
    let pool = setup_test_db();

    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string())
        .await
        .unwrap();
    let item = create_item(
        &pool,
        &item_type.get_id(),
        "Test Item".to_string(),
        json!({"front": "a", "back": "b"}),
    )
    .await
    .unwrap();

    let card = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    // Set SM-2 data with ease_factor that overflows f32 → f32::INFINITY
    let bad_sm2_data = JsonValue(json!({
        "ease_factor": 1e40,
        "interval": 10.0,
    }));
    diesel::update(cards::table.find(card.get_id()))
        .set(cards::scheduler_data.eq(Some(bad_sm2_data)))
        .execute(&mut pool.get().unwrap())
        .unwrap();

    let result = migrate_scheduler_data(&pool).await;
    assert!(result.is_err(), "Migration should fail when memory_state_from_sm2 returns Err");
}

/// Cards with NULL scheduler_data are not loaded by the migration query
/// (filtered out by `is_not_null()`), so the migration succeeds and leaves them untouched.
#[tokio::test]
async fn test_migrate_none_fsrs_0_scheduler_data_none_skipped() {
    let pool = setup_test_db();

    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string())
        .await
        .unwrap();
    let item = create_item(
        &pool,
        &item_type.get_id(),
        "Test Item".to_string(),
        json!({"front": "a", "back": "b"}),
    )
    .await
    .unwrap();

    // Card starts with scheduler_data = None (default after creation)
    let card_before = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();
    assert!(card_before.get_scheduler_data().is_none(), "Precondition: scheduler_data is None");

    migrate_scheduler_data(&pool).await.unwrap();

    // Card should still have None scheduler_data — migration skipped it
    let card_after = crate::schema::cards::table
        .find(card_before.get_id())
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();
    assert!(card_after.get_scheduler_data().is_none(), "Card with None scheduler_data should be unchanged");
}

/// Cards whose scheduler_data is not a JSON object (e.g. a JSON array) are
/// skipped by the `as_object()` check and left unchanged.
#[tokio::test]
async fn test_migrate_none_fsrs_0_scheduler_data_not_object_skipped() {
    let pool = setup_test_db();

    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string())
        .await
        .unwrap();
    let item = create_item(
        &pool,
        &item_type.get_id(),
        "Test Item".to_string(),
        json!({"front": "a", "back": "b"}),
    )
    .await
    .unwrap();

    let card = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(item.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    // Set scheduler_data to a JSON array (not an object)
    let array_data = JsonValue(json!([1, 2, 3]));
    diesel::update(cards::table.find(card.get_id()))
        .set(cards::scheduler_data.eq(Some(array_data.clone())))
        .execute(&mut pool.get().unwrap())
        .unwrap();

    migrate_scheduler_data(&pool).await.unwrap();

    // scheduler_data should be unchanged
    let card_after = crate::schema::cards::table
        .find(card.get_id())
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();
    let data = card_after.get_scheduler_data().unwrap();
    assert!(data.0.is_array(), "Non-object scheduler_data should be left unchanged");
}

/// Helper: insert an item type, item, and card using raw SQL to bypass the
/// card_repo type-name matching logic (which doesn't know "Incremental Reading" etc.)
fn insert_raw_item_with_card(
    pool: &crate::db::DbPool,
    type_name: &str,
    item_title: &str,
) -> (String, String, String) {
    let mut conn = pool.get().unwrap();
    let type_id = uuid::Uuid::new_v4().to_string();
    let item_id = uuid::Uuid::new_v4().to_string();
    let card_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().naive_utc().format("%Y-%m-%d %H:%M:%S").to_string();

    diesel::sql_query(format!(
        "INSERT INTO item_types (id, name, created_at, review_function) \
         VALUES ('{}', '{}', '{}', 'fsrs')",
        type_id, type_name, now,
    ))
    .execute(&mut conn)
    .unwrap();

    diesel::sql_query(format!(
        "INSERT INTO items (id, item_type, title, item_data, created_at, updated_at) \
         VALUES ('{}', '{}', '{}', '{{}}', '{}', '{}')",
        item_id, type_id, item_title, now, now,
    ))
    .execute(&mut conn)
    .unwrap();

    diesel::sql_query(format!(
        "INSERT INTO cards (id, item_id, card_index, next_review, priority, priority_offset) \
         VALUES ('{}', '{}', 0, '{}', 0.5, 0.0)",
        card_id, item_id, now,
    ))
    .execute(&mut conn)
    .unwrap();

    (type_id, item_id, card_id)
}

/// migrate_scheduler_data_fsrs_0_fsrs_1 converts IQ item types and their cards
/// when iq_type_ids is not empty.
///
/// Creates item types with the magic names ("Todo", "Incremental Reading"),
/// sets migration state to "fsrs-0", and verifies:
/// - review_function is updated to "incremental_queue"
/// - card scheduler_data is converted from FSRS format (stability) to IQ format (interval)
/// - non-IQ item types are left unchanged
#[tokio::test]
async fn test_migrate_fsrs_0_fsrs_1_with_iq_types() {
    let pool = setup_test_db();

    // Insert IQ-style item types with the magic names the migration looks for.
    // Use raw SQL because card_repo doesn't know "Incremental Reading".
    let (todo_type_id, _todo_item_id, todo_card_id) =
        insert_raw_item_with_card(&pool, "Todo", "Todo Item");
    let (ir_type_id, _ir_item_id, ir_card_id) =
        insert_raw_item_with_card(&pool, "Incremental Reading", "IR Item");

    // Create a normal FSRS type through the normal path
    let fsrs_type = create_item_type(&pool, "Test Flashcards".to_string(), "fsrs".to_string())
        .await
        .unwrap();
    let fsrs_item = create_item(
        &pool, &fsrs_type.get_id(), "Test Flashcard".to_string(), json!({"front": "a", "back": "b"}),
    ).await.unwrap();
    let fsrs_card = crate::schema::cards::table
        .filter(crate::schema::cards::item_id.eq(fsrs_item.get_id()))
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();

    // Set FSRS-format scheduler_data on all cards
    let fsrs_data = |stability: f64| {
        Some(JsonValue(json!({"stability": stability, "difficulty": 5.0})))
    };
    diesel::update(cards::table.find(&todo_card_id))
        .set(cards::scheduler_data.eq(fsrs_data(15.0)))
        .execute(&mut pool.get().unwrap())
        .unwrap();
    diesel::update(cards::table.find(&ir_card_id))
        .set(cards::scheduler_data.eq(fsrs_data(30.0)))
        .execute(&mut pool.get().unwrap())
        .unwrap();
    diesel::update(cards::table.find(fsrs_card.get_id()))
        .set(cards::scheduler_data.eq(fsrs_data(20.0)))
        .execute(&mut pool.get().unwrap())
        .unwrap();

    // Set migration state to fsrs-0 so only fsrs-0 → fsrs-1 runs
    diesel::replace_into(metadata::table)
        .values((metadata::key.eq("sr-scheduler"), metadata::value.eq("fsrs-0")))
        .execute(&mut pool.get().unwrap())
        .unwrap();

    migrate_scheduler_data(&pool).await.unwrap();

    // Verify IQ item types had review_function updated
    let updated_todo_type = crate::schema::item_types::table
        .find(&todo_type_id)
        .first::<crate::models::ItemType>(&mut pool.get().unwrap())
        .unwrap();
    assert_eq!(
        updated_todo_type.get_review_function(), "incremental_queue",
        "Todo review_function should be updated to incremental_queue"
    );

    let updated_ir_type = crate::schema::item_types::table
        .find(&ir_type_id)
        .first::<crate::models::ItemType>(&mut pool.get().unwrap())
        .unwrap();
    assert_eq!(
        updated_ir_type.get_review_function(), "incremental_queue",
        "Incremental Reading review_function should be updated to incremental_queue"
    );

    // Verify normal FSRS type was NOT changed
    let unchanged_fsrs_type = crate::schema::item_types::table
        .find(fsrs_type.get_id())
        .first::<crate::models::ItemType>(&mut pool.get().unwrap())
        .unwrap();
    assert_eq!(
        unchanged_fsrs_type.get_review_function(), "fsrs",
        "Non-IQ type should remain fsrs"
    );

    // Verify IQ cards had scheduler_data converted from stability to interval
    let updated_todo_card = crate::schema::cards::table
        .find(&todo_card_id)
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();
    let todo_data = updated_todo_card.get_scheduler_data().unwrap().0;
    assert!(
        todo_data.get("interval").is_some(),
        "Todo card should have 'interval' key after migration"
    );
    assert_eq!(
        todo_data["interval"].as_f64().unwrap(), 15.0,
        "Todo card interval should equal the original stability"
    );
    assert!(
        todo_data.get("stability").is_none(),
        "Todo card should not have 'stability' key after migration"
    );

    let updated_ir_card = crate::schema::cards::table
        .find(&ir_card_id)
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();
    let ir_data = updated_ir_card.get_scheduler_data().unwrap().0;
    assert_eq!(
        ir_data["interval"].as_f64().unwrap(), 30.0,
        "IR card interval should equal the original stability"
    );

    // Verify FSRS card was NOT changed
    let unchanged_fsrs_card = crate::schema::cards::table
        .find(fsrs_card.get_id())
        .first::<Card>(&mut pool.get().unwrap())
        .unwrap();
    let fsrs_card_data = unchanged_fsrs_card.get_scheduler_data().unwrap().0;
    assert!(
        fsrs_card_data.get("stability").is_some(),
        "FSRS card should still have 'stability' key"
    );
    assert!(
        fsrs_card_data.get("interval").is_none(),
        "FSRS card should not have 'interval' key"
    );

    // Verify metadata was updated to fsrs-1
    let marker: String = metadata::table
        .find("sr-scheduler")
        .select(metadata::value)
        .first::<String>(&mut pool.get().unwrap())
        .unwrap();
    assert_eq!(marker, "fsrs-1");
}

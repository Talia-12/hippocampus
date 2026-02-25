use super::*;
use crate::repo::tests::setup_test_db;
use crate::repo::{create_item, create_item_type};
use serde_json::json;

#[tokio::test]
async fn test_record_review() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();

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
    let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();

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
    let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();

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
    let (next_review, _) = calculate_next_review(card, rating).unwrap();
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
    let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();
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
    assert_eq!(marker, "fsrs-0");

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

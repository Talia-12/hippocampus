use crate::db::{DbPool, ExecuteWithRetry};
use crate::models::{Card, JsonValue, Review};
use crate::schema::{cards, metadata, reviews};
use diesel::prelude::*;
use anyhow::{Result, anyhow};
use chrono::Utc;
use chrono::Duration;
use fsrs::{FSRS, MemoryState};
use tracing::{instrument, debug, info, warn};

/// Records a review for a card
///
/// This function records a review for a card and updates the card's scheduling
/// information based on the result of the review.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card being reviewed
/// * `rating_val` - The rating given during the review (1-3)
///
/// ### Returns
///
/// A Result containing the newly created Review if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database operations fail
/// - The card does not exist
/// - The rating is invalid (not 1-4)
#[instrument(skip(pool), fields(card_id = %card_id, rating = %rating_val))]
pub async fn record_review(pool: &DbPool, card_id: &str, rating_val: i32) -> Result<Review> {
    debug!("Recording new review for card");
    
    let conn = &mut pool.get()?;
    
    // Validate the rating
    if rating_val < 1 || rating_val > 4 {
        warn!("Invalid rating provided: {}", rating_val);
        return Err(anyhow!("Rating must be between 1 and 4, got {}", rating_val));
    }
    
    // Verify that the card exists and get its current data
    let card = cards::table
        .find(card_id)
        .first::<Card>(conn)
        .map_err(|_| {
            debug!("Card not found");
            anyhow!("Card not found")
        })?;
    
    debug!("Found card, creating review");
    
    // Create the review
    let new_review = Review::new(card_id, rating_val);
    
    // Insert the review into the database
    diesel::insert_into(reviews::table)
        .values(new_review.clone())
        .execute_with_retry(conn).await?;
    
    debug!("Calculating next review date");
    
    // Update the card's scheduling information
    let (next_review, scheduler_data) = calculate_next_review(&card, rating_val)?;
    
    debug!("Next review scheduled for: {}", next_review);
    
    // Update the card in the database
    diesel::update(cards::table.find(card_id.to_string()))
        .set((
            cards::last_review.eq(Utc::now().naive_utc()),
            cards::next_review.eq(next_review.naive_utc()),
            cards::scheduler_data.eq(Some(scheduler_data)),
        ))
        .execute_with_retry(conn).await?;
    
    info!("Successfully recorded review with id: {}", new_review.get_id());
    
    // Return the review
    Ok(new_review)
}


/// Calculates the next review date and updated scheduler data for a card
///
/// This function uses the FSRS (Free Spaced Repetition Scheduler) algorithm
/// to determine the optimal next review time based on the card's memory state.
///
/// ### Arguments
///
/// * `card` - The card being reviewed
/// * `rating` - The rating given during the review (1-4)
///
/// ### Returns
///
/// A Result containing a tuple of (next_review, scheduler_data)
///
/// ### Errors
///
/// Returns an error if:
/// - The rating is invalid (not 1-4)
/// - The FSRS computation fails
#[instrument(skip_all, fields(card_id = %card.get_id(), rating = %rating))]
fn calculate_next_review(card: &Card, rating: i32) -> Result<(chrono::DateTime<Utc>, JsonValue)> {
    debug!("Calculating next review date with FSRS algorithm");

    use serde_json::json;

    let fsrs = FSRS::new(Some(&[]))?;

    // Load current FSRS state from scheduler_data
    let current_state: Option<MemoryState> = card.get_scheduler_data().and_then(|data| {
        let obj = data.0.as_object()?;
        Some(MemoryState {
            stability: obj.get("stability")?.as_f64()? as f32,
            difficulty: obj.get("difficulty")?.as_f64()? as f32,
        })
    });

    // Calculate days elapsed since last review
    let days_elapsed = card.get_last_review()
        .map(|lr| (Utc::now() - lr).num_days().max(0) as u32)
        .unwrap_or(0);

    debug!("Current state: {:?}, days_elapsed: {}", current_state, days_elapsed);

    let next_states = fsrs.next_states(current_state, 0.9, days_elapsed)?;

    // Pick the state for the given rating
    let chosen = match rating {
        1 => next_states.again,
        2 => next_states.hard,
        3 => next_states.good,
        4 => next_states.easy,
        _ => {
            warn!("Invalid rating: {}", rating);
            return Err(anyhow!("Invalid rating: {}", rating));
        }
    };

    let next_review = Utc::now()
        + Duration::days(chosen.interval.ceil() as i64)
        - Duration::hours(1);

    let scheduler_data = JsonValue(json!({
        "stability": chosen.memory.stability,
        "difficulty": chosen.memory.difficulty,
    }));

    debug!("Next review scheduled for: {}, stability: {}, difficulty: {}",
           next_review, chosen.memory.stability, chosen.memory.difficulty);

    Ok((next_review, scheduler_data))
}


/// Gets all possible next review dates for a card based on different rating values
///
/// This function calculates what the next review date and scheduler data would be
/// for each possible rating (1-4) without actually recording a review.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to calculate next reviews for
///
/// ### Returns
///
/// A Result containing a vector of tuples (next_review_date, scheduler_data) for each rating
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The card is not found
/// - The calculation fails for any rating
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn get_all_next_reviews_for_card(pool: &DbPool, card_id: &str) -> Result<Vec<(chrono::DateTime<Utc>, JsonValue)>> {
    debug!("Calculating all possible next review dates for card {}", card_id);
    
    let conn = &mut pool.get()?;
    
    // Get the card from the database
    let card = cards::table
        .find(card_id)
        .first::<Card>(conn)
        .map_err(|e| {
            warn!("Failed to find card {}: {}", card_id, e);
            anyhow!("Card not found: {}, error: {}", card_id, e)
        })?;
    
    debug!("Found card, calculating next reviews for all possible ratings");
    
    // Calculate next review for each possible rating (1-4)
    let mut results = Vec::with_capacity(4);
    
    for rating in 1..=4 {
        debug!("Calculating next review for rating {}", rating);
        match calculate_next_review(&card, rating) {
            Ok((next_review, scheduler_data)) => {
                debug!("Rating {}: next review at {}", rating, next_review);
                results.push((next_review, scheduler_data));
            },
            Err(e) => {
                warn!("Failed to calculate next review for rating {}: {}", rating, e);
                return Err(anyhow!("Failed to calculate next review for rating {}: {}", rating, e));
            }
        }
    }
    
    info!("Successfully calculated {} possible next reviews for card {}", results.len(), card_id);
    
    Ok(results)
}


/// Gets all reviews for a card
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to get reviews for
///
/// ### Returns
///
/// A Result containing a vector of Reviews for the card
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
#[instrument(skip(pool), fields(card_id = %card_id))]
pub fn get_reviews_for_card(pool: &DbPool, card_id: &str) -> Result<Vec<Review>> {
    debug!("Getting reviews for card");
    
    let conn = &mut pool.get()?;
    
    let reviews = reviews::table
        .filter(reviews::card_id.eq(card_id))
        .order_by(reviews::review_timestamp.desc())
        .load::<Review>(conn)?;
    
    info!("Retrieved {} reviews for card {}", reviews.len(), card_id);
    
    Ok(reviews)
}


/// Migrates existing SM-2 scheduler data to FSRS format
///
/// This function checks for a migration marker in the `metadata` table, and if
/// not present, converts all cards with SM-2-format `scheduler_data` (containing
/// `ease_factor` and `interval` keys) to FSRS format (containing `stability`
/// and `difficulty` keys).
///
/// Cards with `scheduler_data: null` (never reviewed) are left as-is and will
/// get FSRS state on their first review.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result indicating success
///
/// ### Errors
///
/// Returns an error if database operations fail or FSRS conversion fails
#[instrument(skip(pool))]
pub async fn migrate_scheduler_data(pool: &DbPool) -> Result<()> {
    use serde_json::json;

    let conn = &mut pool.get()?;

    // Check if already migrated
    let current: Option<String> = metadata::table
        .find("sr-scheduler")
        .select(metadata::value)
        .first::<String>(conn)
        .optional()?;

    if current.as_deref() == Some("fsrs-0") {
        debug!("Scheduler data already migrated to FSRS, skipping");
        return Ok(());
    }

    info!("Migrating scheduler data from SM-2 to FSRS");

    let fsrs = FSRS::new(Some(&[]))?;

    // Load all cards with scheduler_data
    let all_cards: Vec<Card> = cards::table
        .filter(cards::scheduler_data.is_not_null())
        .load::<Card>(conn)?;

    let mut migrated_count = 0;
    for card in &all_cards {
        if let Some(data) = card.get_scheduler_data() {
            if let Some(obj) = data.0.as_object() {
                // Only migrate cards that have SM-2 keys
                if let (Some(ease), Some(interval)) = (
                    obj.get("ease_factor").and_then(|v| v.as_f64()),
                    obj.get("interval").and_then(|v| v.as_f64()),
                ) {
                    let memory = fsrs.memory_state_from_sm2(
                        ease as f32, interval as f32, 0.9
                    )?;

                    let new_data = JsonValue(json!({
                        "stability": memory.stability,
                        "difficulty": memory.difficulty,
                    }));

                    diesel::update(cards::table.find(card.get_id()))
                        .set(cards::scheduler_data.eq(Some(new_data)))
                        .execute(conn)?;

                    migrated_count += 1;
                }
            }
        }
    }

    // Mark migration complete
    diesel::replace_into(metadata::table)
        .values((
            metadata::key.eq("sr-scheduler"),
            metadata::value.eq("fsrs-0"),
        ))
        .execute(conn)?;

    info!("Successfully migrated {} cards from SM-2 to FSRS", migrated_count);

    Ok(())
}


#[cfg(test)]
mod tests {
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
    fn card_with_fsrs_data(stability: f32, difficulty: f32) -> Card {
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
    fn interval_days_for(card: &Card, rating: i32) -> f64 {
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
}

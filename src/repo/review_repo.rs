use crate::db::{DbPool, ExecuteWithRetry};
use crate::models::{Card, JsonValue, Review};
use crate::schema::{cards, reviews};
use diesel::prelude::*;
use anyhow::{Result, anyhow};
use chrono::Utc;
use chrono::Duration;
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
/// This function implements a simplified version of the SM-2 algorithm used by
/// Anki and similar spaced repetition software.
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
/// - The card's scheduler data is invalid
#[instrument(skip_all, fields(card_id = %card.get_id(), rating = %rating))]
fn calculate_next_review(card: &Card, rating: i32) -> Result<(chrono::DateTime<Utc>, JsonValue)> {
    debug!("Calculating next review date with SM-2 algorithm");
    
    use serde_json::json;
    
    // Get the current scheduler data or use default values
    let current_data = match card.get_scheduler_data() {
        Some(data) => data,
        None => {
            debug!("No existing scheduler data, using defaults");
            JsonValue(json!({
                "ease_factor": 2.5,
                "interval": 1,
                "repetitions": 0,
            }))
        },
    };
    
    let data = current_data.0.as_object().ok_or_else(|| anyhow!("Invalid scheduler data"))?;
    
    // Extract current values
    let mut ease_factor = data.get("ease_factor")
        .and_then(|v| v.as_f64())
        .unwrap_or(2.5);

    let current_interval = data.get("interval")
        .and_then(|v| v.as_i64())
        .unwrap_or(1) as i32;

    let mut repetitions = data.get("repetitions")
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    debug!("Current values: ease_factor={}, interval={}, repetitions={}",
           ease_factor, current_interval, repetitions);

    let interval;
    
    // Calculate new values based on the rating.
    //
    // For ratings 2-4 (hard/good/easy), intervals form an arithmetic
    // progression in the rating:
    //
    //   interval(r) = base + (r - 2) * step
    //
    // where base = ceil(current_interval * 1.2) and
    //       step = max(1, ceil(current_interval * (ease_factor - 1.2)))
    //
    // Since step >= 1, this is inherently strictly monotonically increasing
    // in the rating, without needing any post-hoc clamping.
    match rating {
        1 => {
            // Rating of 1 means "again" - reset progress
            debug!("Rating 1 (again): Resetting progress");
            repetitions = 0;
            interval = 1;
            ease_factor = (ease_factor - 0.2).max(1.3);
        },
        2 | 3 | 4 => {
            repetitions += 1;
            interval = if repetitions == 1 {
                // First successful review: fixed per-rating intervals
                match rating {
                    2 => 2,
                    3 => 4,
                    4 => 7,
                    _ => unreachable!(),
                }
            } else {
                // Subsequent reviews: arithmetic progression in rating
                let base = (current_interval as f64 * 1.2).ceil() as i32;
                let step = ((current_interval as f64 * (ease_factor - 1.2)).ceil() as i32).max(1);
                base + (rating - 2) * step
            };

            // Adjust ease factor
            match rating {
                2 => ease_factor = (ease_factor - 0.15).max(1.3),
                3 => ease_factor += 0.15,
                4 => ease_factor += 0.15,
                _ => unreachable!(),
            }
        },
        _ => {
            warn!("Invalid rating: {}", rating);
            return Err(anyhow!("Invalid rating: {}", rating));
        }
    }
    
    debug!("New values: ease_factor={}, interval={}, repetitions={}", 
           ease_factor, interval, repetitions);
    
    // Calculate the next review date
    let next_review = Utc::now() + Duration::days(interval as i64);
    
    // Create updated scheduler data
    let scheduler_data = JsonValue(json!({
        "ease_factor": ease_factor,
        "interval": interval,
        "repetitions": repetitions,
    }));
    
    debug!("Next review scheduled for: {}", next_review);
    
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
        
        // Test different ratings affect the interval correctly
        
        // First, record a review with rating 1 (again)
        let _review1 = record_review(&pool, &card.get_id(), 1).await.unwrap();
        let card1 = crate::schema::cards::table
            .find(card.get_id())
            .first::<Card>(&mut pool.get().unwrap())
            .unwrap();
            
        let data1 = card1.get_scheduler_data().unwrap().0;
        assert_eq!(data1["repetitions"], 0);
        assert!(data1["interval"].as_f64().unwrap() <= 1.0);
        
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
        assert_eq!(data2["repetitions"], 1);
        assert!(data2["interval"].as_f64().unwrap() > 0.999, "Interval should be at least a day, got {}", data2["interval"]); // Should be at least a day
        
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
        assert_eq!(data3["repetitions"], 3);
        assert!(data3["interval"].as_f64().unwrap() > 5.0); // Should be several days
    }

    /// Helper: build a Card with the given scheduler data for pure-logic tests
    fn card_with_scheduler_data(ease_factor: f64, interval: i32, repetitions: i32) -> Card {
        Card::new_with_fields(
            "test-id".to_string(),
            "item-id".to_string(),
            0,
            Utc::now(),
            None,
            Some(JsonValue(json!({
                "ease_factor": ease_factor,
                "interval": interval,
                "repetitions": repetitions,
            }))),
            0.5,
            None,
        )
    }

    /// Extract the interval from calculate_next_review's output
    fn interval_for(card: &Card, rating: i32) -> i32 {
        let (_, data) = calculate_next_review(card, rating).unwrap();
        data.0["interval"].as_i64().unwrap() as i32
    }

    #[test]
    fn test_intervals_monotonic_fresh_card() {
        // Card with no scheduler data (defaults: e=2.5, i=1, r=0)
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

        let intervals: Vec<i32> = (1..=4).map(|r| interval_for(&card, r)).collect();
        for i in 0..3 {
            assert!(
                intervals[i] < intervals[i + 1],
                "rating {} interval ({}) should be < rating {} interval ({})",
                i + 1, intervals[i], i + 2, intervals[i + 1],
            );
        }
    }

    #[test]
    fn test_intervals_monotonic_various_states() {
        let cases = vec![
            (2.5, 1, 1),   // after one review
            (2.5, 4, 2),   // after two reviews, interval 4
            (1.3, 3, 2),   // low ease factor
            (2.5, 10, 3),  // longer interval
            (3.0, 25, 5),  // high ease, long interval
            (1.3, 1, 0),   // min ease, fresh
            (1.3, 100, 10),// min ease, very long interval
        ];

        for (ease, interval, reps) in cases {
            let card = card_with_scheduler_data(ease, interval, reps);
            let intervals: Vec<i32> = (1..=4).map(|r| interval_for(&card, r)).collect();

            for i in 0..3 {
                assert!(
                    intervals[i] < intervals[i + 1],
                    "e={}, i={}, r={}: rating {} interval ({}) should be < rating {} interval ({})",
                    ease, interval, reps,
                    i + 1, intervals[i], i + 2, intervals[i + 1],
                );
            }
        }
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn intervals_monotonic_for_any_card_state(
                ease_factor in 1.3f64..=5.0,
                interval in 1i32..=365,
                repetitions in 0i32..=20,
                r1 in 1i32..=4i32,
                r2 in 1i32..=4i32,
            ) {
                prop_assume!(r1 != r2);
                let (lo, hi) = if r1 < r2 { (r1, r2) } else { (r2, r1) };

                let card = card_with_scheduler_data(ease_factor, interval, repetitions);
                let interval_lo = interval_for(&card, lo);
                let interval_hi = interval_for(&card, hi);

                prop_assert!(
                    interval_lo < interval_hi,
                    "rating {} interval ({}) should be < rating {} interval ({}), \
                     ease={}, interval={}, reps={}",
                    lo, interval_lo, hi, interval_hi,
                    ease_factor, interval, repetitions,
                );
            }
        }
    }
}

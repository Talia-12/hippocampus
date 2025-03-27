use crate::db::DbPool;
use crate::models::{Card, JsonValue, Review};
use crate::schema::{cards, reviews};
use diesel::prelude::*;
use anyhow::{Result, anyhow};
use chrono::Utc;
use chrono::Duration;

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
/// - The rating is invalid (not 1-3)
pub fn record_review(pool: &DbPool, card_id: &str, rating_val: i32) -> Result<Review> {
    let conn = &mut pool.get()?;
    
    // Validate the rating
    if rating_val < 1 || rating_val > 3 {
        return Err(anyhow!("Rating must be between 1 and 3, got {}", rating_val));
    }
    
    // Verify that the card exists and get its current data
    let card = cards::table
        .find(card_id)
        .first::<Card>(conn)
        .map_err(|_| anyhow!("Card not found"))?;
    
    // Create the review
    let new_review = Review::new(card_id, rating_val);
    
    // Insert the review into the database
    diesel::insert_into(reviews::table)
        .values(&new_review)
        .execute(conn)?;
    
    // Update the card's scheduling information
    let (next_review, scheduler_data) = calculate_next_review(&card, rating_val)?;
    
    // Update the card in the database
    diesel::update(cards::table.find(card_id))
        .set((
            cards::last_review.eq(Utc::now().naive_utc()),
            cards::next_review.eq(next_review.naive_utc()),
            cards::scheduler_data.eq(Some(scheduler_data)),
        ))
        .execute(conn)?;
    
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
/// * `rating` - The rating given during the review (1-3)
///
/// ### Returns
///
/// A Result containing a tuple of (next_review, scheduler_data)
///
/// ### Errors
///
/// Returns an error if:
/// - The rating is invalid (not 1-3)
/// - The card's scheduler data is invalid
fn calculate_next_review(card: &Card, rating: i32) -> Result<(chrono::DateTime<Utc>, JsonValue)> {
    use serde_json::json;
    
    // Get the current scheduler data or use default values
    let current_data = match card.get_scheduler_data() {
        Some(data) => data,
        None => JsonValue(json!({
            "ease_factor": 2.5,
            "interval": 1,
            "repetitions": 0,
        })),
    };
    
    let data = current_data.0.as_object().ok_or_else(|| anyhow!("Invalid scheduler data"))?;
    
    // Extract current values
    let mut ease_factor = data.get("ease_factor")
        .and_then(|v| v.as_f64())
        .unwrap_or(2.5);
    
    let mut interval = data.get("interval")
        .and_then(|v| v.as_i64())
        .unwrap_or(1) as i32;
    
    let mut repetitions = data.get("repetitions")
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    
    // Calculate new values based on the rating
    match rating {
        1 => {
            // Rating of 1 means "again" - reset progress
            repetitions = 0;
            interval = 1;
            ease_factor = std::cmp::max((ease_factor - 0.2) as i32, 1) as f64;
        },
        2 => {
            // Rating of 2 means "hard" - small increase in interval
            repetitions += 1;
            if repetitions == 1 {
                interval = 1;
            } else if repetitions == 2 {
                interval = 3;
            } else {
                interval = (interval as f64 * 1.2).ceil() as i32;
            }
            ease_factor = std::cmp::max((ease_factor - 0.15) as i32, 1) as f64;
        },
        3 => {
            // Rating of 3 means "easy" - larger increase in interval
            repetitions += 1;
            if repetitions == 1 {
                interval = 1;
            } else if repetitions == 2 {
                interval = 4;
            } else {
                interval = (interval as f64 * ease_factor).ceil() as i32;
            }
            ease_factor += 0.15;
        },
        _ => return Err(anyhow!("Invalid rating: {}", rating)),
    }
    
    // Calculate the next review date
    let next_review = Utc::now() + Duration::days(interval as i64);
    
    // Create updated scheduler data
    let scheduler_data = JsonValue(json!({
        "ease_factor": ease_factor,
        "interval": interval,
        "repetitions": repetitions,
    }));
    
    Ok((next_review, scheduler_data))
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
pub fn get_reviews_for_card(pool: &DbPool, card_id: &str) -> Result<Vec<Review>> {
    let conn = &mut pool.get()?;
    
    let reviews = reviews::table
        .filter(reviews::card_id.eq(card_id))
        .order_by(reviews::review_timestamp.desc())
        .load::<Review>(conn)?;
    
    Ok(reviews)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::tests::setup_test_db;
    use crate::repo::{create_item, create_item_type};
    use serde_json::json;
    
    #[test]
    fn test_record_review() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            json!({"front": "Hello", "back": "World"})
        ).unwrap();
        
        // Get the card created for the item
        let card = crate::schema::cards::table
            .filter(crate::schema::cards::item_id.eq(item.get_id()))
            .first::<Card>(&mut pool.get().unwrap())
            .unwrap();
        
        // Test recording a review
        let rating = 2;
        let review = record_review(&pool, &card.get_id(), rating).unwrap();
        
        assert_eq!(review.get_card_id(), card.get_id());
        assert_eq!(review.get_rating(), rating);
        
        // Verify that the card was updated
        let updated_card = crate::schema::cards::table
            .find(card.get_id())
            .first::<Card>(&mut pool.get().unwrap())
            .unwrap();
        
        assert!(updated_card.get_last_review().is_some());
        assert!(updated_card.get_next_review().is_some());
        assert!(updated_card.get_scheduler_data().is_some());
        
        // The next review should be in the future
        let now = Utc::now();
        let next_review = updated_card.get_next_review().unwrap();
        assert!(next_review > now);
    }
    
    #[test]
    fn test_get_reviews_for_card() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            json!({"front": "Hello", "back": "World"})
        ).unwrap();
        
        // Get the card created for the item
        let card = crate::schema::cards::table
            .filter(crate::schema::cards::item_id.eq(item.get_id()))
            .first::<Card>(&mut pool.get().unwrap())
            .unwrap();
        
        // Record some reviews
        let review1 = record_review(&pool, &card.get_id(), 2).unwrap();
        
        // We need to wait a moment to ensure the timestamps are different
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        let review2 = record_review(&pool, &card.get_id(), 3).unwrap();
        
        // Get reviews for the card
        let reviews = get_reviews_for_card(&pool, &card.get_id()).unwrap();
        
        // Should have 2 reviews, with the most recent first
        assert_eq!(reviews.len(), 2);
        assert_eq!(reviews[0].get_id(), review2.get_id());
        assert_eq!(reviews[1].get_id(), review1.get_id());
    }
    
    #[test]
    fn test_record_review_edge_cases() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            json!({"front": "Hello", "back": "World"})
        ).unwrap();
        
        // Get the card created for the item
        let card = crate::schema::cards::table
            .filter(crate::schema::cards::item_id.eq(item.get_id()))
            .first::<Card>(&mut pool.get().unwrap())
            .unwrap();
        
        // Try an invalid rating
        let result = record_review(&pool, &card.get_id(), 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Rating must be between 1 and 3"));
        
        let result = record_review(&pool, &card.get_id(), 4);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Rating must be between 1 and 3"));
        
        // Try a non-existent card
        let result = record_review(&pool, "nonexistent-id", 2);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Card not found"));
        
        // Test different ratings affect the interval correctly
        
        // First, record a review with rating 1 (again)
        let review1 = record_review(&pool, &card.get_id(), 1).unwrap();
        let card1 = crate::schema::cards::table
            .find(card.get_id())
            .first::<Card>(&mut pool.get().unwrap())
            .unwrap();
            
        let data1 = card1.get_scheduler_data().unwrap().0;
        assert_eq!(data1["repetitions"], 0);
        assert_eq!(data1["interval"], 1);
        
        // Now record a review with rating 3 (easy)
        let review2 = record_review(&pool, &card.get_id(), 3).unwrap();
        let card2 = crate::schema::cards::table
            .find(card.get_id())
            .first::<Card>(&mut pool.get().unwrap())
            .unwrap();
            
        let data2 = card2.get_scheduler_data().unwrap().0;
        assert_eq!(data2["repetitions"], 1);
        assert_eq!(data2["interval"], 1);
        
        // Record another review with rating 3 (easy)
        let review3 = record_review(&pool, &card.get_id(), 3).unwrap();
        let card3 = crate::schema::cards::table
            .find(card.get_id())
            .first::<Card>(&mut pool.get().unwrap())
            .unwrap();
            
        let data3 = card3.get_scheduler_data().unwrap().0;
        assert_eq!(data3["repetitions"], 2);
        assert_eq!(data3["interval"], 4);
    }
} 
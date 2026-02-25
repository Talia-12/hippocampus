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
mod tests;
#[cfg(test)]
mod prop_tests;

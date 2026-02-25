use crate::db::{DbPool, ExecuteWithRetry};
use crate::models::{Card, JsonValue, Review};
use crate::schema::{cards, items, item_types, metadata, reviews};
use diesel::prelude::*;
use anyhow::{Result, anyhow};
use chrono::Utc;
use chrono::Duration;
use fsrs::{FSRS, MemoryState};
use tracing::{instrument, debug, info, warn};

/// Valid review function values
pub const VALID_REVIEW_FUNCTIONS: &[&str] = &["fsrs", "incremental_queue"];

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
    
    // Look up the review_function for this card's item type
    let review_function: String = items::table
        .inner_join(item_types::table.on(item_types::id.eq(items::item_type)))
        .filter(items::id.eq(card.get_item_id()))
        .select(item_types::review_function)
        .first::<String>(conn)
        .map_err(|e| anyhow!("Failed to look up review function: {}", e))?;

    debug!("Calculating next review date using review function: {}", review_function);

    // Update the card's scheduling information
    let (next_review, scheduler_data) = calculate_next_review(&card, &review_function, rating_val)?;
    
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


/// Dispatches to the appropriate review calculation function based on the review function name
///
/// ### Arguments
///
/// * `card` - The card being reviewed
/// * `review_function` - The name of the review function to use
/// * `rating` - The rating given during the review (1-4)
///
/// ### Returns
///
/// A Result containing a tuple of (next_review, scheduler_data)
///
/// ### Errors
///
/// Returns an error if the review function is unknown, the rating is invalid, or computation fails
#[instrument(skip_all, fields(card_id = %card.get_id(), review_function = %review_function, rating = %rating))]
fn calculate_next_review(card: &Card, review_function: &str, rating: i32) -> Result<(chrono::DateTime<Utc>, JsonValue)> {
    match review_function {
        "fsrs" => calculate_next_fsrs_review(card, rating),
        "incremental_queue" => calculate_next_incremental_queue_review(card, rating),
        _ => Err(anyhow!("Unknown review function: {}", review_function)),
    }
}

/// Calculates the next review date using the FSRS algorithm
///
/// ### Arguments
///
/// * `card` - The card being reviewed
/// * `rating` - The rating given during the review (1-4)
///
/// ### Returns
///
/// A Result containing a tuple of (next_review, scheduler_data)
#[instrument(skip_all, fields(card_id = %card.get_id(), rating = %rating))]
fn calculate_next_fsrs_review(card: &Card, rating: i32) -> Result<(chrono::DateTime<Utc>, JsonValue)> {
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

/// Calculates the next review date using the incremental queue algorithm
///
/// This scheduler is designed for Todo/Incremental Reading/Incremental Watching
/// item types where priority controls how often content is revisited.
///
/// ### Arguments
///
/// * `card` - The card being reviewed
/// * `rating` - The rating given during the review (1-4)
///
/// ### Returns
///
/// A Result containing a tuple of (next_review, scheduler_data)
#[instrument(skip_all, fields(card_id = %card.get_id(), rating = %rating))]
fn calculate_next_incremental_queue_review(card: &Card, rating: i32) -> Result<(chrono::DateTime<Utc>, JsonValue)> {
    debug!("Calculating next review date for incremental queue");

    use serde_json::json;

    let current_data = match card.get_scheduler_data() {
        Some(data) => data,
        None => JsonValue(json!({ "interval": 1.0 })),
    };

    let data = current_data.0
        .as_object()
        .ok_or_else(|| anyhow!("Invalid scheduler data"))?;

    let current_interval = data
        .get("interval")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);

    let priority = card.get_priority() as f64;

    // Priority controls the growth rate of intervals.
    //   priority 1.0 (highest) -> multiplier ~1.2 (slow growth, seen often)
    //   priority 0.0 (lowest)  -> multiplier ~3.0 (fast growth, fades away)
    const GROWTH_AT_MAX_PRIORITY: f64 = 1.2;
    const GROWTH_AT_MIN_PRIORITY: f64 = 3.0;
    let base_multiplier =
        GROWTH_AT_MIN_PRIORITY - priority * (GROWTH_AT_MIN_PRIORITY - GROWTH_AT_MAX_PRIORITY);

    // Jitter +/-15% to prevent clustering
    let jitter = 1.0 + (rand::random::<f64>() - 0.5) * 0.3;

    // Rating semantics:
    //   1 (again) -> reset to 1.0 day
    //   2 (hard)  -> sooner than default (min 2 days)
    //   3 (good)  -> normal pace (min 4 days)
    //   4 (easy)  -> longer interval (min 7 days)
    let new_interval = match rating {
        1 => 1.0,
        2 => (current_interval * base_multiplier * 0.6 * jitter).max(2.0),
        3 => (current_interval * base_multiplier * jitter).max(4.0),
        4 => (current_interval * base_multiplier * 1.8 * jitter).max(7.0),
        _ => return Err(anyhow!("Invalid rating: {}", rating)),
    };

    let next_review = Utc::now()
        + Duration::days(new_interval.ceil() as i64)
        - Duration::hours(1);

    let scheduler_data = JsonValue(json!({ "interval": new_interval }));

    debug!(
        "IR scheduling: priority={:.2}, base_mult={:.2}, rating={}, interval {:.1} -> {:.1} days",
        priority, base_multiplier, rating, current_interval, new_interval
    );

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

    // Look up the review_function for this card's item type
    let review_function: String = items::table
        .inner_join(item_types::table.on(item_types::id.eq(items::item_type)))
        .filter(items::id.eq(card.get_item_id()))
        .select(item_types::review_function)
        .first::<String>(conn)
        .map_err(|e| anyhow!("Failed to look up review function: {}", e))?;

    debug!("Found card, calculating next reviews for all possible ratings using {}", review_function);

    // Calculate next review for each possible rating (1-4)
    let mut results = Vec::with_capacity(4);

    for rating in 1..=4 {
        debug!("Calculating next review for rating {}", rating);
        match calculate_next_review(&card, &review_function, rating) {
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


/// Migrates SM-2 scheduler data to FSRS format (none -> fsrs-0)
///
/// Converts all cards with SM-2-format `scheduler_data` (containing
/// `ease_factor` and `interval` keys) to FSRS format (containing `stability`
/// and `difficulty` keys).
fn migrate_scheduler_data_none_fsrs_0(conn: &mut diesel::SqliteConnection) -> Result<()> {
    use serde_json::json;

    info!("Migrating scheduler data from SM-2 to FSRS (none -> fsrs-0)");

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

    info!("Successfully migrated {} cards from SM-2 to FSRS", migrated_count);
    Ok(())
}

/// Migrates FSRS data for incremental queue item types (fsrs-0 -> fsrs-1)
///
/// Sets review_function to "incremental_queue" for Todo, Incremental Reading,
/// and Incremental Watching item types. Converts their cards' FSRS scheduler_data
/// (stability) to incremental queue format (interval).
fn migrate_scheduler_data_fsrs_0_fsrs_1(conn: &mut diesel::SqliteConnection) -> Result<()> {
    use serde_json::json;

    info!("Migrating incremental queue item types (fsrs-0 -> fsrs-1)");

    let iq_type_names = ["Todo", "Incremental Reading", "Incremental Watching"];

    // Find item types that should use incremental_queue
    let iq_types: Vec<crate::models::ItemType> = item_types::table
        .filter(item_types::name.eq_any(&iq_type_names))
        .load::<crate::models::ItemType>(conn)?;

    let iq_type_ids: Vec<String> = iq_types.iter().map(|it| it.get_id()).collect();

    if iq_type_ids.is_empty() {
        info!("No incremental queue item types found, skipping fsrs-0 -> fsrs-1 migration");
        return Ok(());
    }

    // Update review_function for these item types
    diesel::update(item_types::table.filter(item_types::id.eq_any(&iq_type_ids)))
        .set(item_types::review_function.eq("incremental_queue"))
        .execute(conn)?;

    info!("Set review_function to 'incremental_queue' for {} item types", iq_type_ids.len());

    // Convert FSRS scheduler_data to incremental queue format for cards of these types
    let iq_cards: Vec<Card> = cards::table
        .inner_join(items::table.on(items::id.eq(cards::item_id)))
        .filter(items::item_type.eq_any(&iq_type_ids))
        .filter(cards::scheduler_data.is_not_null())
        .select(cards::all_columns)
        .load::<Card>(conn)?;

    let mut converted_count = 0;
    for card in &iq_cards {
        if let Some(data) = card.get_scheduler_data() {
            if let Some(obj) = data.0.as_object() {
                if let Some(stability) = obj.get("stability").and_then(|v| v.as_f64()) {
                    let new_data = JsonValue(json!({ "interval": stability }));
                    diesel::update(cards::table.find(card.get_id()))
                        .set(cards::scheduler_data.eq(Some(new_data)))
                        .execute(conn)?;
                    converted_count += 1;
                }
            }
        }
    }

    info!("Converted {} cards to incremental queue format", converted_count);
    Ok(())
}

/// Orchestrates scheduler data migration
///
/// Checks the current migration state in the `metadata` table and runs
/// the appropriate migration steps:
/// - If "fsrs-1": already done, skip
/// - If "fsrs-0": run fsrs-0 -> fsrs-1 only
/// - Otherwise (None): run none -> fsrs-0 then fsrs-0 -> fsrs-1
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
/// Returns an error if database operations fail or migration fails
#[instrument(skip(pool))]
pub async fn migrate_scheduler_data(pool: &DbPool) -> Result<()> {
    let conn = &mut pool.get()?;

    // Check current migration state
    let current: Option<String> = metadata::table
        .find("sr-scheduler")
        .select(metadata::value)
        .first::<String>(conn)
        .optional()?;

    match current.as_deref() {
        Some("fsrs-1") => {
            debug!("Scheduler data already at fsrs-1, skipping");
            return Ok(());
        }
        Some("fsrs-0") => {
            info!("Running migration fsrs-0 -> fsrs-1");
            migrate_scheduler_data_fsrs_0_fsrs_1(conn)?;
        }
        _ => {
            info!("Running full migration: none -> fsrs-0 -> fsrs-1");
            migrate_scheduler_data_none_fsrs_0(conn)?;
            migrate_scheduler_data_fsrs_0_fsrs_1(conn)?;
        }
    }

    // Mark migration complete at fsrs-1
    diesel::replace_into(metadata::table)
        .values((
            metadata::key.eq("sr-scheduler"),
            metadata::value.eq("fsrs-1"),
        ))
        .execute(conn)?;

    info!("Scheduler data migration complete (now at fsrs-1)");
    Ok(())
}


#[cfg(test)]
mod tests;
#[cfg(test)]
mod prop_tests;

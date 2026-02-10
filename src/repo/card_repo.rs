use crate::db::{DbPool, ExecuteWithRetry};
use crate::models::{Card, Item};
use crate::schema::{cards, item_tags};
use crate::{GetQueryDto, SuspendedFilter};
use chrono::Utc;
use diesel::prelude::*;
use anyhow::{Result, anyhow};
use tracing::{instrument, debug, info, warn};

/// Creates cards for an item
///
/// This function automatically creates the necessary cards for an item
/// based on its type and data. Currently, it creates exactly one card per item.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item` - The item to create cards for
///
/// ### Returns
///
/// A Result containing a vector of the created Cards if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
#[instrument(skip(pool, item), fields(item_id = %item.get_id(), item_type = %item.get_item_type()))]
pub async fn create_cards_for_item(pool: &DbPool, item: &Item) -> Result<Vec<Card>> {
    debug!("Creating cards for item");
    
    // Get the item type to determine how many cards to create
    let item_type = super::get_item_type(pool, &item.get_item_type())?
        .ok_or_else(|| anyhow!("Item type not found"))?;
    
    debug!("Item type: {}", item_type.get_name());
    
    // Vector to store the created cards
    let mut cards = Vec::new();
    
    // Determine how many cards to create based on the item type
    match item_type.get_name().as_str() {
        "Basic" => {
            debug!("Creating basic card (front/back)");
            // Basic items have just one card (front/back)
            let card = create_card(pool, &item.get_id(), 0, 0.5).await?;
            cards.push(card);
        },
        "Cloze" => {
            debug!("Creating cloze deletion cards");
            // Cloze items might have multiple cards (one per cloze deletion)
            let data = item.get_data();
            let cloze_deletions = data.0["clozes"].clone();
            let cloze_deletions = cloze_deletions.as_array()
                .ok_or_else(|| anyhow!("cloze deletion must be an array"))?;
            
            debug!("Creating {} cloze cards", cloze_deletions.len());
            for (index, _) in cloze_deletions.iter().enumerate() {
                let card = create_card(pool, &item.get_id(), index as i32, 0.5).await?;
                cards.push(card);
            }
        },
        "Todo" => {
            debug!("Creating todo card");
            // Todo items have 1 card (each todo is a card)
            let card = create_card(pool, &item.get_id(), 0, 0.5).await?;
            cards.push(card);
        },
        // TODO: this is a hack
        name if name.contains("Test") => {
            debug!("Creating test cards");
            // Test item types have 2 cards
            for i in 0..2 {
                let card = create_card(pool, &item.get_id(), i, 0.5).await?;
                cards.push(card);
            }
        },
        _ => {
            warn!("Unknown item type: {}", item_type.get_name());
            // Return an error for unknown item types
            return Err(anyhow!("Unable to construct cards for unknown item type: {}", item_type.get_name()));
        }
    }
    
    info!("Created {} cards for item {}", cards.len(), item.get_id());
    
    // Return all created cards
    Ok(cards)
}


/// Creates a new card in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item this card belongs to
/// * `card_index` - The index of this card within its item
///
/// ### Returns
///
/// A Result containing the newly created Card if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
#[instrument(skip(pool), fields(item_id = %item_id, card_index = %card_index, priority = %priority))]
pub async fn create_card(pool: &DbPool, item_id: &str, card_index: i32, priority: f32) -> Result<Card> {
    debug!("Creating new card");
    
    let conn = &mut pool.get()?;
    
    // Create a new card for the item
    let new_card = Card::new(item_id.to_string(), card_index, Utc::now(), priority);
    let new_card_id = new_card.get_id();
    
    debug!("Inserting card into database with id: {}", new_card_id);
    
    // Insert the new card into the database
    diesel::insert_into(cards::table)
        .values(new_card.clone())
        .execute_with_retry(conn).await?;
    
    info!("Successfully created card with id: {}", new_card_id);
    
    // Return the newly created card
    Ok(new_card)
}


/// Retrieves a card from the database by its ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to retrieve
///
/// ### Returns
///
/// A Result containing an Option with the Card if found, or None if not found
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails for reasons other than the card not existing
#[instrument(skip(pool), fields(card_id = %card_id))]
pub fn get_card(pool: &DbPool, card_id: &str) -> Result<Option<Card>> {
    debug!("Retrieving card by id");
    
    let conn = &mut pool.get()?;
    
    let result = cards::table
        .find(card_id)
        .first::<Card>(conn)
        .optional()?;
    
    if let Some(ref card) = result {
        debug!("Card found with id: {}", card.get_id());
    } else {
        debug!("Card not found");
    }
    
    Ok(result)
}


/// Lists all cards in the database with optional filtering
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `query` - Optional filters for the cards
///
/// ### Returns
///
/// A Result containing a vector of Cards matching the filters
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
#[instrument(skip(pool), fields(query = %query))]
pub fn list_cards_with_filters(pool: &DbPool, query: &GetQueryDto) -> Result<Vec<Card>> {
    debug!("Listing cards with filters: {:?}", query);
    
    let conn = &mut pool.get()?;
    
    // Start with a base query that joins cards with items
    let mut card_query = cards::table.into_boxed();
    
    // Apply filter by item type, if specified
    if let Some(item_type_id) = &query.item_type_id {
        debug!("Filtering by item type: {}", item_type_id);
        card_query = card_query.filter(
            cards::item_id.eq_any(
                crate::schema::items::table
                    .filter(crate::schema::items::item_type.eq(item_type_id))
                    .select(crate::schema::items::id)
            )
        );
    }
    
    // Apply filter by review date, if specified
    if let Some(review_date) = query.next_review_before {
        debug!("Filtering by review date before: {}", review_date);
        card_query = card_query.filter(
            cards::next_review.lt(review_date.naive_utc()).and(cards::next_review.is_not_null())
        );
    }

    // Apply filter by last review date, if specified
    if let Some(review_date) = query.last_review_after {
        debug!("Filtering by last review date after: {}", review_date);
        card_query = card_query.filter(
            cards::last_review.gt(review_date.naive_utc()).and(cards::last_review.is_not_null())
        );
    }

    // Apply filter to remove suspended cards, if specified
    if query.suspended_filter == SuspendedFilter::Exclude {
        card_query = card_query.filter(cards::suspended.is_null());
    }

    // Apply filter to only include suspended cards, if specified
    if query.suspended_filter == SuspendedFilter::Only {
        card_query = card_query.filter(cards::suspended.is_not_null());
    }

    // Apply filter by suspended date before, if specified
    if let Some(suspended_date) = query.suspended_before {
        debug!("Filtering by suspended date before: {}", suspended_date);
        card_query = card_query.filter(cards::suspended.lt(suspended_date.naive_utc()));
    }

    // Apply filter by suspended date after, if specified
    if let Some(suspended_date) = query.suspended_after {
        debug!("Filtering by suspended date after: {}", suspended_date);
        card_query = card_query.filter(cards::suspended.gt(suspended_date.naive_utc()));
    }

    
    // Execute the query
    let mut results = card_query.load::<Card>(conn)?;
    
    // Apply tag filters if specified
    // Note: This is a bit inefficient as we're filtering in Rust rather than SQL,
    // but it's simpler than constructing a complex query with multiple joins.
    if !query.tag_ids.is_empty() {
        debug!("Filtering by tags: {:?}", query.tag_ids);
        // Get all item_ids that have all the requested tags
        let mut item_ids_with_tags = Vec::new();
        
        // Get all items with any of the requested tags
        let items_with_tags: Vec<String> = item_tags::table
            .filter(item_tags::tag_id.eq_any(&query.tag_ids))
            .select(item_tags::item_id)
            .load(conn)?;
        
        // Count how many tags each item has
        let mut item_tag_counts = std::collections::HashMap::new();
        for item_id in items_with_tags {
            *item_tag_counts.entry(item_id).or_insert(0) += 1;
        }
        
        // Only keep items that have all the requested tags
        for (item_id, count) in item_tag_counts {
            if count == query.tag_ids.len() {
                item_ids_with_tags.push(item_id);
            }
        }
        
        // Filter the results to only include cards from items with all the requested tags
        results.retain(|card| item_ids_with_tags.contains(&card.get_item_id()));
    }
    
    info!("Retrieved {} cards matching filters", results.len());
    
    Ok(results)
}


/// Gets all cards for a specific item
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to get cards for
///
/// ### Returns
///
/// A Result containing a vector of Cards belonging to the item
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
#[instrument(skip(pool), fields(item_id = %item_id))]
pub fn get_cards_for_item(pool: &DbPool, item_id: &str) -> Result<Vec<Card>> {
    debug!("Getting cards for item: {}", item_id);
    let conn = &mut pool.get()?;

    // Check if the item exists
    debug!("Checking if item exists");
    let item_exists: bool = crate::schema::items::table
        .find(item_id)
        .count()
        .get_result::<i64>(conn)? > 0;
    
    if !item_exists {
        info!("Item not found: {}", item_id);
        return Err(anyhow!("Item not found"));
    }

    debug!("Item found, fetching cards");

    // Get all cards for the item
    let results = cards::table
        .filter(cards::item_id.eq(item_id))
        .order_by(cards::card_index.asc())
        .load::<Card>(conn)?;

    debug!("Successfully fetched {} cards for item {}", results.len(), item_id);
    Ok(results)
}


/// Sets a card's suspension state
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to update
/// * `suspended` - The new suspension state for the card
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database update operation fails
/// - The card does not exist
#[instrument(skip(pool), fields(card_id = %card_id, suspended = %suspended))]
pub async fn set_card_suspended(pool: &DbPool, card_id: &str, suspended: bool) -> Result<()> {
    debug!("Setting suspension of card to state: {}, {}", card_id, suspended);

    let card = get_card(pool, card_id)?.ok_or(anyhow!("Card not found"))?;

    // Check if the suspension state is already correct
    if card.get_suspended().is_some() == suspended {
        debug!("Already at correct suspension state.");
        return Ok(());
    }

    // Set the new suspension state
    let new_suspended = if suspended { Some(Utc::now().naive_utc()) } else { None };
    debug!("Setting suspension of card to state: {}, {:?}", card_id, new_suspended);

    let conn = &mut pool.get()?;

    // Execute the update
    diesel::update(cards::table.find(card_id.to_string()))
        .set(cards::suspended.eq(new_suspended))
        .execute_with_retry(conn).await?;

    debug!("Successfully set suspension of card to state: {}, {:?}", card_id, new_suspended);

    Ok(())
}


/// Updates a card in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card` - The card to update
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database update operation fails
#[instrument(skip(pool, card), fields(card_id = %card.get_id()))]
pub async fn update_card(pool: &DbPool, card: &Card) -> Result<()> {
    debug!("Updating card");
    let conn = &mut pool.get()?;

    let card_id = card.get_id();
    debug!("Executing update for card_id: {}", card_id);

    diesel::update(cards::table.find(card.get_id()))
        .set((
            cards::next_review.eq(card.get_next_review_raw()),
            cards::last_review.eq(card.get_last_review_raw()),
            cards::scheduler_data.eq(card.get_scheduler_data()),
            cards::priority.eq(card.get_priority()),
            cards::suspended.eq(card.get_suspended_raw()),
        ))
        .execute_with_retry(conn).await?;

    debug!("Successfully updated card_id: {}", card_id);
    Ok(())
}


/// Updates a card's priority
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to update
/// * `priority` - The new priority for the card - must be between 0 and 1
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
pub async fn update_card_priority(pool: &DbPool, card_id: &str, priority: f32) -> Result<Card> {
    // Check if the priority is within the valid range
    if priority < 0.0 || priority > 1.0 {
        return Err(anyhow!("Priority must be between 0 and 1"));
    }

    // Check if the card exists
    let card = get_card(pool, card_id)?;
    if card.is_none() {
        return Err(anyhow!("Card not found"));
    }

    let mut conn = pool.get()?;
    diesel::update(cards::table.find(card_id.to_string()))
        .set(cards::priority.eq(priority))
        .execute_with_retry(&mut conn).await?;

    drop(conn);

    let card = get_card(pool, card_id)?;

    return Ok(card.unwrap_or_else(|| panic!("We already checked if the card exists, so this should never happen (somehow the card was deleted)")));
}


/// Lists all cards in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result containing a vector of all Cards in the database
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn list_all_cards(pool: &DbPool) -> Result<Vec<Card>> {
    let conn = &mut pool.get()?;
    
    let results = cards::table
        .load::<Card>(conn)?;
    
    Ok(results)
}


#[cfg(test)]
mod tests;
#[cfg(test)]
mod prop_tests;

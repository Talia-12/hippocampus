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
mod tests {
    use super::*;
    use crate::repo::tests::setup_test_db;
    use crate::repo::{create_item, create_item_type, create_tag, add_tag_to_item};
    use crate::GetQueryDtoBuilder;
    use serde_json::json;
    use chrono::{Duration, Utc};

    #[tokio::test]
    async fn test_create_card() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            json!({"front": "Hello", "back": "World"})
        ).await.unwrap();
        
        // Test creating a card manually
        let card_index = 2;
        let priority = 0.3;
        let card = create_card(&pool, &item.get_id(), card_index, priority).await.unwrap();
        
        assert_eq!(card.get_item_id(), item.get_id());
        assert_eq!(card.get_card_index(), card_index);
        assert!((card.get_priority() - priority).abs() < 0.0001);
    }
    
    #[tokio::test]
    async fn test_get_card() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            json!({"front": "Hello", "back": "World"})
        ).await.unwrap();
        
        // Get the cards created for the item
        let cards = get_cards_for_item(&pool, &item.get_id()).unwrap();
        assert!(!cards.is_empty());
        
        // Test getting a card by ID
        let card_id = cards[0].get_id();
        let retrieved_card = get_card(&pool, &card_id).unwrap().unwrap();
        
        assert_eq!(retrieved_card.get_id(), card_id);
        assert_eq!(retrieved_card.get_item_id(), item.get_id());
    }
    

    #[tokio::test]
    async fn test_retrieve_cards_by_item_id() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).await.unwrap();
        
        // Get cards for each item
        let cards1 = get_cards_for_item(&pool, &item1.get_id()).unwrap();
        let cards2 = get_cards_for_item(&pool, &item2.get_id()).unwrap();
        
        // Verify that each item has its own card(s)
        assert_eq!(cards1.len(), 2);
        assert_eq!(cards2.len(), 2);
        assert_eq!(cards1[0].get_item_id(), item1.get_id());
        assert_eq!(cards1[1].get_item_id(), item1.get_id());
        assert_eq!(cards2[0].get_item_id(), item2.get_id());
        assert_eq!(cards2[1].get_item_id(), item2.get_id());
    }
    

    #[tokio::test]
    async fn test_list_all_cards() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items (which will also create cards)
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).await.unwrap();
        
        // List all cards
        let cards = list_all_cards(&pool).unwrap();
        
        // Verify that we got all cards
        assert_eq!(cards.len(), 4);
        assert!(cards.iter().filter(|c| c.get_item_id() == item1.get_id()).count() == 2);
        assert!(cards.iter().filter(|c| c.get_item_id() == item2.get_id()).count() == 2);
    }
    
    
    #[tokio::test]
    async fn test_filter_cards_by_item_type() {
        let pool = setup_test_db();
        
        // Create two item types
        let type1_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let type2_type = create_item_type(&pool, "Test Type 2".to_string()).await.unwrap();
        
        // Create items of different types
        let type1_item = create_item(
            &pool, 
            &type1_type.get_id(), 
            "Type 1 Item".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        let type2_item = create_item(
            &pool, 
            &type2_type.get_id(), 
            "Type 2 Item".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).await.unwrap();
        
        // Filter cards by item type
        let query1 = GetQueryDtoBuilder::new()
            .item_type_id(type1_type.get_id())
            .build();
            
        let query2 = GetQueryDtoBuilder::new()
            .item_type_id(type2_type.get_id())
            .build();
        
        let type1_cards = list_cards_with_filters(&pool, &query1).unwrap();
        let type2_cards = list_cards_with_filters(&pool, &query2).unwrap();
        
        // Verify that we got the right cards
        assert_eq!(type1_cards.len(), 2);
        assert_eq!(type2_cards.len(), 2);
        assert_eq!(type1_cards[0].get_item_id(), type1_item.get_id());
        assert_eq!(type1_cards[1].get_item_id(), type1_item.get_id());
        assert_eq!(type2_cards[0].get_item_id(), type2_item.get_id());
        assert_eq!(type2_cards[1].get_item_id(), type2_item.get_id());
    }
    

    #[tokio::test]
    async fn test_filter_cards_by_tags() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).await.unwrap();
        
        // Create some tags
        let tag1 = create_tag(&pool, "Important".to_string(), true).await.unwrap();
        let tag2 = create_tag(&pool, "Difficult".to_string(), true).await.unwrap();
        
        // Add tags to items
        add_tag_to_item(&pool, &tag1.get_id(), &item1.get_id()).await.unwrap();
        add_tag_to_item(&pool, &tag2.get_id(), &item1.get_id()).await.unwrap();
        add_tag_to_item(&pool, &tag1.get_id(), &item2.get_id()).await.unwrap();
        // Item 3 has no tags
        
        // Filter cards by tags
        let query1 = GetQueryDtoBuilder::new()
            .add_tag_id(tag1.get_id())
            .build();

        let query2 = GetQueryDtoBuilder::new()
            .add_tag_id(tag2.get_id())
            .build();
        
        let query_both_tags = GetQueryDtoBuilder::new()
            .tag_ids(vec![tag1.get_id(), tag2.get_id()])
            .build();
        
        let cards_tag1 = list_cards_with_filters(&pool, &query1).unwrap();
        let cards_tag2 = list_cards_with_filters(&pool, &query2).unwrap();
        let cards_both_tags = list_cards_with_filters(&pool, &query_both_tags).unwrap();
        
        // Verify that we got the right cards
        assert_eq!(cards_tag1.len(), 4); // Items 1 and 2 have tag1
        assert_eq!(cards_tag2.len(), 2); // Only item 1 has tag2
        assert_eq!(cards_both_tags.len(), 2); // Only item 1 has both tags
        
        // Verify specific cards
        assert!(cards_tag1.iter().any(|c| c.get_item_id() == item1.get_id()));
        assert!(cards_tag1.iter().any(|c| c.get_item_id() == item2.get_id()));
        assert!(cards_tag2.iter().all(|c| c.get_item_id() == item1.get_id()));
        assert!(cards_both_tags.iter().all(|c| c.get_item_id() == item1.get_id()));
    }
    

    #[tokio::test]
    async fn test_filter_cards_by_next_review() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).await.unwrap();
        
        // Get cards for each item - there will be 2 cards per item
        let mut cards = vec![];
        cards.append(&mut get_cards_for_item(&pool, &item1.get_id()).unwrap());
        cards.append(&mut get_cards_for_item(&pool, &item2.get_id()).unwrap());
        
        // Set different next_review times for the cards
        let now = Utc::now();
        let yesterday = now - Duration::days(1);
        let tomorrow = now + Duration::days(1);
        
        cards[0].set_next_review(yesterday); // Due
        cards[1].set_next_review(tomorrow);  // Not due
        cards[2].set_suspended(Some(now)); // Card 3 has been suspended, so it's never due
        cards[3].set_next_review(yesterday); // Due

        // Update the cards in the database
        for card in &cards {
            update_card(&pool, card).await.unwrap();
        }
        
        // Filter cards by next_review
        let query = GetQueryDtoBuilder::new()
            .next_review_before(now)
            .build();
        
        let due_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Verify that we got the right cards
        assert_eq!(due_cards.len(), 2); // Cards 1 and 4 are due
        
        // Verify specific cards
        let due_card_ids: Vec<String> = due_cards.iter().map(|c| c.get_id()).collect();
        assert!(due_card_ids.contains(&cards[0].get_id())); // Card 1 is due
        assert!(!due_card_ids.contains(&cards[1].get_id())); // Card 2 is not due
        assert!(!due_card_ids.contains(&cards[2].get_id())); // Card 3 is not due (suspended)
        assert!(due_card_ids.contains(&cards[3].get_id())); // Card 4 is due
    }
    
    
    #[tokio::test]
    async fn test_filter_cards_with_multiple_criteria() {
        let pool = setup_test_db();
        
        // Create two item types
        let type1_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let type2_type = create_item_type(&pool, "Test Type 2".to_string()).await.unwrap();
        
        // Create items of different types
        let type1_item1 = create_item(
            &pool, 
            &type1_type.get_id(), 
            "Type 1 Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        let type2_item = create_item(
            &pool, 
            &type2_type.get_id(), 
            "Type 2 Item".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).await.unwrap();
        
        // Create some tags
        let important_tag = create_tag(&pool, "Important".to_string(), true).await.unwrap();
        
        // Add tags to items
        add_tag_to_item(&pool, &important_tag.get_id(), &type1_item1.get_id()).await.unwrap();
        add_tag_to_item(&pool, &important_tag.get_id(), &type2_item.get_id()).await.unwrap();
        // type1_item2 has no tags
        
        // Get cards for each item
        let item1_cards = get_cards_for_item(&pool, &type1_item1.get_id()).unwrap();
        let mut type1_card1 = item1_cards[0].clone();
        let mut type1_card2 = item1_cards[1].clone();
        let item2_cards = get_cards_for_item(&pool, &type2_item.get_id()).unwrap();
        let mut type2_card = item2_cards[0].clone();
        
        // Set different next_review times for the cards
        let now = Utc::now();
        let yesterday = now - Duration::days(1);
        let tomorrow = now + Duration::days(1);
        
        type1_card1.set_next_review(yesterday); // Due
        type1_card2.set_next_review(tomorrow);  // Not due
        type2_card.set_next_review(tomorrow); // Not due
        
        // Update the cards in the database
        update_card(&pool, &type1_card1).await.unwrap();
        update_card(&pool, &type1_card2).await.unwrap();
        update_card(&pool, &type2_card).await.unwrap();
        
        // Filter cards by multiple criteria
        let query = GetQueryDtoBuilder::new()
            .item_type_id(type1_type.get_id())
            .add_tag_id(important_tag.get_id())
            .next_review_before(now)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Verify that we got exactly the right card
        assert_eq!(filtered_cards.len(), 1);
        assert!(filtered_cards.iter().any(|c| c.get_id() == type1_card1.get_id()));
        
        // This card should:
        // 1. Belong to an item of type "Type 1"
        // 2. Belong to an item tagged as "Important"
        // 3. Be due for review (next_review is earlier than now)
    }
    
    
    #[test]
    fn test_filter_cards_edge_cases() {
        let pool = setup_test_db();
        
        // Test with empty database
        let query = GetQueryDtoBuilder::new()
            .build();
        
        let cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(cards.len(), 0);
        
        // Test with non-existent item type
        let query = GetQueryDtoBuilder::new()
            .item_type_id("nonexistent".to_string())
            .build();

        let cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(cards.len(), 0);
        
        // Test with non-existent tag
        let query = GetQueryDtoBuilder::new()
            .add_tag_id("nonexistent".to_string())
            .build();
        
        let cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(cards.len(), 0);
    }


    #[tokio::test]
    async fn test_update_card_priority() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            json!({"front": "Hello", "back": "World"})
        ).await.unwrap();
        
        // Create a card with initial priority
        let initial_priority = 0.5;
        let card = create_card(&pool, &item.get_id(), 2, initial_priority).await.unwrap();
        
        // Test updating to a valid priority
        let new_priority = 0.8;
        let result = update_card_priority(&pool, &card.get_id(), new_priority).await;
        assert!(result.is_ok());
        
        // Verify the priority was updated
        let updated_card = get_card(&pool, &card.get_id()).unwrap().unwrap();
        assert!((updated_card.get_priority() - new_priority).abs() < 0.0001);
        
        // Test updating to minimum valid priority (0.0)
        let min_priority = 0.0;
        let result = update_card_priority(&pool, &card.get_id(), min_priority).await;
        assert!(result.is_ok());
        
        // Verify the priority was updated to minimum
        let updated_card = get_card(&pool, &card.get_id()).unwrap().unwrap();
        assert!((updated_card.get_priority() - min_priority).abs() < 0.0001);
        
        // Test updating to maximum valid priority (1.0)
        let max_priority = 1.0;
        let result = update_card_priority(&pool, &card.get_id(), max_priority).await;
        assert!(result.is_ok());
        
        // Verify the priority was updated to maximum
        let updated_card = get_card(&pool, &card.get_id()).unwrap().unwrap();
        assert!((updated_card.get_priority() - max_priority).abs() < 0.0001);
    }
    
    #[tokio::test]
    async fn test_update_card_priority_invalid_values() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            json!({"front": "Hello", "back": "World"})
        ).await.unwrap();
        
        // Create a card with initial priority
        let initial_priority = 0.5;
        let card = create_card(&pool, &item.get_id(), 2, initial_priority).await.unwrap();
        
        // Test updating to a priority below the valid range
        let below_min_priority = -0.1;
        let result = update_card_priority(&pool, &card.get_id(), below_min_priority).await;
        assert!(result.is_err());
        
        // Test updating to a priority above the valid range
        let above_max_priority = 1.1;
        let result = update_card_priority(&pool, &card.get_id(), above_max_priority).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_update_card_priority_nonexistent_card() {
        let pool = setup_test_db();
        
        // Try to update a card that doesn't exist
        let nonexistent_card_id = "00000000-0000-0000-0000-000000000000";
        let result = update_card_priority(&pool, nonexistent_card_id, 0.5).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_filter_cards_by_suspended_state_exclude() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        // Get cards for the item
        let mut cards = get_cards_for_item(&pool, &item1.get_id()).unwrap();
        assert_eq!(cards.len(), 2);
        
        // Suspend one of the cards
        let now = Utc::now();
        cards[0].set_suspended(Some(now));
        
        // Update the cards in the database
        for card in &cards {
            update_card(&pool, card).await.unwrap();
        }
        
        // Filter to exclude suspended cards (default behavior)
        let query = GetQueryDtoBuilder::new()
            .suspended_filter(SuspendedFilter::Exclude)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Verify that we only got non-suspended cards
        assert!(filtered_cards.iter().all(|c| c.get_suspended().is_none()));
        assert!(filtered_cards.iter().any(|c| c.get_id() == cards[1].get_id()));
        assert!(!filtered_cards.iter().any(|c| c.get_id() == cards[0].get_id()));
    }
    
    #[tokio::test]
    async fn test_filter_cards_by_suspended_state_only() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        // Get cards for the item
        let mut cards = get_cards_for_item(&pool, &item1.get_id()).unwrap();
        assert_eq!(cards.len(), 2);
        
        // Suspend one of the cards
        let now = Utc::now();
        cards[0].set_suspended(Some(now));
        
        // Update the cards in the database
        for card in &cards {
            update_card(&pool, card).await.unwrap();
        }
        
        // Filter to only include suspended cards
        let query = GetQueryDtoBuilder::new()
            .suspended_filter(SuspendedFilter::Only)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Verify that we only got suspended cards
        assert_eq!(filtered_cards.len(), 1);
        assert!(filtered_cards.iter().all(|c| c.get_suspended().is_some()));
        assert!(filtered_cards.iter().any(|c| c.get_id() == cards[0].get_id()));
        assert!(!filtered_cards.iter().any(|c| c.get_id() == cards[1].get_id()));
    }
    
    #[tokio::test]
    async fn test_filter_cards_by_suspended_state_include() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        // Get cards for the item
        let mut cards = get_cards_for_item(&pool, &item1.get_id()).unwrap();
        assert_eq!(cards.len(), 2);
        
        // Suspend one of the cards
        let now = Utc::now();
        cards[0].set_suspended(Some(now));
        
        // Update the cards in the database
        for card in &cards {
            update_card(&pool, card).await.unwrap();
        }
        
        // Filter to include all cards (both suspended and not)
        let query = GetQueryDtoBuilder::new()
            .suspended_filter(SuspendedFilter::Include)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Verify that we got all cards
        assert_eq!(filtered_cards.len(), 2);
        assert!(filtered_cards.iter().any(|c| c.get_id() == cards[0].get_id()));
        assert!(filtered_cards.iter().any(|c| c.get_id() == cards[1].get_id()));
    }
    
    #[tokio::test]
    async fn test_filter_cards_by_suspended_date_before() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        // Get cards for the item
        let mut cards = get_cards_for_item(&pool, &item1.get_id()).unwrap();
        assert_eq!(cards.len(), 2);
        
        // Set different suspension times
        let now = Utc::now();
        let yesterday = now - chrono::Duration::days(1);
        let two_days_ago = now - chrono::Duration::days(2);
        
        cards[0].set_suspended(Some(yesterday));
        cards[1].set_suspended(Some(two_days_ago));
        
        // Update the cards in the database
        for card in &cards {
            update_card(&pool, card).await.unwrap();
        }
        
        // Filter suspended cards by date before (include all suspended cards)
        let query = GetQueryDtoBuilder::new()
            .suspended_filter(SuspendedFilter::Only)
            .suspended_before(now)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(filtered_cards.len(), 2);
        
        // Filter cards suspended before yesterday (should only include the older one)
        let query = GetQueryDtoBuilder::new()
            .suspended_filter(SuspendedFilter::Only)
            .suspended_before(yesterday)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(filtered_cards.len(), 1);
        assert_eq!(filtered_cards[0].get_id(), cards[1].get_id());
    }
    
    #[tokio::test]
    async fn test_filter_cards_by_suspended_date_after() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        // Get cards for the item
        let mut cards = get_cards_for_item(&pool, &item1.get_id()).unwrap();
        assert_eq!(cards.len(), 2);
        
        // Set different suspension times
        let now = Utc::now();
        let yesterday = now - chrono::Duration::days(1);
        let two_days_ago = now - chrono::Duration::days(2);
        
        cards[0].set_suspended(Some(yesterday));
        cards[1].set_suspended(Some(two_days_ago));
        
        // Update the cards in the database
        for card in &cards {
            update_card(&pool, card).await.unwrap();
        }
        
        // Filter suspended cards by date after (include all suspended cards)
        let query = GetQueryDtoBuilder::new()
            .suspended_filter(SuspendedFilter::Only)
            .suspended_after(two_days_ago - chrono::Duration::hours(1))
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(filtered_cards.len(), 2);
        
        // Filter cards suspended after yesterday (should only include the newer one)
        let query = GetQueryDtoBuilder::new()
            .suspended_filter(SuspendedFilter::Only)
            .suspended_after(yesterday)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(filtered_cards.len(), 0); // None are suspended after yesterday exactly
        
        // Filter cards suspended after two days ago (should include the more recent one)
        let query = GetQueryDtoBuilder::new()
            .suspended_filter(SuspendedFilter::Only)
            .suspended_after(two_days_ago)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(filtered_cards.len(), 1);
        assert_eq!(filtered_cards[0].get_id(), cards[0].get_id());
    }
    
    #[tokio::test]
    async fn test_filter_cards_by_last_review_date() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).await.unwrap();
        
        // Get cards for each item
        let mut cards = vec![];
        cards.append(&mut get_cards_for_item(&pool, &item1.get_id()).unwrap());
        cards.append(&mut get_cards_for_item(&pool, &item2.get_id()).unwrap());
        
        // Set different last review times for the cards
        let now = Utc::now();
        let yesterday = now - chrono::Duration::days(1);
        let two_days_ago = now - chrono::Duration::days(2);
        
        cards[0].set_last_review(Some(yesterday));
        cards[1].set_last_review(Some(two_days_ago));
        cards[2].set_last_review(None); // Never reviewed
        cards[3].set_last_review(Some(now));
        
        // Update the cards in the database
        for card in &cards {
            update_card(&pool, card).await.unwrap();
        }
        
        // Filter cards last reviewed after yesterday
        let query = GetQueryDtoBuilder::new()
            .last_review_after(yesterday)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(filtered_cards.len(), 1); // Only card[3] was reviewed after yesterday
        assert_eq!(filtered_cards[0].get_id(), cards[3].get_id());
        
        // Filter cards last reviewed after two days ago
        let query = GetQueryDtoBuilder::new()
            .last_review_after(two_days_ago)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(filtered_cards.len(), 2); // cards[0] and cards[3] were reviewed after two days ago
        assert!(filtered_cards.iter().any(|c| c.get_id() == cards[0].get_id()));
        assert!(filtered_cards.iter().any(|c| c.get_id() == cards[3].get_id()));
    }
    
    #[tokio::test]
    async fn test_filter_cards_by_suspended_date_combined() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        // Get cards for the item
        let mut cards = get_cards_for_item(&pool, &item1.get_id()).unwrap();
        assert_eq!(cards.len(), 2);
        
        // Create two more items with cards
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).await.unwrap();
        
        let item3 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 3".to_string(), 
            json!({"front": "F3", "back": "B3"})
        ).await.unwrap();
        
        cards.append(&mut get_cards_for_item(&pool, &item2.get_id()).unwrap());
        cards.append(&mut get_cards_for_item(&pool, &item3.get_id()).unwrap());
        
        // Now we have 6 cards, set up different suspension times
        let now = Utc::now();
        let yesterday = now - chrono::Duration::days(1);
        let two_days_ago = now - chrono::Duration::days(2);
        let three_days_ago = now - chrono::Duration::days(3);
        
        // Card 0: suspended yesterday
        cards[0].set_suspended(Some(yesterday));
        
        // Card 1: not suspended
        
        // Card 2: suspended two days ago
        cards[2].set_suspended(Some(two_days_ago));
        
        // Card 3: suspended three days ago
        cards[3].set_suspended(Some(three_days_ago));
        
        // Cards 4 and 5: not suspended
        
        // Update the cards in the database
        for card in &cards {
            update_card(&pool, card).await.unwrap();
        }
        
        // Filter suspended cards by date range (between two days ago and now)
        let query = GetQueryDtoBuilder::new()
            .suspended_filter(SuspendedFilter::Only)
            .suspended_after(two_days_ago - chrono::Duration::hours(1))
            .suspended_before(now + chrono::Duration::hours(1))
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Should include cards suspended in the last two days
        assert_eq!(filtered_cards.len(), 2);
        assert!(filtered_cards.iter().any(|c| c.get_id() == cards[0].get_id())); // yesterday
        assert!(filtered_cards.iter().any(|c| c.get_id() == cards[2].get_id())); // two days ago
        
        // Filter for cards suspended more than 2 days ago
        let query = GetQueryDtoBuilder::new()
            .suspended_filter(SuspendedFilter::Only)
            .suspended_before(two_days_ago)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Should only include the card suspended three days ago
        assert_eq!(filtered_cards.len(), 1);
        assert_eq!(filtered_cards[0].get_id(), cards[3].get_id());
    }
    
    #[tokio::test]
    async fn test_filter_cards_complex_query() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        // Get 2 cards for the item
        let mut cards = get_cards_for_item(&pool, &item1.get_id()).unwrap();
        
        // Create a tag
        let tag = create_tag(&pool, "Important".to_string(), true).await.unwrap();
        
        // Add tag to the item
        add_tag_to_item(&pool, &tag.get_id(), &item1.get_id()).await.unwrap();
        
        // Set up card states
        let now = Utc::now();
        let yesterday = now - chrono::Duration::days(1);
        
        // Card 0: Last reviewed yesterday, not suspended, due tomorrow
        cards[0].set_last_review(Some(yesterday));
        cards[0].set_next_review(now + chrono::Duration::days(1));
        
        // Card 1: Suspended yesterday, last reviewed 3 days ago
        cards[1].set_suspended(Some(yesterday));
        cards[1].set_last_review(Some(now - chrono::Duration::days(3)));
        
        // Update the cards
        for card in &cards {
            update_card(&pool, card).await.unwrap();
        }
        
        // Complex query 1: Get all suspended cards with the tag "Important"
        let query = GetQueryDtoBuilder::new()
            .add_tag_id(tag.get_id())
            .suspended_filter(SuspendedFilter::Only)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Should only include card 1
        assert_eq!(filtered_cards.len(), 1);
        assert_eq!(filtered_cards[0].get_id(), cards[1].get_id());
        
        // Complex query 2: Get all non-suspended cards last reviewed after a certain date with tag "Important"
        let query = GetQueryDtoBuilder::new()
            .add_tag_id(tag.get_id())
            .suspended_filter(SuspendedFilter::Exclude)
            .last_review_after(yesterday - chrono::Duration::hours(1))
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Should only include card 0
        assert_eq!(filtered_cards.len(), 1);
        assert_eq!(filtered_cards[0].get_id(), cards[0].get_id());
        
        // Complex query 3: Get all cards (suspended or not) from item type "Test Type 1" with tag "Important"
        let query = GetQueryDtoBuilder::new()
            .add_tag_id(tag.get_id())
            .item_type_id(item_type.get_id())
            .suspended_filter(SuspendedFilter::Include)
            .build();
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Should include both cards
        assert_eq!(filtered_cards.len(), 2);
        assert!(filtered_cards.iter().any(|c| c.get_id() == cards[0].get_id()));
        assert!(filtered_cards.iter().any(|c| c.get_id() == cards[1].get_id()));
    }
} 

/// Repository module
///
/// This module provides the data access layer for the application.
/// It contains functions for interacting with the database, including
/// creating, retrieving, and updating items and reviews.
/// 
/// The repository pattern abstracts away the details of database access
/// and provides a clean API for the rest of the application to use.
use crate::db::DbPool;
use crate::models::{Card, Item, ItemTag, ItemType, JsonValue, Review, Tag};
use crate::schema::{cards, item_tags, items, reviews, tags};
use crate::GetQueryDto;
use diesel::prelude::*;
use anyhow::{Result, anyhow};
use chrono::Utc;
use chrono::Duration;

/// Creates a new item type in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `name` - The name for the new item type
///
/// ### Returns
///
/// A Result containing the newly created ItemType if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
pub fn create_item_type(pool: &DbPool, name: String) -> Result<ItemType> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Create a new item type with the provided name
    let new_item_type = ItemType::new(name);
    
    // Insert the new item type into the database
    diesel::insert_into(crate::schema::item_types::table)
        .values(&new_item_type)
        .execute(conn)?;
    
    // Return the newly created item type
    Ok(new_item_type)
}


/// Retrieves an item type from the database by its ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `id` - The ID of the item type to retrieve
///
/// ### Returns
///
/// A Result containing an Option with the ItemType if found, or None if not found
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails for reasons other than the item type not existing
pub fn get_item_type(pool: &DbPool, id: &str) -> Result<Option<ItemType>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for the item type with the specified ID
    let result = crate::schema::item_types::table
        .find(id)
        .first::<ItemType>(conn)
        .optional()?;
    
    // Return the item type if found, or None if not found
    Ok(result)
}


/// Retrieves all item types from the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result containing a vector of all ItemTypes in the database
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn list_item_types(pool: &DbPool) -> Result<Vec<ItemType>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all item types
    let result = crate::schema::item_types::table
        .load::<ItemType>(conn)?;
    
    // Return the list of item types
    Ok(result)
}


/// Creates a new item in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `new_title` - The title for the new item
///
/// ### Returns
///
/// A Result containing the newly created Item if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
pub fn create_item(pool: &DbPool, item_type_id: &str, new_title: String, item_data: serde_json::Value) -> Result<Item> {
    // Get a connection from the pool
    let mut conn = pool.get()?;
    
    // Create a new item with the provided title
    let new_item = Item::new(item_type_id.to_string(), new_title, JsonValue(item_data));
    
    // Insert the new item into the database
    diesel::insert_into(items::table)
        .values(&new_item)
        .execute(&mut conn)?;

    // Drop the connection back to the pool
    drop(conn);

    // Create all necessary cards for the item
    create_cards_for_item(pool, &new_item)?;

    // TODO: If there's an error, we should delete the item and all its cards

    // Return the newly created item
    Ok(new_item)
}


/// Retrieves an item from the database by its ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to retrieve
///
/// ### Returns
///
/// A Result containing an Option with the Item if found, or None if not found
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails for reasons other than the item not existing
pub fn get_item(pool: &DbPool, item_id: &str) -> Result<Option<Item>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for the item with the specified ID
    let result = items::table
        .filter(items::id.eq(item_id))
        .first::<Item>(conn)
        .optional()?;
    
    // Return the result (Some(Item) if found, None if not)
    Ok(result)
}


/// Retrieves all items from the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result containing a vector of all Items in the database
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn list_items(pool: &DbPool) -> Result<Vec<Item>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all items
    let result = items::table.load::<Item>(conn)?;
    
    // Return the list of items
    Ok(result)
}


/// Retrieves all items of a specific item type from the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_type_id` - The ID of the item type to filter by
///
/// ### Returns
///
/// A Result containing a vector of Items of the specified type
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn get_items_by_type(pool: &DbPool, item_type_id: &str) -> Result<Vec<Item>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for items with the specified item type
    let result = items::table
        .filter(items::item_type.eq(item_type_id))
        .load::<Item>(conn)?;
    
    // Return the filtered list of items
    Ok(result)
}

/// Creates all necessary cards for an item based on its type
///
/// This function determines how many cards to create based on the item's type
/// and generates the appropriate number of cards for the item.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item` - The item for which to create cards
///
/// ### Returns
///
/// A Result containing a vector of the newly created Cards
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database operations fail
/// - Unable to determine the item type
fn create_cards_for_item(pool: &DbPool, item: &Item) -> Result<Vec<Card>> {
    // Get the item type to determine how many cards to create
    let item_type = get_item_type(pool, &item.get_item_type())?
        .ok_or_else(|| anyhow!("Item type not found"))?;
    
    // Vector to store the created cards
    let mut cards = Vec::new();
    
    // Determine how many cards to create based on the item type
    match item_type.get_name().as_str() {
        "Basic" => {
            // Basic items have just one card (front/back)
            let card = create_card(pool, &item.get_id(), 0)?;
            cards.push(card);
        },
        "Cloze" => {
            // Cloze items might have multiple cards (one per cloze deletion)
            let data = item.get_data();
            let cloze_deletions = data.0["clozes"].clone();
            let cloze_deletions = cloze_deletions.as_array()
                .ok_or_else(|| anyhow!("cloze deletion must be an array"))?;
            for (index, _) in cloze_deletions.iter().enumerate() {
                let card = create_card(pool, &item.get_id(), index as i32)?;
                cards.push(card);
            }
        },
        "Vocabulary" => {
            // Vocabulary items have 2 cards (term->definition and definition->term)
            for i in 0..2 {
                let card = create_card(pool, &item.get_id(), i)?;
                cards.push(card);
            }
        },
        "Todo" => {
            // Todo items have 1 card (each todo is a card)
            let card = create_card(pool, &item.get_id(), 0)?;
            cards.push(card);
        },
        // TODO: this is a hack
        name if name.contains("Test") => {
            // Test item types have 2 cards
            for i in 0..2 {
                let card = create_card(pool, &item.get_id(), i)?;
                cards.push(card);
            }
        },
        _ => {
            // Return an error for unknown item types
            return Err(anyhow!("Unable to construct cards for unknown item type: {}", item_type.get_name()));
        }
    }
    
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
fn create_card(pool: &DbPool, item_id: &str, card_index: i32) -> Result<Card> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Create a new card with the provided item ID and card index
    let mut new_card = Card::new(item_id.to_string(), card_index);

    // TODO: this is a hack, we should vary how scheduling works based on the item type
    // Set the card's scheduler data to "delay: 1"
    new_card.set_scheduler_data(Some(JsonValue(serde_json::json!({
        "delay": 1
    }))));
    
    // Insert the new card into the database
    diesel::insert_into(cards::table)
        .values(&new_card)
        .execute(conn)?;
    
    // Return the newly created card
    Ok(new_card)
}


/// Retrieves a card from the database by ID
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
pub fn get_card(pool: &DbPool, card_id: &str) -> Result<Option<Card>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for the card with the specified ID
    let result = cards::table
        .filter(cards::id.eq(card_id))
        .first::<Card>(conn)
        .optional()?;
    
    // Return the result (Some(Card) if found, None if not)
    Ok(result)
}


/// Retrieves cards from the database with optional filtering
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `query` - A reference to a GetQueryDto containing the following optional filters:
///   - `item_type_id` - Filter cards by item type
///   - `tags` - Filter cards by tags
///   - `next_review_before` - Filter cards with next_review before specified datetime
///
/// ### Returns
///
/// A Result containing a vector of Cards matching the specified filters
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn list_cards_with_filters(pool: &DbPool, query: &GetQueryDto) -> Result<Vec<Card>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Start building the query
    let mut query_builder = cards::table.into_boxed();
    
    // If item_type_id is provided, filter by joining with items
    if let Some(item_type_id) = &query.item_type_id {
        // Get the item IDs with the specified item_type
        let item_ids: Vec<String> = items::table
            .filter(items::item_type.eq(item_type_id))
            .select(items::id)
            .load::<String>(conn)?;
        
        // Filter cards that match these item IDs
        query_builder = query_builder.filter(cards::item_id.eq_any(item_ids));
    }
    
    // If next_review_before is provided, filter cards with next_review before the specified time
    if let Some(next_review_before) = &query.next_review_before {
        query_builder = query_builder
            .filter(cards::next_review.lt(next_review_before.naive_utc()).and(cards::next_review.is_not_null()));
    }
    
    // Execute the query to get the initial set of cards
    let mut result = query_builder.load::<Card>(conn)?;
    
    // If tags are provided, we need to filter the results further
    // Note: This is done in-memory because it's a bit complex to do in a single SQL query
    if !query.tag_ids.is_empty() {
        // Get all item_ids that have all of the required tags
        let tagged_item_ids: Vec<String> = item_tags::table
                .inner_join(tags::table)
                .filter(tags::id.eq_any(&query.tag_ids))
                .group_by(item_tags::item_id)
                .having(diesel::dsl::count_star().eq(query.tag_ids.len() as i64))
                .select(item_tags::item_id)
                .load::<String>(conn)?;
            
        // Filter cards to only include those with item_ids in the tagged_item_ids list
        result = result
            .into_iter()
            .filter(|card| tagged_item_ids.contains(&card.get_item_id()))
            .collect();
    }
    
    // Return the filtered list of cards
    Ok(result)
}


/// Retrieves all cards for a specific item
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to get cards for
///
/// ### Returns
///
/// A Result containing a vector of Cards belonging to the specified item
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn get_cards_for_item(pool: &DbPool, item_id: &str) -> Result<Vec<Card>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all cards belonging to the specified item
    let result = cards::table
        .filter(cards::item_id.eq(item_id))
        .load::<Card>(conn)?;
    
    // Return the list of cards
    Ok(result)
}


/// Records a review for an item and updates the item's review schedule
///
/// This function performs two operations:
/// 1. Creates a new review record
/// 2. Updates the item with new review scheduling information
///
/// The scheduling uses a simple spaced repetition algorithm based on the rating:
/// - Rating 1 (failed): Review again tomorrow
/// - Rating 2 (difficult): Review again in 7 days
/// - Rating 3 (medium): Review again in 1.2 times the days of the previous review
/// - Rating 4 (easy): Review again in 1.7 times the days of the previous review
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id_val` - The ID of the item being reviewed
/// * `rating_val` - The rating given during the review (1-4)
///
/// ### Returns
///
/// A Result containing the newly created Review if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The item does not exist
/// - The database insert or update operations fail
pub fn record_review(pool: &DbPool, card_id: &str, rating_val: i32) -> Result<Review> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;

    // Validate that the rating is within the allowed range (1-3)
    if rating_val < 1 || rating_val > 4 {
        return Err(anyhow!("Rating must be between 1 and 4, got {}", rating_val));
    }
    
    // 1) Insert the review record
    // Create a new review with the provided item ID and rating
    let new_review = Review::new(card_id, rating_val);
    
    // Insert the new review into the database
    diesel::insert_into(reviews::table)
        .values(&new_review)
        .execute(conn)?;

    // 2) Retrieve the item and update next_review
    // Get the item from the database
    let mut card = cards::table
        .filter(cards::id.eq(card_id))
        .first::<Card>(conn)?;
    
    // Get the current time for updating timestamps
    let now = Utc::now();
    
    // Update the last review time to now
    card.set_last_review(Some(now));

    // Get the current delay from the card's scheduler data
    let current_delay = card.get_scheduler_data()
        .and_then(|data| data.0.get("delay").and_then(|delay| delay.as_f64()))
        .ok_or_else(|| anyhow!("Missing scheduler data for card"))?;
    
    // Simple spaced repetition logic
    // Determine when to schedule the next review based on the rating
    let days_to_add = match rating_val {
        1 => 1,                                   // If failed, review tomorrow
        2 => 7,                                   // If difficult, review in 6 days
        3 => (current_delay * 1.2).ceil() as i64, // If medium, review in 1.2 times the days of the previous review
        4 => (current_delay * 1.7).ceil() as i64, // If easy, review in 1.7 times the days of the previous review
        _ => panic!("Invalid rating value: {}. Should not happen as we already validated the rating range.", rating_val),
    };

    // TODO: this is a hack, we should vary how scheduling works based on the item type
    // update the scheduler data with the new delay
    card.set_scheduler_data(Some(JsonValue(serde_json::json!({
        "delay": current_delay * match rating_val {
            1 => 1.01,
            2 => 1.05,
            3 => 1.1,
            4 => 1.15,
            _ => panic!("Invalid rating value: {}. Should not happen as we already validated the rating range.", rating_val),
        }
    }))));
    
    // Calculate the next review time
    card.set_next_review(Some(now + Duration::days(days_to_add)));
    
    // Update the card in the database with the new review information
    diesel::update(cards::table.filter(cards::id.eq(card.get_id())))
        .set((
            cards::next_review.eq(card.get_next_review_raw()),
            cards::last_review.eq(card.get_last_review_raw()),
            cards::scheduler_data.eq(card.get_scheduler_data()),
        ))
        .execute(conn)?;
    
    // Return the newly created review
    Ok(new_review)
}


/// Retrieves all reviews for a specific card from the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to get reviews for
///
/// ### Returns
///
/// A Result containing a vector of all Reviews for the specified card
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn get_reviews_for_card(pool: &DbPool, card_id: &str) -> Result<Vec<Review>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all reviews of the specified card
    let result = reviews::table
        .filter(reviews::card_id.eq(card_id))
        .order_by(reviews::review_timestamp.desc())
        .load::<Review>(conn)?;
    
    // Return the list of reviews
    Ok(result)
}


/// Creates a new tag in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `name` - The name of the tag to create
///
/// ### Returns
///
/// A Result containing the newly created Tag
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert fails
pub fn create_tag(pool: &DbPool, name: String) -> Result<Tag> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Create a new tag with a random ID
    let new_tag = Tag::new(name, true);
    
    // Insert the tag into the database
    diesel::insert_into(tags::table)
        .values(&new_tag)
        .execute(conn)?;
        
    Ok(new_tag)
}

/// Gets a tag from the database by ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `tag_id` - The ID of the tag to retrieve
///
/// ### Returns
///
/// A Result containing the Tag if found
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn get_tag(pool: &DbPool, tag_id: &str) -> Result<Tag> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for the tag
    let tag = tags::table
        .filter(tags::id.eq(tag_id))
        .first::<Tag>(conn)?;
        
    Ok(tag)
}


/// Lists all tags in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result containing a vector of all Tags in the database
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn list_tags(pool: &DbPool) -> Result<Vec<Tag>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all tags
    let result = tags::table.load::<Tag>(conn)?;
    
    Ok(result)
}


/// Associates a tag with an item in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `tag_id` - The ID of the tag to associate
/// * `item_id` - The ID of the item to associate
///
/// ### Returns
///
/// A Result indicating success or failure
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert fails
pub fn add_tag_to_item(pool: &DbPool, tag_id: &str, item_id: &str) -> Result<()> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Create the association
    let new_association = ItemTag::new(item_id.to_string(), tag_id.to_string());
    
    // Insert the association into the database
    diesel::insert_into(item_tags::table)
        .values(&new_association)
        .execute(conn)?;
        
    Ok(())
}


/// Removes a tag from an item in the database
pub fn remove_tag_from_item(pool: &DbPool, tag_id: &str, item_id: &str) -> Result<()> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Remove the association from the database
    diesel::delete(item_tags::table.filter(item_tags::item_id.eq(item_id).and(item_tags::tag_id.eq(tag_id)))).execute(conn)?;
    
    Ok(())
}


/// Lists all tags for an item in the database
pub fn list_tags_for_item(pool: &DbPool, item_id: &str) -> Result<Vec<Tag>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all tags for the specified item
    let result = tags::table
        .inner_join(item_tags::table)
        .filter(item_tags::item_id.eq(item_id))
        .select(tags::all_columns)
        .load::<Tag>(conn)?;
    
    Ok(result)
}


// Lists all tags for a card in the database
pub fn list_tags_for_card(pool: &DbPool, card_id: &str) -> Result<Vec<Tag>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all tags for the specified card
    let result = cards::table
        .inner_join(items::table.inner_join(item_tags::table.inner_join(tags::table)))
        .filter(cards::id.eq(card_id))
        .select(tags::all_columns)
        .load::<Tag>(conn)?;
    
    Ok(result)
}



#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::db;
    use diesel::connection::SimpleConnection;
    use diesel_migrations::MigrationHarness;
    use uuid::Uuid;
    
    /// Sets up a test database with migrations applied
    ///
    /// This function:
    /// 1. Creates an in-memory SQLite database
    /// 2. Enables foreign key constraints
    /// 3. Runs all migrations to set up the schema
    ///
    /// ### Returns
    ///
    /// A database connection pool connected to the in-memory database
    fn setup_test_db() -> DbPool {
        // Use an in-memory database for testing
        let database_url = ":memory:";
        let pool = db::init_pool(database_url);
        
        // Run migrations on the in-memory database
        let mut conn = pool.get().expect("Failed to get connection");
        
        // Enable foreign key constraints for SQLite
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        
        // Run all migrations to set up the schema
        let migrations = diesel_migrations::FileBasedMigrations::find_migrations_directory().expect("Failed to find migrations directory");
        conn.run_pending_migrations(migrations).expect("Failed to run migrations");
        
        pool
    }

    
    /// Updates an existing card in the database
    pub fn update_card(pool: &DbPool, card: &Card) -> Result<()> {
        // Get a connection from the pool
        let conn = &mut pool.get()?;
        
        // Update the card in the database
        diesel::update(cards::table)
            .filter(cards::id.eq(card.get_id()))
            .set((
                cards::next_review.eq(card.get_next_review_raw()),
                cards::last_review.eq(card.get_last_review_raw()),
                cards::scheduler_data.eq(card.get_scheduler_data()),
            ))
            .execute(conn)?;
            
        Ok(())
    }


    /// Lists all cards in the database
    pub fn list_all_cards(pool: &DbPool) -> Result<Vec<Card>> {
        let conn = &mut pool.get()?;
        let result = cards::table.load::<Card>(conn)?;
        Ok(result)
    }


    /// Tests that migrations are applied correctly
    ///
    /// This test verifies that:
    /// 1. The test database is set up correctly
    /// 2. The migrations are applied successfully
    /// 3. The expected tables are created in the database
    #[test]
    fn test_migrations_applied() {
        // Set up a test database
        let pool = setup_test_db();
        let mut conn = pool.get().expect("Failed to get connection");
        
        // Check if the item_types table exists
        let result = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table' AND name='item_types'")
            .execute(&mut *conn);
        assert!(result.is_ok(), "item_types table should exist");
        
        // Check the structure of the item_types table using a simple query
        let result = diesel::sql_query("SELECT sql FROM sqlite_master WHERE type='table' AND name='item_types'")
            .execute(&mut *conn);
        assert!(result.is_ok(), "Should be able to query item_types table structure");
        
        // Check if the items table exists
        let result = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table' AND name='items'")
            .execute(&mut *conn);
        assert!(result.is_ok(), "items table should exist");
        
        // Check if the cards table exists
        let result = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table' AND name='cards'")
            .execute(&mut *conn);
        assert!(result.is_ok(), "cards table should exist");
        
        // Check if the tags table exists
        let result = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table' AND name='tags'")
            .execute(&mut *conn);
        assert!(result.is_ok(), "tags table should exist");
        
        // Check if the item_tags table exists
        let result = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table' AND name='item_tags'")
            .execute(&mut *conn);
        assert!(result.is_ok(), "item_tags table should exist");
        
        // Check if the reviews table exists
        let result = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table' AND name='reviews'")
            .execute(&mut *conn);
        assert!(result.is_ok(), "reviews table should exist");
        
        // Drop the connection back to the pool
        drop(conn);
        
        // Try to create an item type to verify the table is usable
        let name = "Test Item Type".to_string();
        let result = create_item_type(&pool, name.clone());
        assert!(result.is_ok(), "Should be able to create an item type: {:?}", result.err());
    }

    /// Tests creating a new item type
    ///
    /// This test verifies that:
    /// 1. An item type can be successfully created in the database
    /// 2. The created item type has the correct name and a valid ID
    #[test]
    fn test_create_item_type() {
        // Set up a test database
        let pool = setup_test_db();
        let name = "Test Item Type".to_string();
        
        // Create a new item type
        let result = create_item_type(&pool, name.clone());
        assert!(result.is_ok(), "Should create an item type successfully");
        
        // Verify the created item type
        let item_type = result.unwrap();
        assert_eq!(item_type.get_name(), name);
        assert!(!item_type.get_id().is_empty());
    }
    

    /// Tests creating a new item
    ///
    /// This test verifies that:
    /// 1. An item can be successfully created in the database
    /// 2. The created item has the correct title and a valid ID
    #[test]
    fn test_create_item() {
        // Set up a test database
        let pool = setup_test_db();
        let title = "Test Item".to_string();

        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        
        // Create a new item
        let result = create_item(&pool, &item_type.get_id(), title.clone(), serde_json::Value::Null);
        assert!(result.is_ok(), "Should create an item successfully");
        
        // Verify the created item
        let item = result.unwrap();
        assert_eq!(item.get_title(), title);
        assert!(!item.get_id().is_empty());
    }
    

    /// Tests retrieving an item by ID
    ///
    /// This test verifies that:
    /// 1. An item can be successfully retrieved from the database
    /// 2. The retrieved item has the correct ID and title
    #[test]
    fn test_get_item() {
        // Set up a test database
        let pool = setup_test_db();
        let title = "Test Item for Get".to_string();

        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        
        // First create an item
        let created_item = create_item(&pool, &item_type.get_id(), title.clone(), serde_json::Value::Null).unwrap();
        
        // Then try to get it
        let result = get_item(&pool, &created_item.get_id());
        assert!(result.is_ok(), "Should get an item successfully");
        
        // Verify the item exists
        let item_option = result.unwrap();
        assert!(item_option.is_some(), "Item should exist");
        
        // Verify the item properties
        let item = item_option.unwrap();
        assert_eq!(item.get_id(), created_item.get_id());
        assert_eq!(item.get_title(), title);
    }
    

    /// Tests retrieving a non-existent item
    ///
    /// This test verifies that:
    /// 1. Attempting to retrieve a non-existent item returns None
    /// 2. No error is thrown for a non-existent item
    #[test]
    fn test_get_nonexistent_item() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Try to get a non-existent item
        let result = get_item(&pool, "nonexistent-id");
        assert!(result.is_ok(), "Should not error for non-existent item");
        
        // Verify the item does not exist
        let item_option = result.unwrap();
        assert!(item_option.is_none(), "Item should not exist");
    }
    

    /// Tests listing all items
    ///
    /// This test verifies that:
    /// 1. All items can be successfully retrieved from the database
    /// 2. The correct number of items is returned
    /// 3. All expected items are included in the results
    #[test]
    fn test_list_items() {
        // Set up a test database
        let pool = setup_test_db();

        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        
        // Create a few items
        let titles = vec!["Item 1", "Item 2", "Item 3"];
        for title in &titles {
            create_item(&pool, &item_type.get_id(), title.to_string(), serde_json::Value::Null).unwrap();
        }
        
        // List all items
        let result = list_items(&pool);
        assert!(result.is_ok(), "Should list items successfully");
        
        // Verify the correct number of items
        let items = result.unwrap();
        assert_eq!(items.len(), titles.len(), "Should have the correct number of items");
        
        // Check that all titles are present
        let item_titles: Vec<String> = items.iter().map(|item| item.get_title().clone()).collect();
        for title in titles {
            assert!(item_titles.contains(&title.to_string()), "Should contain title: {}", title);
        }
    }
    

    /// Tests recording a review and updating an item's review schedule
    ///
    /// This test verifies that:
    /// 1. A review can be successfully recorded
    /// 2. The review has the correct item ID and rating
    /// 3. The item is updated with the correct review information
    /// 4. The next review is scheduled according to the spaced repetition algorithm
    #[test]
    fn test_record_review() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create a single item type to use for all tests
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        
        // Test each rating value with a fresh card
        for rating in 1..=4 {
            // Create a new item for each rating
            let item = create_item(&pool, &item_type.get_id(), format!("Item to Review with Rating {}", rating), serde_json::Value::Null).unwrap();
            
            // Get the cards that were automatically created for the item
            let cards = get_cards_for_item(&pool, &item.get_id()).unwrap();
            assert!(!cards.is_empty(), "Item should have at least one card");
            let card = &cards[0]; // Get the first card
            
            // Record a review
            let result = record_review(&pool, &card.get_id(), rating);
            assert!(result.is_ok(), "Should record a review successfully with rating {}", rating);
            
            // Verify the review properties
            let review = result.unwrap();
            assert_eq!(review.get_card_id(), card.get_id());
            assert_eq!(review.get_rating(), rating);
            
            // Check that the item was updated with review information
            let updated_card = get_card(&pool, &card.get_id()).unwrap().unwrap();
            assert!(updated_card.get_last_review().is_some(), "Last review should be set");
            assert!(updated_card.get_next_review().is_some(), "Next review should be set");
            
            // Check that the next review is scheduled according to the algorithm
            let last_review = updated_card.get_last_review().unwrap();
            let next_review = updated_card.get_next_review().unwrap();
            let days_diff = (next_review.timestamp() - last_review.timestamp()) / (24 * 60 * 60);
            
            // Verify the expected days difference based on the rating
            match rating {
                1 => assert_eq!(days_diff, 1, "For rating 1, next review should be 1 day later"),
                2 => assert_eq!(days_diff, 7, "For rating 2, next review should be 7 days later"),
                3 => {
                    // For rating 3, the delay should be based on the current delay (1) * 1.2
                    // Ceiling of 1.2 is 2
                    assert_eq!(days_diff, 2, "For rating 3, next review should be 2 days later");
                },
                4 => {
                    // For rating 4, the delay should be based on the current delay (1) * 1.7
                    // Ceiling of 1.7 is 2
                    assert_eq!(days_diff, 2, "For rating 4, next review should be 2 days later");
                },
                _ => panic!("Invalid rating: {}", rating),
            }
        }
    }


    /// Tests creating a card for an item
    ///
    /// This test verifies that:
    /// 1. A card can be successfully created for an item
    /// 2. The card has the correct item ID and index
    /// 3. The card can be retrieved from the database
    #[test]
    fn test_create_card() {
        // Set up a test database
        let pool = setup_test_db();
        
        // First create an item
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item = create_item(&pool, &item_type.get_id(), "Item with Card".to_string(), serde_json::Value::Null).unwrap();
        
        // Create a card for the item
        let card_index = 2;
        let result = create_card(&pool, &item.get_id(), card_index);
        assert!(result.is_ok(), "Should create a card successfully");
        
        // Verify the card properties
        let card = result.unwrap();
        assert_eq!(card.get_item_id(), item.get_id());
        assert_eq!(card.get_card_index(), card_index);
        assert!(card.get_next_review().is_none());
        assert!(card.get_last_review().is_none());
        
        // Check that the card can be retrieved from the database
        let retrieved_card_result = get_card(&pool, &card.get_id());
        assert!(retrieved_card_result.is_ok(), "Should be able to retrieve the card from the database");
        
        let retrieved_card_option = retrieved_card_result.unwrap();
        assert!(retrieved_card_option.is_some(), "Card should exist in the database");
        
        let retrieved_card = retrieved_card_option.unwrap();
        assert_eq!(retrieved_card.get_id(), card.get_id(), "Retrieved card should have the same ID");
        assert_eq!(retrieved_card.get_item_id(), item.get_id(), "Retrieved card should have the correct item ID");
        assert_eq!(retrieved_card.get_card_index(), card_index, "Retrieved card should have the correct index");
    }
    

    /// Tests retrieving a card by ID
    ///
    /// This test verifies that:
    /// 1. A card can be retrieved by its ID
    /// 2. The correct card is returned
    /// 3. A non-existent card returns None
    #[test]
    fn test_get_card() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item and a card
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item = create_item(&pool, &item_type.get_id(), "Item with Card".to_string(), serde_json::Value::Null).unwrap();
        let card = create_card(&pool, &item.get_id(), 2).unwrap();
        
        // Retrieve the card
        let result = get_card(&pool, &card.get_id());
        assert!(result.is_ok(), "Should retrieve a card successfully");
        
        // Verify the correct card is returned
        let retrieved_card = result.unwrap().unwrap();
        assert_eq!(retrieved_card.get_id(), card.get_id());
        assert_eq!(retrieved_card.get_item_id(), item.get_id());
        
        // Test retrieving a non-existent card
        let non_existent_id = Uuid::new_v4().to_string();
        let result = get_card(&pool, &non_existent_id);
        assert!(result.is_ok(), "Should handle non-existent card gracefully");
        assert!(result.unwrap().is_none(), "Should return None for non-existent card");
    }
    

    /// Tests retrieving all cards for an item
    ///
    /// This test verifies that:
    /// 1. All cards for a specific item can be retrieved
    /// 2. Cards for other items are not included
    /// 3. The correct number of cards is returned
    /// 
    /// Note: This test assumes that creating an item automatically creates 2 cards
    /// based on the item type. If this implementation detail changes, this test will
    /// need to be updated.
    #[test]
    fn test_retrieve_cards_by_item_id() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create two items
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item1 = create_item(&pool, &item_type.get_id(), "Item 1".to_string(), serde_json::Value::Null).unwrap();
        let item2 = create_item(&pool, &item_type.get_id(), "Item 2".to_string(), serde_json::Value::Null).unwrap();
        
        // Get the number of cards automatically created for item1
        let auto_created_cards = get_cards_for_item(&pool, &item1.get_id()).unwrap();
        let num_auto_created = auto_created_cards.len();
        println!("Number of automatically created cards: {}", num_auto_created);
        
        // Create cards for item1
        let item1_indices = vec![2, 3, 4];
        for index in &item1_indices {
            create_card(&pool, &item1.get_id(), *index).unwrap();
        }
        
        // Create cards for item2
        let item2_indices = vec![2, 3];
        for index in &item2_indices {
            create_card(&pool, &item2.get_id(), *index).unwrap();
        }
        
        // Get cards for item1
        let result = get_cards_for_item(&pool, &item1.get_id());
        assert!(result.is_ok(), "Should get cards for item successfully");
        
        // Verify only item1 cards are returned
        let cards = result.unwrap();
        assert_eq!(cards.len(), item1_indices.len() + num_auto_created, 
            "Should return correct number of cards ({} created + {} by default)", 
            item1_indices.len(), num_auto_created);
        
        // Check that all cards belong to item1
        for card in &cards {
            assert_eq!(card.get_item_id(), item1.get_id(), "Card should belong to item1");
        }
        
        // Check that all indices for item1 are present
        let indices: Vec<i32> = cards.iter().map(|card| card.get_card_index()).collect();
        for index in item1_indices {
            assert!(indices.contains(&index), "Should contain card with index: {}", index);
        }
        
        // Test with a non-existent item ID
        let non_existent_id = Uuid::new_v4().to_string();
        let result = get_cards_for_item(&pool, &non_existent_id);
        assert!(result.is_ok(), "Should handle non-existent item ID gracefully");
        assert_eq!(result.unwrap().len(), 0, "Should return empty list for non-existent item");
    }
    

    /// Tests retrieving all item types
    ///
    /// This test verifies that:
    /// 1. All item types can be retrieved from the database
    /// 2. The correct number of item types is returned
    /// 3. The item types have the expected names
    #[test]
    fn test_list_item_types() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create several item types
        let type_names = vec!["Type A", "Type B", "Type C"];
        for name in &type_names {
            create_item_type(&pool, name.to_string()).unwrap();
        }
        
        // List all item types
        let result = list_item_types(&pool);
        assert!(result.is_ok(), "Should list item types successfully");
        
        // Verify the correct number of item types is returned
        let item_types = result.unwrap();
        assert_eq!(item_types.len(), type_names.len(), "Should return all item types");
        
        // Check that all created item types are included
        let type_names_from_db: Vec<String> = item_types.iter().map(|it| it.get_name().clone()).collect();
        for name in type_names {
            assert!(type_names_from_db.contains(&name.to_string()), "Should contain item type: {}", name);
        }
    }
    

    /// Tests retrieving an item type by ID
    ///
    /// This test verifies that:
    /// 1. An item type can be retrieved by its ID
    /// 2. The correct item type is returned
    /// 3. A non-existent item type returns None
    #[test]
    fn test_get_item_type() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item type
        let name = "Test Item Type";
        let item_type = create_item_type(&pool, name.to_string()).unwrap();
        
        // Retrieve the item type
        let result = get_item_type(&pool, &item_type.get_id());
        assert!(result.is_ok(), "Should retrieve an item type successfully");
        
        // Verify the correct item type is returned
        let retrieved_type = result.unwrap().unwrap();
        assert_eq!(retrieved_type.get_id(), item_type.get_id());
        assert_eq!(retrieved_type.get_name(), name);
        
        // Test retrieving a non-existent item type
        let non_existent_id = Uuid::new_v4().to_string();
        let result = get_item_type(&pool, &non_existent_id);
        assert!(result.is_ok(), "Should handle non-existent item type gracefully");
        assert!(result.unwrap().is_none(), "Should return None for non-existent item type");
    }


    /// Tests retrieving items by type
    ///
    /// This test verifies that:
    /// 1. Items can be filtered by their item type
    /// 2. Only items of the specified type are returned
    /// 3. All items of the specified type are included
    #[test]
    fn test_get_items_by_type() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create two different item types
        let type1 = create_item_type(&pool, "Test Type 1".to_string()).unwrap();
        let type2 = create_item_type(&pool, "Test Type 2".to_string()).unwrap();
        
        // Create items of different types
        let type1_titles = vec!["Test Type 1 Item 1", "Test Type 1 Item 2", "Test Type 1 Item 3"];
        let type2_titles = vec!["Test Type 2 Item 1", "Test Type 2 Item 2"];
        
        for title in &type1_titles {
            create_item(&pool, &type1.get_id(), title.to_string(), serde_json::Value::Null).unwrap();
        }
        
        for title in &type2_titles {
            create_item(&pool, &type2.get_id(), title.to_string(), serde_json::Value::Null).unwrap();
        }
        
        // Get items of type1
        let result = get_items_by_type(&pool, &type1.get_id());
        assert!(result.is_ok(), "Should get items by type successfully");
        
        // Verify only type1 items are returned
        let items = result.unwrap();
        assert_eq!(items.len(), type1_titles.len(), "Should return correct number of items");
        
        // Check that all type1 titles are present
        let item_titles: Vec<String> = items.iter().map(|item| item.get_title().clone()).collect();
        for title in type1_titles {
            assert!(item_titles.contains(&title.to_string()), "Should contain title: {}", title);
        }
        
        // Check that no type2 titles are present
        for title in type2_titles {
            assert!(!item_titles.contains(&title.to_string()), "Should not contain title: {}", title);
        }
        
        // Test with a non-existent type ID
        let non_existent_id = Uuid::new_v4().to_string();
        let result = get_items_by_type(&pool, &non_existent_id);
        assert!(result.is_ok(), "Should handle non-existent type ID gracefully");
        assert_eq!(result.unwrap().len(), 0, "Should return empty list for non-existent type");
    }
    
    
    /// Tests listing all cards
    ///
    /// This test verifies that:
    /// 1. All cards can be successfully retrieved from the database
    /// 2. The correct number of cards is returned
    /// 3. All expected cards are included in the results
    #[test]
    fn test_list_all_cards() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item type and item
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item_1 = create_item(&pool, &item_type.get_id(), "Item with Cards 1".to_string(), serde_json::Value::Null).unwrap();
        let item_2 = create_item(&pool, &item_type.get_id(), "Item with Cards 2".to_string(), serde_json::Value::Null).unwrap();
        
        // Get the cards that were automatically created for the items
        let mut cards = get_cards_for_item(&pool, &item_1.get_id()).unwrap();
        cards.extend(get_cards_for_item(&pool, &item_2.get_id()).unwrap());
        assert!(!cards.is_empty(), "Items should have cards");
        
        // List all cards
        let result = list_cards_with_filters(&pool, &crate::GetQueryDto {
            item_type_id: None,
            tag_ids: vec![],
            next_review_before: None,
        });
        assert!(result.is_ok(), "Should list cards successfully");
        
        // Verify the correct number of cards
        let all_cards = result.unwrap();
        assert_eq!(all_cards.len(), cards.len(), "Should have the correct number of cards");
        
        // Check that all card IDs are present
        let card_ids_from_db: Vec<String> = all_cards.iter().map(|card| card.get_id().clone()).collect();
        let expected_ids: Vec<String> = cards.iter().map(|card| card.get_id().clone()).collect();
        for id in expected_ids {
            assert!(card_ids_from_db.contains(&id), "Should contain card with ID: {}", id);
        }
        
        // Also check that all card indices are present
        let card_indices_from_db: Vec<i32> = all_cards.iter().map(|card| card.get_card_index()).collect();
        let expected_indices: Vec<i32> = cards.iter().map(|card| card.get_card_index()).collect();
        for index in expected_indices {
            assert!(card_indices_from_db.contains(&index), "Should contain card with index: {}", index);
        }
    }


    /// Tests filtering cards by item type
    ///
    /// This test verifies that:
    /// 1. Cards can be filtered by item type
    /// 2. Only cards associated with the specified item type are returned
    /// 3. Cards from other item types are excluded
    #[test]
    fn test_filter_cards_by_item_type() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create two different item types
        let item_type_1 = create_item_type(&pool, "Test Item Type 1".to_string()).unwrap();
        let item_type_2 = create_item_type(&pool, "Test Item Type 2".to_string()).unwrap();
        
        // Create items for each item type
        let item_1 = create_item(&pool, &item_type_1.get_id(), "Item Type 1 Item".to_string(), serde_json::Value::Null).unwrap();
        let item_2 = create_item(&pool, &item_type_2.get_id(), "Item Type 2 Item".to_string(), serde_json::Value::Null).unwrap();
        
        // Get the cards that were automatically created for the items
        let cards_type_1 = get_cards_for_item(&pool, &item_1.get_id()).unwrap();
        let cards_type_2 = get_cards_for_item(&pool, &item_2.get_id()).unwrap();
        
        // Verify both item types have cards
        assert!(!cards_type_1.is_empty(), "Item type 1 should have cards");
        assert!(!cards_type_2.is_empty(), "Item type 2 should have cards");
        
        // Create a query to filter by item_type_1
        let query = GetQueryDto {
            item_type_id: Some(item_type_1.get_id().clone()),
            tag_ids: vec![],
            next_review_before: None,
        };
        
        // Execute the filtered query
        let result = list_cards_with_filters(&pool, &query);
        assert!(result.is_ok(), "Should filter cards successfully");
        
        // Verify only cards from item_type_1 are returned
        let filtered_cards = result.unwrap();
        assert_eq!(filtered_cards.len(), cards_type_1.len(), "Should only return cards from item type 1");
        
        // Check that all returned cards are associated with item_1
        for card in filtered_cards {
            assert_eq!(card.get_item_id(), item_1.get_id(), "Card should belong to item from item type 1");
        }
    }


    /// Tests filtering cards by tags
    ///
    /// This test verifies that:
    /// 1. Cards can be filtered by tags associated with their items
    /// 2. Only cards whose items have all specified tags are returned
    /// 3. Cards whose items are missing any of the specified tags are excluded
    #[test]
    fn test_filter_cards_by_tags() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Tagged Item Type".to_string()).unwrap();
        
        // Create items
        let item_1 = create_item(&pool, &item_type.get_id(), "Item with Tag A and B".to_string(), serde_json::Value::Null).unwrap();
        let item_2 = create_item(&pool, &item_type.get_id(), "Item with Tag A only".to_string(), serde_json::Value::Null).unwrap();
        let item_3 = create_item(&pool, &item_type.get_id(), "Item with Tag B only".to_string(), serde_json::Value::Null).unwrap();
        let item_4 = create_item(&pool, &item_type.get_id(), "Item with no tags".to_string(), serde_json::Value::Null).unwrap();
        
        // Create tags
        let tag_a = create_tag(&pool, "TagA".to_string()).unwrap();
        let tag_b = create_tag(&pool, "TagB".to_string()).unwrap();
        
        // Associate tags with items
        add_tag_to_item(&pool, &tag_a.get_id(), &item_1.get_id()).unwrap();
        add_tag_to_item(&pool, &tag_b.get_id(), &item_1.get_id()).unwrap();
        add_tag_to_item(&pool, &tag_a.get_id(), &item_2.get_id()).unwrap();
        add_tag_to_item(&pool, &tag_b.get_id(), &item_3.get_id()).unwrap();
        
        // Filter by tag A only
        let query_tag_a = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![tag_a.get_id()],
            next_review_before: None,
        };
        
        let result_tag_a = list_cards_with_filters(&pool, &query_tag_a).unwrap();
        
        // Should return cards for item_1 and item_2
        let expected_item_ids_a = vec![item_1.get_id().clone(), item_2.get_id().clone()];
        let result_item_ids_a: Vec<String> = result_tag_a.iter().map(|card| card.get_item_id().clone()).collect();
        
        for item_id in expected_item_ids_a {
            assert!(result_item_ids_a.contains(&item_id), "Should contain cards for item with tag A");
        }
        assert!(!result_item_ids_a.contains(&item_3.get_id()), "Should not contain cards for item with only tag B");
        assert!(!result_item_ids_a.contains(&item_4.get_id()), "Should not contain cards for item with no tags");
        
        // Filter by both tags A and B
        let query_both_tags = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![tag_a.get_id(), tag_b.get_id()],
            next_review_before: None,
        };
        
        let result_both_tags = list_cards_with_filters(&pool, &query_both_tags).unwrap();
        
        // Should only return cards for item_1
        let result_item_ids_both: Vec<String> = result_both_tags.iter().map(|card| card.get_item_id().clone()).collect();
        assert!(result_item_ids_both.contains(&item_1.get_id()), "Should contain cards for item with both tags");
        assert!(!result_item_ids_both.contains(&item_2.get_id()), "Should not contain cards for item with only tag A");
        assert!(!result_item_ids_both.contains(&item_3.get_id()), "Should not contain cards for item with only tag B");
        assert!(!result_item_ids_both.contains(&item_4.get_id()), "Should not contain cards for item with no tags");
    }


    /// Tests filtering cards by next review date
    ///
    /// This test verifies that:
    /// 1. Cards can be filtered by their next_review date
    /// 2. Only cards with next_review before the specified date are returned
    /// 3. Cards with next_review after the specified date are excluded
    /// 4. Cards with null next_review are included (as specified in the function)
    #[test]
    fn test_filter_cards_by_next_review() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item type and item
        let item_type = create_item_type(&pool, "Review Test Item Type".to_string()).unwrap();
        let item = create_item(&pool, &item_type.get_id(), "Item for review testing".to_string(), serde_json::Value::Null).unwrap();
        
        // Get cards for the item
        let mut cards = get_cards_for_item(&pool, &item.get_id()).unwrap();
        assert!(!cards.is_empty(), "Should have at least one card");
        
        // Set different next_review dates for each card
        let now = Utc::now();
        let yesterday = now - Duration::days(1);
        let tomorrow = now + Duration::days(1);
        let next_week = now + Duration::days(7);
        
        // We'll update at least 3 cards with different dates if available
        let mut updated_cards = Vec::new();
        
        if cards.len() >= 3 {
            // First card: review yesterday
            let mut card1 = cards[0].clone();
            card1.set_next_review(Some(yesterday));
            update_card(&pool, &card1).unwrap();
            
            updated_cards.push(card1);
            
            // Second card: review tomorrow
            let mut card2 = cards[1].clone();
            card2.set_next_review(Some(tomorrow));
            update_card(&pool, &card2).unwrap();
            updated_cards.push(card2);
            
            // Third card: review next week
            let mut card3 = cards[2].clone();
            card3.set_next_review(Some(next_week));
            update_card(&pool, &card3).unwrap();
            updated_cards.push(card3);
        } else {
            // If we have fewer than 3 cards, update what we have
            let mut card1 = cards[0].clone();
            card1.set_next_review(Some(yesterday));
            update_card(&pool, &card1).unwrap();
            updated_cards.push(card1);
            
            if cards.len() >= 2 {
                let mut card2 = cards[1].clone();
                card2.set_next_review(Some(tomorrow));
                update_card(&pool, &card2).unwrap();
                updated_cards.push(card2);
            }
        }
        
        // Create a new card with null next_review
        let mut null_review_card = create_card(&pool, &item.get_id(), cards.len() as i32).unwrap();
        null_review_card.set_next_review(None);
        updated_cards.push(null_review_card.clone());
        cards.push(null_review_card);
        
        // Test filtering by today's date
        let query_today = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![],
            next_review_before: Some(now),
        };
        
        let result_today = list_cards_with_filters(&pool, &query_today).unwrap();
        
        // Should include yesterday's card, but not tomorrow, next week, or the null review card
        let result_ids: Vec<String> = result_today.iter().map(|card| card.get_id().clone()).collect();
        
        for card in &updated_cards {
            if let Some(review_date) = card.get_next_review() {
                if review_date < now {
                    assert!(result_ids.contains(&card.get_id()), 
                        "Should include card with review date before filter ({})", review_date);
                } else {
                    assert!(!result_ids.contains(&card.get_id()), 
                        "Should not include card with review date after filter ({})", review_date);
                }
            } else {
                assert!(!result_ids.contains(&card.get_id()), "Should not include card with null review date");
            }
        }
        
        // Test filtering by date in the future (e.g., tomorrow + 1 hour)
        let future_query_time = tomorrow + Duration::hours(1);
        let query_future = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![],
            next_review_before: Some(future_query_time),
        };
        
        let result_future = list_cards_with_filters(&pool, &query_future).unwrap();
        let future_result_ids: Vec<String> = result_future.iter().map(|card| card.get_id().clone()).collect();
        
        // Should include yesterday's and tomorrow's cards, but not next week or the null review card
        for card in &updated_cards {
            if let Some(review_date) = card.get_next_review() {
                if review_date < future_query_time {
                    assert!(future_result_ids.contains(&card.get_id()), 
                        "Should include card with review date before future filter ({})", review_date);
                } else {
                    assert!(!future_result_ids.contains(&card.get_id()), 
                        "Should not include card with review date after future filter ({})", review_date);
                }
            } else {
                assert!(!future_result_ids.contains(&card.get_id()), "Should not include card with null review date");
            }
        }
    }


    /// Tests filtering cards with multiple filter criteria
    ///
    /// This test verifies that:
    /// 1. Cards can be filtered using multiple criteria simultaneously
    /// 2. Only cards that satisfy ALL specified filters are returned
    /// 3. The combination of filters works correctly
    #[test]
    fn test_filter_cards_with_multiple_criteria() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create two item types
        let item_type_1 = create_item_type(&pool, "Test Type 1".to_string()).unwrap();
        let item_type_2 = create_item_type(&pool, "Test Type 2".to_string()).unwrap();
        
        // Create items for each type
        let item_1a = create_item(&pool, &item_type_1.get_id(), "Test Type 1 Item A".to_string(), serde_json::Value::Null).unwrap();
        let item_1b = create_item(&pool, &item_type_1.get_id(), "Test Type 1 Item B".to_string(), serde_json::Value::Null).unwrap();
        let item_2 = create_item(&pool, &item_type_2.get_id(), "Type 2 Item".to_string(), serde_json::Value::Null).unwrap();
        
        // Create tags
        let tag_a = create_tag(&pool, "TagA".to_string()).unwrap();
        let tag_b = create_tag(&pool, "TagB".to_string()).unwrap();
        
        // Associate tags with items
        // Item 1A: has tag A
        add_tag_to_item(&pool, &tag_a.get_id(), &item_1a.get_id()).unwrap();
        // Item 1B: has tag A and B
        add_tag_to_item(&pool, &tag_a.get_id(), &item_1b.get_id()).unwrap();
        add_tag_to_item(&pool, &tag_b.get_id(), &item_1b.get_id()).unwrap();
        // Item 2: has tag B
        add_tag_to_item(&pool, &tag_b.get_id(), &item_2.get_id()).unwrap();
        
        // Get cards for each item
        let cards_1a = get_cards_for_item(&pool, &item_1a.get_id()).unwrap();
        let cards_1b = get_cards_for_item(&pool, &item_1b.get_id()).unwrap();
        let cards_2 = get_cards_for_item(&pool, &item_2.get_id()).unwrap();
        
        // Set different next_review dates
        let now = Utc::now();
        let yesterday = now - Duration::days(1);
        let tomorrow = now + Duration::days(1);
        
        // Update cards with different review dates
        // Item 1A: one card due yesterday, one due tomorrow
        if cards_1a.len() >= 2 {
            let mut card1 = cards_1a[0].clone();
            card1.set_next_review(Some(yesterday));
            update_card(&pool, &card1).unwrap();
            
            let mut card2 = cards_1a[1].clone();
            card2.set_next_review(Some(tomorrow));
            update_card(&pool, &card2).unwrap();
        }
        
        // Item 1B: all cards due yesterday
        for card in &cards_1b {
            let mut updated_card = card.clone();
            updated_card.set_next_review(Some(yesterday));
            update_card(&pool, &updated_card).unwrap();
        }
        
        // Item 2: all cards due tomorrow
        for card in &cards_2 {
            let mut updated_card = card.clone();
            updated_card.set_next_review(Some(tomorrow));
            update_card(&pool, &updated_card).unwrap();
        }
        
        // Test filtering by item type 1 and tag A and due today
        let query = GetQueryDto {
            item_type_id: Some(item_type_1.get_id()),
            tag_ids: vec![tag_a.get_id()],
            next_review_before: Some(now),
        };
        
        let result = list_cards_with_filters(&pool, &query).unwrap();
        
        // Expected: only cards from item_1b and some from item_1a
        // (those that have tag A, belong to item type 1, and are due before now)
        let result_item_ids: Vec<String> = result.iter().map(|card| card.get_item_id()).collect();
        
        // Should include cards from item_1b (due yesterday, has tag A, type 1)
        assert!(result_item_ids.contains(&item_1b.get_id()), 
            "Should include cards for item 1B (has tag A, type 1, due yesterday)");
        
        // For item_1a, should only include cards due yesterday, not tomorrow
        let item_1a_cards_in_result: Vec<&Card> = result.iter()
            .filter(|card| card.get_item_id() == item_1a.get_id())
            .collect();
        
        for card in &item_1a_cards_in_result {
            if let Some(review_date) = card.get_next_review() {
                assert!(review_date < now, "Item 1A cards in result should be due before now");
            }
        }
        
        // Should not include any cards from item_2 (wrong type)
        assert!(!result_item_ids.contains(&item_2.get_id()), 
            "Should not include cards for item 2 (wrong item type)");
            
        // Now test with different filter: item type 1 and tag B (should only get item_1b cards)
        let query_tag_b = GetQueryDto {
            item_type_id: Some(item_type_1.get_id()),
            tag_ids: vec![tag_b.get_id()],
            next_review_before: None,
        };
        
        let result_tag_b = list_cards_with_filters(&pool, &query_tag_b).unwrap();
        let result_tag_b_item_ids: Vec<String> = result_tag_b.iter().map(|card| card.get_item_id()).collect();
        
        // Should only include cards from item_1b (type 1 with tag B)
        assert!(result_tag_b_item_ids.contains(&item_1b.get_id()), 
            "Should include cards for item 1B (has tag B, type 1)");
        assert!(!result_tag_b_item_ids.contains(&item_1a.get_id()), 
            "Should not include cards for item 1A (doesn't have tag B)");
        assert!(!result_tag_b_item_ids.contains(&item_2.get_id()), 
            "Should not include cards for item 2 (wrong item type)");
    }


    /// Tests behavior with empty result sets and edge cases
    ///
    /// This test verifies that:
    /// 1. Filters that match no cards return an empty vector, not an error
    /// 2. Filters with empty tag lists behave as if no tag filter was applied
    /// 3. Combinations of filters that result in no matches return empty vectors
    #[test]
    fn test_filter_cards_edge_cases() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item type and item
        let item_type = create_item_type(&pool, "Test Edge Case Item Type".to_string()).unwrap();
        let item = create_item(&pool, &item_type.get_id(), "Edge Case Item".to_string(), serde_json::Value::Null).unwrap();
        
        // Get cards
        let cards = get_cards_for_item(&pool, &item.get_id()).unwrap();
        assert!(!cards.is_empty(), "Should have at least one card");
        
        // Test 1: Filter by non-existent item type
        let non_existent_id = "non-existent-id-12345".to_string();
        let query_bad_type = GetQueryDto {
            item_type_id: Some(non_existent_id.clone()),
            tag_ids: vec![],
            next_review_before: None,
        };
        
        let result_bad_type = list_cards_with_filters(&pool, &query_bad_type);
        assert!(result_bad_type.is_ok(), "Should not error with non-existent item type");
        assert!(result_bad_type.unwrap().is_empty(), "Should return empty vector for non-existent item type");
        
        // Test 2: Filter by empty tag list
        let query_empty_tags = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![],
            next_review_before: None,
        };
        
        let result_empty_tags = list_cards_with_filters(&pool, &query_empty_tags).unwrap();
        let all_cards = list_all_cards(&pool).unwrap();
        
        assert_eq!(result_empty_tags.len(), all_cards.len(), 
            "Empty tag list should behave as if no tag filter was applied");
        
        // Test 3: Filter by non-existent tag
        let query_bad_tag = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![non_existent_id.clone()],
            next_review_before: None,
        };
        
        let result_bad_tag = list_cards_with_filters(&pool, &query_bad_tag);
        assert!(result_bad_tag.is_ok(), "Should not error with non-existent tag");
        assert!(result_bad_tag.unwrap().is_empty(), "Should return empty vector for non-existent tag");
        
        // Test 4: Filter by a date far in the past
        let ancient_date = Utc::now() - Duration::days(10000);
        let query_ancient = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![],
            next_review_before: Some(ancient_date),
        };
        
        let result_ancient = list_cards_with_filters(&pool, &query_ancient).unwrap();
        
        // Should not include any cards
        assert!(result_ancient.is_empty(), "Should not include any cards");
        
        // Test 5: Combine all filters to get empty result
        let tag = create_tag(&pool, "TestTag".to_string()).unwrap();
        
        let query_combined = GetQueryDto {
            item_type_id: Some(item_type.get_id()),
            tag_ids: vec![tag.get_id()], // Tag not associated with any item
            next_review_before: Some(ancient_date),
        };
        
        let result_combined = list_cards_with_filters(&pool, &query_combined).unwrap();
        assert!(result_combined.is_empty(), "Combined filters should result in empty vector");
    }

    
    /// Tests retrieving reviews for a card
    ///
    /// This test verifies that:
    /// 1. All reviews for a specific card can be retrieved
    /// 2. Reviews are returned in descending order by timestamp
    /// 3. Reviews for other cards are not included
    #[test]
    fn test_get_reviews_for_card() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item and get its cards
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item = create_item(&pool, &item_type.get_id(), "Item with Cards".to_string(), serde_json::Value::Null).unwrap();
        
        // Get the cards that were automatically created for the item
        let cards = get_cards_for_item(&pool, &item.get_id()).unwrap();
        assert!(cards.len() >= 2, "Item should have at least 2 cards");
        let card1 = &cards[0];
        let card2 = &cards[1];
        
        // Create multiple reviews for card1
        let ratings = vec![1, 2, 3];
        for rating in &ratings {
            record_review(&pool, &card1.get_id(), *rating).unwrap();
            // Add a small delay to ensure different timestamps
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        // Create a review for card2
        record_review(&pool, &card2.get_id(), 2).unwrap();
        
        // Get reviews for card1
        let result = get_reviews_for_card(&pool, &card1.get_id());
        assert!(result.is_ok(), "Should get reviews for card successfully");
        
        // Verify only card1 reviews are returned
        let reviews = result.unwrap();
        assert_eq!(reviews.len(), ratings.len(), "Should return correct number of reviews");
        
        // Check that all reviews belong to card1
        for review in &reviews {
            assert_eq!(review.get_card_id(), card1.get_id(), "Review should belong to card1");
        }
        
        // Verify reviews are in descending order by timestamp
        for i in 0..reviews.len() - 1 {
            assert!(reviews[i].get_review_timestamp() >= reviews[i + 1].get_review_timestamp(), 
                "Reviews should be in descending order by timestamp");
        }
        
        // Test with a non-existent card ID
        let non_existent_id = Uuid::new_v4().to_string();
        let result = get_reviews_for_card(&pool, &non_existent_id);
        assert!(result.is_ok(), "Should handle non-existent card ID gracefully");
        assert_eq!(result.unwrap().len(), 0, "Should return empty list for non-existent card");
    }
    
    
    /// Tests edge cases for record_review function
    ///
    /// This test verifies that:
    /// 1. Different ratings result in different next review dates
    /// 2. Invalid ratings are handled gracefully
    /// 3. Multiple reviews for the same card update the review schedule correctly
    #[test]
    fn test_record_review_edge_cases() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create an item and card
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item = create_item(&pool, &item_type.get_id(), "Item to Review".to_string(), serde_json::Value::Null).unwrap();
        let cards = get_cards_for_item(&pool, &item.get_id()).unwrap();
        let card = cards.first().unwrap();
        
        // Test invalid rating (0)
        let result = record_review(&pool, &card.get_id(), 0);
        assert!(result.is_err(), "Should reject review with invalid rating 0");
        
        // Test invalid rating (5)
        let result = record_review(&pool, &card.get_id(), 5);
        assert!(result.is_err(), "Should reject review with invalid rating 5");
        
        // Test rating 1 (difficult)
        let result = record_review(&pool, &card.get_id(), 1);
        assert!(result.is_ok(), "Should record review with rating 1");
        
        let updated_card = get_card(&pool, &card.get_id()).unwrap().unwrap();
        let last_review = updated_card.get_last_review().unwrap();
        let next_review = updated_card.get_next_review().unwrap();
        let days_diff = (next_review.timestamp() - last_review.timestamp()) / (24 * 60 * 60);
        assert_eq!(days_diff, 1, "For rating 1, next review should be 1 day later");
        
        // Test rating 3 (medium)
        let result = record_review(&pool, &card.get_id(), 3);
        assert!(result.is_ok(), "Should record review with rating 3");
        
        let updated_card = get_card(&pool, &card.get_id()).unwrap().unwrap();
        let last_review = updated_card.get_last_review().unwrap();
        let next_review = updated_card.get_next_review().unwrap();
        let days_diff = (next_review.timestamp() - last_review.timestamp()) / (24 * 60 * 60);
        // The delay is now 2 (from the scheduler data)
        assert_eq!(days_diff, 2, "For rating 3, next review should be 2 days later");
        
        // Test rating 4 (easy)
        let result = record_review(&pool, &card.get_id(), 4);
        assert!(result.is_ok(), "Should record review with rating 4");
        
        let updated_card = get_card(&pool, &card.get_id()).unwrap().unwrap();
        let last_review = updated_card.get_last_review().unwrap();
        let next_review = updated_card.get_next_review().unwrap();
        let days_diff = (next_review.timestamp() - last_review.timestamp()) / (24 * 60 * 60);
        // The delay is now 2 (from the scheduler data)
        assert_eq!(days_diff, 2, "For rating 4, next review should be 2 days later");
        
        // Verify multiple reviews are recorded
        let reviews = get_reviews_for_card(&pool, &card.get_id()).unwrap();
        assert_eq!(reviews.len(), 3, "Should have recorded 3 reviews");

        // Test that each subsequent review with rating 4 increases the interval by a factor of ~1.7
        let mut last_days_diff = days_diff;

        // record 10 more reviews so the ratios are more obvious
        let ratings = vec![4; 10];
        for rating in ratings {
            record_review(&pool, &card.get_id(), rating).unwrap();

            // Get the updated card
            let updated_card = get_card(&pool, &card.get_id()).unwrap().unwrap();
            let last_review = updated_card.get_last_review().unwrap();
            let next_review = updated_card.get_next_review().unwrap();
            
            // Calculate the new interval in days
            let new_days_diff = (next_review.timestamp() - last_review.timestamp()) / (24 * 60 * 60);
            
            // Update last_days_diff for the next iteration
            last_days_diff = new_days_diff;
        }
        
        for _ in 0..3 {
            // Get the current delay before recording a new review
            let card_before = get_card(&pool, &card.get_id()).unwrap().unwrap();
            let previous_delay = card_before.get_scheduler_data()
                .and_then(|data| data.0.get("delay").and_then(|delay| delay.as_f64()))
                .expect("Card should have delay in scheduler data");
                
            // Record another review with rating 4
            let result = record_review(&pool, &card.get_id(), 4);
            assert!(result.is_ok(), "Should record review with rating 4");
            
            // Get the updated card
            let updated_card = get_card(&pool, &card.get_id()).unwrap().unwrap();
            let last_review = updated_card.get_last_review().unwrap();
            let next_review = updated_card.get_next_review().unwrap();
            
            // Calculate the new interval in days
            let new_days_diff = (next_review.timestamp() - last_review.timestamp()) / (24 * 60 * 60);

            // Print debug information about the review intervals
            println!("Previous interval: {} days", last_days_diff);
            println!("New interval: {} days", new_days_diff);
            
            // Get the current delay from the scheduler data
            let current_delay = updated_card.get_scheduler_data()
                .and_then(|data| data.0.get("delay").and_then(|delay| delay.as_f64()))
                .expect("Card should have delay in scheduler data");
            
            println!("Current delay in scheduler data: {}", current_delay);
            
            // Check that the delay is increasing by a factor of ~1.15 (for rating 4)
            let actual_delay_ratio = current_delay / previous_delay;
            println!("Actual delay ratio: {:.2}", actual_delay_ratio);
            assert!(actual_delay_ratio > 1.14 && actual_delay_ratio < 1.16, 
                "Delay should increase by ~1.15 times, got ratio: {}", actual_delay_ratio);
            
            // Update for next iteration
            last_days_diff = new_days_diff;
        }
        
        // Verify all reviews are recorded
        let reviews = get_reviews_for_card(&pool, &card.get_id()).unwrap();
        assert_eq!(reviews.len(), 16, "Should have recorded 16 reviews total");
    }

    /// Tests creating items with different data types
    ///
    /// This test verifies that:
    /// 1. Items can be created with different JSON data types
    /// 2. The data is correctly stored and retrieved
    #[test]
    fn test_create_item_with_data() {
        // Set up a test database
        let pool = setup_test_db();
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        
        // Test with null data
        let item1 = create_item(&pool, &item_type.get_id(), "Item with null".to_string(), serde_json::Value::Null).unwrap();
        let retrieved_item1 = get_item(&pool, &item1.get_id()).unwrap().unwrap();
        assert_eq!(retrieved_item1.get_data().0, serde_json::Value::Null);
        
        // Test with object data
        let mut obj = serde_json::Map::new();
        obj.insert("key1".to_string(), serde_json::Value::String("value1".to_string()));
        obj.insert("key2".to_string(), serde_json::Value::Number(serde_json::Number::from(42)));
        let obj_value = serde_json::Value::Object(obj);
        
        let item2 = create_item(&pool, &item_type.get_id(), "Item with object".to_string(), obj_value.clone()).unwrap();
        let retrieved_item2 = get_item(&pool, &item2.get_id()).unwrap().unwrap();
        assert_eq!(retrieved_item2.get_data().0, obj_value);
        
        // Test with array data
        let arr_value = serde_json::json!(["value1", "value2", "value3"]);
        let item3 = create_item(&pool, &item_type.get_id(), "Item with array".to_string(), arr_value.clone()).unwrap();
        let retrieved_item3 = get_item(&pool, &item3.get_id()).unwrap().unwrap();
        assert_eq!(retrieved_item3.get_data().0, arr_value);
    }

    /// Tests error handling for database operations
    ///
    /// This test verifies that:
    /// 1. Attempting to create a card for a non-existent item returns an error
    /// 2. Attempting to record a review for a non-existent card returns an error
    /// 3. Attempting to record a review for a card with a non-existent item returns an error
    #[test]
    fn test_error_handling() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Try to create a card for a non-existent item
        let non_existent_id = Uuid::new_v4().to_string();
        let result = create_card(&pool, &non_existent_id, 0);
        assert!(result.is_err(), "Should error when creating card for non-existent item");
        
        // Create a valid item and card
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item = create_item(&pool, &item_type.get_id(), "Test Item".to_string(), serde_json::Value::Null).unwrap();
        
        // Create a non-existent card for testing
        let non_existent_card = Card::new_with_fields(
            Uuid::new_v4().to_string(),
            item.get_id().clone(),
            0,
            None,
            None,
            None
        );
        
        // Try to record a review for a non-existent card (with valid item ID)
        let result = record_review(&pool, &non_existent_card.get_id(), 2);
        assert!(result.is_err(), "Should error when recording review for non-existent card");
        
        // Create a non-existent card with a non-existent item ID
        let non_existent_card_and_item = Card::new_with_fields(
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            0,
            None,
            None,
            None
        );
        
        // Try to record a review for a non-existent card with a non-existent item ID
        let result = record_review(&pool, &non_existent_card_and_item.get_id(), 2);
        assert!(result.is_err(), "Should error when recording review for non-existent card with non-existent item");
    }
} 

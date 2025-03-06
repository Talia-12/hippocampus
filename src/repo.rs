/// Repository module
///
/// This module provides the data access layer for the application.
/// It contains functions for interacting with the database, including
/// creating, retrieving, and updating items and reviews.
/// 
/// The repository pattern abstracts away the details of database access
/// and provides a clean API for the rest of the application to use.
use crate::db::DbPool;
use crate::models::{Card, Item, ItemType, JsonValue, Review};
use crate::schema::{cards, items, reviews};
use diesel::prelude::*;
use anyhow::Result;
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
    let conn = &mut pool.get()?;
    
    // Create a new item with the provided title
    let new_item = Item::new(item_type_id.to_string(), new_title, JsonValue(item_data));
    
    // Insert the new item into the database
    diesel::insert_into(items::table)
        .values(&new_item)
        .execute(conn)?;
    
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
pub fn create_card(pool: &DbPool, item_id: &str, card_index: i32) -> Result<Card> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Create a new card with the provided item ID and card index
    let new_card = Card::new(item_id.to_string(), card_index);
    
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


/// Retrieves all cards from the database
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
pub fn list_cards(pool: &DbPool) -> Result<Vec<Card>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all cards
    let result = cards::table.load::<Card>(conn)?;
    
    // Return the list of cards
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
/// - Rating 1 (difficult): Review again tomorrow
/// - Rating 2 (medium): Review again in 3 days
/// - Rating 3 (easy): Review again in 7 days
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id_val` - The ID of the item being reviewed
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
/// - The item does not exist
/// - The database insert or update operations fail
pub fn record_review(pool: &DbPool, card_id: &str, rating_val: i32) -> Result<Review> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
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
    card.last_review = Some(now.naive_utc());
    
    // Simple spaced repetition logic
    // Determine when to schedule the next review based on the rating
    let days_to_add = match rating_val {
        1 => 1,  // If difficult, review tomorrow
        2 => 3,  // If medium, review in 3 days
        3 => 7,  // If easy, review in a week
        _ => 1,  // Default to tomorrow for any unexpected rating
    };
    
    // Calculate the next review time
    card.next_review = Some(now.naive_utc() + Duration::days(days_to_add));
    
    // Update the card in the database with the new review information
    diesel::update(cards::table.filter(cards::id.eq(card.id)))
        .set((
            cards::next_review.eq(card.next_review),
            cards::last_review.eq(card.last_review),
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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use diesel::connection::SimpleConnection;
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    use uuid::Uuid;
    
    /// Embedded migrations for testing
    /// 
    /// This constant holds the embedded migrations that will be run
    /// on the test database to set up the schema.
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
    
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
        conn.run_pending_migrations(MIGRATIONS).expect("Failed to run migrations");
        
        pool
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
        assert_eq!(item_type.name, name);
        assert!(!item_type.id.is_empty());
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
        let result = create_item(&pool, &item_type.id, title.clone(), serde_json::Value::Null);
        assert!(result.is_ok(), "Should create an item successfully");
        
        // Verify the created item
        let item = result.unwrap();
        assert_eq!(item.title, title);
        assert!(!item.id.is_empty());
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
        let created_item = create_item(&pool, &item_type.id, title.clone(), serde_json::Value::Null).unwrap();
        
        // Then try to get it
        let result = get_item(&pool, &created_item.id);
        assert!(result.is_ok(), "Should get an item successfully");
        
        // Verify the item exists
        let item_option = result.unwrap();
        assert!(item_option.is_some(), "Item should exist");
        
        // Verify the item properties
        let item = item_option.unwrap();
        assert_eq!(item.id, created_item.id);
        assert_eq!(item.title, title);
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
            create_item(&pool, &item_type.id, title.to_string(), serde_json::Value::Null).unwrap();
        }
        
        // List all items
        let result = list_items(&pool);
        assert!(result.is_ok(), "Should list items successfully");
        
        // Verify the correct number of items
        let items = result.unwrap();
        assert_eq!(items.len(), titles.len(), "Should have the correct number of items");
        
        // Check that all titles are present
        let item_titles: Vec<String> = items.iter().map(|item| item.title.clone()).collect();
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
        
        // First create an item
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item = create_item(&pool, &item_type.id, "Item to Review".to_string(), serde_json::Value::Null).unwrap();
        let card = create_card(&pool, &item.id, 0).unwrap();
        
        // Record a review
        let rating = 2;
        let result = record_review(&pool, &card.id, rating);
        assert!(result.is_ok(), "Should record a review successfully");
        
        // Verify the review properties
        let review = result.unwrap();
        assert_eq!(review.card_id, card.id);
        assert_eq!(review.rating, rating);
        
        // Check that the item was updated with review information
        let updated_card = get_card(&pool, &card.id).unwrap().unwrap();
        assert!(updated_card.last_review.is_some(), "Last review should be set");
        assert!(updated_card.next_review.is_some(), "Next review should be set");
        
        // For rating 2, next review should be 3 days later
        let last_review = updated_card.last_review.unwrap();
        let next_review = updated_card.next_review.unwrap();
        let days_diff = (next_review.and_utc().timestamp() - last_review.and_utc().timestamp()) / (24 * 60 * 60);
        assert_eq!(days_diff, 3, "For rating 2, next review should be 3 days later");
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
        let item = create_item(&pool, &item_type.id, "Item with Card".to_string(), serde_json::Value::Null).unwrap();
        
        // Create a card for the item
        let card_index = 0;
        let result = create_card(&pool, &item.id, card_index);
        assert!(result.is_ok(), "Should create a card successfully");
        
        // Verify the card properties
        let card = result.unwrap();
        assert_eq!(card.item_id, item.id);
        assert_eq!(card.card_index, card_index);
        assert!(card.next_review.is_none());
        assert!(card.last_review.is_none());
        
        // Check that the card can be retrieved
        let retrieved_card = get_card(&pool, &card.id).unwrap().unwrap();
        assert_eq!(retrieved_card.id, card.id);
        assert_eq!(retrieved_card.item_id, item.id);
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
        let item = create_item(&pool, &item_type.id, "Item with Card".to_string(), serde_json::Value::Null).unwrap();
        let card = create_card(&pool, &item.id, 0).unwrap();
        
        // Retrieve the card
        let result = get_card(&pool, &card.id);
        assert!(result.is_ok(), "Should retrieve a card successfully");
        
        // Verify the correct card is returned
        let retrieved_card = result.unwrap().unwrap();
        assert_eq!(retrieved_card.id, card.id);
        assert_eq!(retrieved_card.item_id, item.id);
        
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
    #[test]
    fn test_get_cards_for_item() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create two items
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item1 = create_item(&pool, &item_type.id, "Item 1".to_string(), serde_json::Value::Null).unwrap();
        let item2 = create_item(&pool, &item_type.id, "Item 2".to_string(), serde_json::Value::Null).unwrap();
        
        // Create multiple cards for each item
        let card1_1 = create_card(&pool, &item1.id, 0).unwrap();
        let card1_2 = create_card(&pool, &item1.id, 1).unwrap();
        let _card2_1 = create_card(&pool, &item2.id, 0).unwrap();
        
        // Retrieve cards for item1
        let result = get_cards_for_item(&pool, &item1.id);
        assert!(result.is_ok(), "Should retrieve cards successfully");
        
        // Verify the correct cards are returned
        let cards = result.unwrap();
        assert_eq!(cards.len(), 2, "Should return the correct number of cards");
        
        // Check that all cards belong to item1
        for card in &cards {
            assert_eq!(card.item_id, item1.id, "Card should belong to the correct item");
        }
        
        // Check that the specific cards are included
        let card_ids: Vec<String> = cards.iter().map(|card| card.id.clone()).collect();
        assert!(card_ids.contains(&card1_1.id), "Should contain the first card");
        assert!(card_ids.contains(&card1_2.id), "Should contain the second card");
    }
    

    /// Tests listing all cards in the database
    ///
    /// This test verifies that:
    /// 1. All cards can be retrieved from the database
    /// 2. The correct number of cards is returned
    #[test]
    fn test_list_cards() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create items and cards
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item1 = create_item(&pool, &item_type.id, "Item 1".to_string(), serde_json::Value::Null).unwrap();
        let item2 = create_item(&pool, &item_type.id, "Item 2".to_string(), serde_json::Value::Null).unwrap();
        
        let card1 = create_card(&pool, &item1.id, 0).unwrap();
        let card2 = create_card(&pool, &item1.id, 1).unwrap();
        let card3 = create_card(&pool, &item2.id, 0).unwrap();
        
        // List all cards
        let result = list_cards(&pool);
        assert!(result.is_ok(), "Should list cards successfully");
        
        // Verify the correct number of cards is returned
        let cards = result.unwrap();
        assert_eq!(cards.len(), 3, "Should return all cards");
        
        // Check that all created cards are included
        let card_ids: Vec<String> = cards.iter().map(|card| card.id.clone()).collect();
        assert!(card_ids.contains(&card1.id), "Should contain the first card");
        assert!(card_ids.contains(&card2.id), "Should contain the second card");
        assert!(card_ids.contains(&card3.id), "Should contain the third card");
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
        let type_names_from_db: Vec<String> = item_types.iter().map(|it| it.name.clone()).collect();
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
        let result = get_item_type(&pool, &item_type.id);
        assert!(result.is_ok(), "Should retrieve an item type successfully");
        
        // Verify the correct item type is returned
        let retrieved_type = result.unwrap().unwrap();
        assert_eq!(retrieved_type.id, item_type.id);
        assert_eq!(retrieved_type.name, name);
        
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
        let type1 = create_item_type(&pool, "Type 1".to_string()).unwrap();
        let type2 = create_item_type(&pool, "Type 2".to_string()).unwrap();
        
        // Create items of different types
        let type1_titles = vec!["Type1 Item 1", "Type1 Item 2", "Type1 Item 3"];
        let type2_titles = vec!["Type2 Item 1", "Type2 Item 2"];
        
        for title in &type1_titles {
            create_item(&pool, &type1.id, title.to_string(), serde_json::Value::Null).unwrap();
        }
        
        for title in &type2_titles {
            create_item(&pool, &type2.id, title.to_string(), serde_json::Value::Null).unwrap();
        }
        
        // Get items of type1
        let result = get_items_by_type(&pool, &type1.id);
        assert!(result.is_ok(), "Should get items by type successfully");
        
        // Verify only type1 items are returned
        let items = result.unwrap();
        assert_eq!(items.len(), type1_titles.len(), "Should return correct number of items");
        
        // Check that all type1 titles are present
        let item_titles: Vec<String> = items.iter().map(|item| item.title.clone()).collect();
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
        let item = create_item(&pool, &item_type.id, "Item with Cards".to_string(), serde_json::Value::Null).unwrap();
        
        // Create multiple cards for the item
        let card_indices = vec![0, 1, 2];
        for index in &card_indices {
            create_card(&pool, &item.id, *index).unwrap();
        }
        
        // List all cards
        let result = list_cards(&pool);
        assert!(result.is_ok(), "Should list cards successfully");
        
        // Verify the correct number of cards
        let cards = result.unwrap();
        assert_eq!(cards.len(), card_indices.len(), "Should have the correct number of cards");
        
        // Check that all card indices are present
        let card_indices_from_db: Vec<i32> = cards.iter().map(|card| card.card_index).collect();
        for index in card_indices {
            assert!(card_indices_from_db.contains(&index), "Should contain card with index: {}", index);
        }
    }
    
    
    /// Tests retrieving all cards for a specific item
    ///
    /// This test verifies that:
    /// 1. All cards for a specific item can be retrieved
    /// 2. Cards for other items are not included
    /// 3. The correct number of cards is returned
    #[test]
    fn test_retrieve_cards_by_item_id() {
        // Set up a test database
        let pool = setup_test_db();
        
        // Create two items
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item1 = create_item(&pool, &item_type.id, "Item 1".to_string(), serde_json::Value::Null).unwrap();
        let item2 = create_item(&pool, &item_type.id, "Item 2".to_string(), serde_json::Value::Null).unwrap();
        
        // Create cards for item1
        let item1_indices = vec![0, 1, 2];
        for index in &item1_indices {
            create_card(&pool, &item1.id, *index).unwrap();
        }
        
        // Create cards for item2
        let item2_indices = vec![0, 1];
        for index in &item2_indices {
            create_card(&pool, &item2.id, *index).unwrap();
        }
        
        // Get cards for item1
        let result = get_cards_for_item(&pool, &item1.id);
        assert!(result.is_ok(), "Should get cards for item successfully");
        
        // Verify only item1 cards are returned
        let cards = result.unwrap();
        assert_eq!(cards.len(), item1_indices.len(), "Should return correct number of cards");
        
        // Check that all cards belong to item1
        for card in &cards {
            assert_eq!(card.item_id, item1.id, "Card should belong to item1");
        }
        
        // Check that all indices for item1 are present
        let indices: Vec<i32> = cards.iter().map(|card| card.card_index).collect();
        for index in item1_indices {
            assert!(indices.contains(&index), "Should contain card with index: {}", index);
        }
        
        // Test with a non-existent item ID
        let non_existent_id = Uuid::new_v4().to_string();
        let result = get_cards_for_item(&pool, &non_existent_id);
        assert!(result.is_ok(), "Should handle non-existent item ID gracefully");
        assert_eq!(result.unwrap().len(), 0, "Should return empty list for non-existent item");
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
        
        // Create an item and two cards
        let item_type = create_item_type(&pool, "Test Item Type".to_string()).unwrap();
        let item = create_item(&pool, &item_type.id, "Item with Cards".to_string(), serde_json::Value::Null).unwrap();
        let card1 = create_card(&pool, &item.id, 0).unwrap();
        let card2 = create_card(&pool, &item.id, 1).unwrap();
        
        // Create multiple reviews for card1
        let ratings = vec![1, 2, 3];
        for rating in &ratings {
            record_review(&pool, &card1.id, *rating).unwrap();
            // Add a small delay to ensure different timestamps
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        // Create a review for card2
        record_review(&pool, &card2.id, 2).unwrap();
        
        // Get reviews for card1
        let result = get_reviews_for_card(&pool, &card1.id);
        assert!(result.is_ok(), "Should get reviews for card successfully");
        
        // Verify only card1 reviews are returned
        let reviews = result.unwrap();
        assert_eq!(reviews.len(), ratings.len(), "Should return correct number of reviews");
        
        // Check that all reviews belong to card1
        for review in &reviews {
            assert_eq!(review.card_id, card1.id, "Review should belong to card1");
        }
        
        // Verify reviews are in descending order by timestamp
        for i in 0..reviews.len() - 1 {
            assert!(reviews[i].review_timestamp >= reviews[i + 1].review_timestamp, 
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
        let item = create_item(&pool, &item_type.id, "Item to Review".to_string(), serde_json::Value::Null).unwrap();
        let card = create_card(&pool, &item.id, 0).unwrap();
        
        // Test rating 1 (difficult)
        let result = record_review(&pool, &card.id, 1);
        assert!(result.is_ok(), "Should record review with rating 1");
        
        let updated_card = get_card(&pool, &card.id).unwrap().unwrap();
        let last_review = updated_card.last_review.unwrap();
        let next_review = updated_card.next_review.unwrap();
        let days_diff = (next_review.and_utc().timestamp() - last_review.and_utc().timestamp()) / (24 * 60 * 60);
        assert_eq!(days_diff, 1, "For rating 1, next review should be 1 day later");
        
        // Test rating 3 (easy)
        let result = record_review(&pool, &card.id, 3);
        assert!(result.is_ok(), "Should record review with rating 3");
        
        let updated_card = get_card(&pool, &card.id).unwrap().unwrap();
        let last_review = updated_card.last_review.unwrap();
        let next_review = updated_card.next_review.unwrap();
        let days_diff = (next_review.and_utc().timestamp() - last_review.and_utc().timestamp()) / (24 * 60 * 60);
        assert_eq!(days_diff, 7, "For rating 3, next review should be 7 days later");
        
        // Test invalid rating (should default to 1 day)
        let result = record_review(&pool, &card.id, 5);
        assert!(result.is_ok(), "Should handle invalid rating gracefully");
        
        let updated_card = get_card(&pool, &card.id).unwrap().unwrap();
        let last_review = updated_card.last_review.unwrap();
        let next_review = updated_card.next_review.unwrap();
        let days_diff = (next_review.and_utc().timestamp() - last_review.and_utc().timestamp()) / (24 * 60 * 60);
        assert_eq!(days_diff, 1, "For invalid rating, next review should default to 1 day later");
        
        // Verify multiple reviews are recorded
        let reviews = get_reviews_for_card(&pool, &card.id).unwrap();
        assert_eq!(reviews.len(), 3, "Should have recorded 3 reviews");
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
        let item1 = create_item(&pool, &item_type.id, "Item with null".to_string(), serde_json::Value::Null).unwrap();
        let retrieved_item1 = get_item(&pool, &item1.id).unwrap().unwrap();
        assert_eq!(retrieved_item1.item_data.0, serde_json::Value::Null);
        
        // Test with object data
        let mut obj = serde_json::Map::new();
        obj.insert("key1".to_string(), serde_json::Value::String("value1".to_string()));
        obj.insert("key2".to_string(), serde_json::Value::Number(serde_json::Number::from(42)));
        let obj_value = serde_json::Value::Object(obj);
        
        let item2 = create_item(&pool, &item_type.id, "Item with object".to_string(), obj_value.clone()).unwrap();
        let retrieved_item2 = get_item(&pool, &item2.id).unwrap().unwrap();
        assert_eq!(retrieved_item2.item_data.0, obj_value);
        
        // Test with array data
        let arr_value = serde_json::json!(["value1", "value2", "value3"]);
        let item3 = create_item(&pool, &item_type.id, "Item with array".to_string(), arr_value.clone()).unwrap();
        let retrieved_item3 = get_item(&pool, &item3.id).unwrap().unwrap();
        assert_eq!(retrieved_item3.item_data.0, arr_value);
    }
    
    
    /// Tests error handling for database operations
    ///
    /// This test verifies that:
    /// 1. Attempting to create a card for a non-existent item returns an error
    /// 2. Attempting to record a review for a non-existent card returns an error
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
        let item = create_item(&pool, &item_type.id, "Test Item".to_string(), serde_json::Value::Null).unwrap();
        
        // Create a non-existent card for testing
        let non_existent_card = Card {
            id: Uuid::new_v4().to_string(),
            item_id: item.id.clone(),
            card_index: 0,
            next_review: None,
            last_review: None,
            scheduler_data: None,
        };
        
        // Try to record a review for a non-existent card
        let result = record_review(&pool, &non_existent_card.id, 2);
        assert!(result.is_err(), "Should error when recording review for non-existent card");
    }
} 

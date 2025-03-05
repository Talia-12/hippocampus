/// Repository module
///
/// This module provides the data access layer for the application.
/// It contains functions for interacting with the database, including
/// creating, retrieving, and updating items and reviews.
/// 
/// The repository pattern abstracts away the details of database access
/// and provides a clean API for the rest of the application to use.
use crate::db::DbPool;
use crate::models::{Item, Review};
use crate::schema::{items, reviews};
use diesel::prelude::*;
use anyhow::Result;
use chrono::Utc;
use chrono::Duration;

/// Creates a new item in the database
///
/// # Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `new_title` - The title for the new item
///
/// # Returns
///
/// A Result containing the newly created Item if successful
///
/// # Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
pub fn create_item(pool: &DbPool, new_title: String) -> Result<Item> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Create a new item with the provided title
    let new_item = Item::new(new_title);
    
    // Insert the new item into the database
    diesel::insert_into(items::table)
        .values(&new_item)
        .execute(conn)?;
    
    // Return the newly created item
    Ok(new_item)
}

/// Retrieves an item from the database by its ID
///
/// # Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to retrieve
///
/// # Returns
///
/// A Result containing an Option with the Item if found, or None if not found
///
/// # Errors
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
/// # Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// # Returns
///
/// A Result containing a vector of all Items in the database
///
/// # Errors
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
/// # Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id_val` - The ID of the item being reviewed
/// * `rating_val` - The rating given during the review (1-3)
///
/// # Returns
///
/// A Result containing the newly created Review if successful
///
/// # Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The item does not exist
/// - The database insert or update operations fail
pub fn record_review(pool: &DbPool, item_id_val: &str, rating_val: i32) -> Result<Review> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // 1) Insert the review record
    // Create a new review with the provided item ID and rating
    let new_review = Review::new(item_id_val, rating_val);
    
    // Insert the new review into the database
    diesel::insert_into(reviews::table)
        .values(&new_review)
        .execute(conn)?;

    // 2) Retrieve the item and update next_review
    // Get the item from the database
    let mut item = items::table
        .filter(items::id.eq(item_id_val))
        .first::<Item>(conn)?;
    
    // Get the current time for updating timestamps
    let now = Utc::now();
    
    // Update the last review time to now
    item.last_review = Some(now.naive_utc());
    
    // Simple spaced repetition logic
    // Determine when to schedule the next review based on the rating
    let days_to_add = match rating_val {
        1 => 1,  // If difficult, review tomorrow
        2 => 3,  // If medium, review in 3 days
        3 => 7,  // If easy, review in a week
        _ => 1,  // Default to tomorrow for any unexpected rating
    };
    
    // Calculate the next review time
    item.next_review = Some(now.naive_utc() + Duration::days(days_to_add));
    
    // Update the item's updated_at timestamp
    item.updated_at = now.naive_utc();
    
    // Update the item in the database with the new review information
    diesel::update(items::table.filter(items::id.eq(item_id_val)))
        .set((
            items::next_review.eq(item.next_review),
            items::last_review.eq(item.last_review),
            items::updated_at.eq(item.updated_at),
        ))
        .execute(conn)?;
    
    // Return the newly created review
    Ok(new_review)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::schema;
    use diesel::connection::SimpleConnection;
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    
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
    /// # Returns
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
        
        // Create a new item
        let result = create_item(&pool, title.clone());
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
        
        // First create an item
        let created_item = create_item(&pool, title.clone()).unwrap();
        
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
        
        // Create a few items
        let titles = vec!["Item 1", "Item 2", "Item 3"];
        for title in &titles {
            create_item(&pool, title.to_string()).unwrap();
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
        let item = create_item(&pool, "Item to Review".to_string()).unwrap();
        
        // Record a review
        let rating = 2;
        let result = record_review(&pool, &item.id, rating);
        assert!(result.is_ok(), "Should record a review successfully");
        
        // Verify the review properties
        let review = result.unwrap();
        assert_eq!(review.item_id, item.id);
        assert_eq!(review.rating, rating);
        
        // Check that the item was updated with review information
        let updated_item = get_item(&pool, &item.id).unwrap().unwrap();
        assert!(updated_item.last_review.is_some(), "Last review should be set");
        assert!(updated_item.next_review.is_some(), "Next review should be set");
        
        // For rating 2, next review should be 3 days later
        let last_review = updated_item.last_review.unwrap();
        let next_review = updated_item.next_review.unwrap();
        let days_diff = (next_review.timestamp() - last_review.timestamp()) / (24 * 60 * 60);
        assert_eq!(days_diff, 3, "For rating 2, next review should be 3 days later");
    }
} 
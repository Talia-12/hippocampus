use crate::db::{DbPool, ExecuteWithRetry};
use crate::models::{Item, JsonValue};
use crate::schema::items;
use chrono::{Utc, NaiveDateTime};
use diesel::prelude::*;
use anyhow::Result;
use tracing::{instrument, debug, info};

use super::card_repo::create_cards_for_item;

/// Creates a new item in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_type_id` - The ID of the item type for this item
/// * `new_title` - The title for the new item
/// * `item_data` - JSON data specific to this item type
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
#[instrument(skip(pool, item_data), fields(item_type_id = %item_type_id, title = %new_title))]
pub async fn create_item(pool: &DbPool, item_type_id: &str, new_title: String, item_data: serde_json::Value) -> Result<Item> {
    debug!("Creating new item");
    
    // Get a connection from the pool
    let mut conn = pool.get()?;
    
    // Create a new item with the provided title
    let new_item = Item::new(item_type_id.to_string(), new_title, JsonValue(item_data));
    
    debug!("Inserting item into database with id: {}", new_item.get_id());
    
    // Insert the new item into the database
    diesel::insert_into(items::table)
        .values(new_item.clone())
        .execute_with_retry(&mut conn).await?;

    // Drop the connection back to the pool
    drop(conn);

    debug!("Creating cards for item");
    
    // Create all necessary cards for the item
    create_cards_for_item(pool, &new_item).await?;

    // TODO: If there's an error, we should delete the item and all its cards

    info!("Successfully created item with id: {}", new_item.get_id());
    
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
#[instrument(skip(pool), fields(item_id = %item_id))]
pub fn get_item(pool: &DbPool, item_id: &str) -> Result<Option<Item>> {
    debug!("Retrieving item by id");
    
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for the item with the specified ID
    let result = items::table
        .filter(items::id.eq(item_id))
        .first::<Item>(conn)
        .optional()?;
    
    if let Some(ref item) = result {
        debug!("Item found with id: {}", item.get_id());
    } else {
        debug!("Item not found");
    }
    
    // Return the result (Some(Item) if found, None if not)
    Ok(result)
}


/// Updates an item in the database by its ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to update
/// * `title` - The new title for the item
/// * `item_data` - The new JSON data for the item
///
/// ### Returns
///
/// A Result containing the updated Item if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database update operation fails
/// - The item is not found
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn update_item(pool: &DbPool, item_id: &str, title: Option<String>, item_data: Option<serde_json::Value>) -> Result<Item> {
    debug!("Updating item by id");
    
    // Get the current item to check if it exists
    let _item = get_item(pool, item_id)?
        .ok_or_else(|| anyhow::anyhow!("Item with id {} not found", item_id))?;
    
    // Always update the updated_at timestamp
    let now = Utc::now().naive_utc();
    
    // Create a struct for changeset that implements AsChangeset
    // This allows us to only include fields that are Some
    #[derive(AsChangeset)]
    #[diesel(table_name = items)]
    struct ItemChangeset {
        title: Option<String>,
        item_data: Option<JsonValue>,
        updated_at: NaiveDateTime,
    }
    
    let changeset = ItemChangeset {
        title,
        item_data: item_data.map(JsonValue),
        updated_at: now,
    };

    let mut conn = pool.get()?;
    
    // Execute the update with the dynamic changeset
    diesel::update(items::table.find(item_id.to_string()))
        .set(changeset)
        .execute_with_retry(&mut conn).await?;

    drop(conn);
    
    // Get the updated item
    let updated_item = get_item(pool, item_id)?
        .ok_or_else(|| panic!("Item with id {} not found after update", item_id))?; // this should panic because updating the item should never result in the item being deleted
    
    Ok(updated_item)
}


/// Deletes an item from the database by its ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to delete
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database delete operation fails
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn delete_item(pool: &DbPool, item_id: &str) -> Result<()> {
    debug!("Deleting item by id");

    let mut conn = pool.get()?;

    diesel::delete(items::table.find(item_id.to_string()))
        .execute_with_retry(&mut conn).await?;

    debug!("Successfully deleted item with id: {}", item_id);
    Ok(())
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
#[instrument(skip(pool))]
pub fn list_items(pool: &DbPool) -> Result<Vec<Item>> {
    debug!("Listing all items");
    
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all items
    let result = items::table
        .load::<Item>(conn)?;
    
    info!("Retrieved {} items", result.len());
    
    // Return the list of items
    Ok(result)
}

/// Retrieves items of a specific type from the database
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
#[instrument(skip(pool), fields(item_type_id = %item_type_id))]
pub fn get_items_by_type(pool: &DbPool, item_type_id: &str) -> Result<Vec<Item>> {
    debug!("Getting items by type");
    
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all items of the specified type
    let result = items::table
        .filter(items::item_type.eq(item_type_id))
        .load::<Item>(conn)?;
    
    info!("Retrieved {} items of type {}", result.len(), item_type_id);
    
    // Return the list of items
    Ok(result)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::tests::setup_test_db;
    use crate::repo::create_item_type;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_create_item() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item of that type
        let title = "Example Item".to_string();
        let data = json!({
            "front": "Hello",
            "back": "World"
        });
        
        let item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();
        
        assert_eq!(item.get_title(), title);
        assert_eq!(item.get_item_type(), item_type.get_id());
        assert_eq!(item.get_data().0, data);
    }
    
    #[tokio::test]
    async fn test_get_item() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item
        let title = "Example Item".to_string();
        let data = json!({
            "front": "Hello",
            "back": "World"
        });
        
        let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();
        
        // Retrieve the item
        let retrieved_item = get_item(&pool, &created_item.get_id()).unwrap().unwrap();
        
        assert_eq!(retrieved_item.get_id(), created_item.get_id());
        assert_eq!(retrieved_item.get_title(), title);
        assert_eq!(retrieved_item.get_item_type(), item_type.get_id());
        assert_eq!(retrieved_item.get_data().0, data);
    }
    
    #[tokio::test]
    async fn test_get_nonexistent_item() {
        let pool = setup_test_db();
        
        // Try to retrieve a non-existent item
        let result = get_item(&pool, "nonexistent-id").unwrap();
        
        assert!(result.is_none());
    }
    
    #[tokio::test]
    async fn test_list_items() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
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
        
        // List all items
        let items = list_items(&pool).unwrap();
        
        // Verify that the list contains the created items
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.get_id() == item1.get_id()));
        assert!(items.iter().any(|i| i.get_id() == item2.get_id()));
    }
    
    #[tokio::test]
    async fn test_get_items_by_type() {
        let pool = setup_test_db();
        
        // Create two item types
        let vocab_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let grammar_type = create_item_type(&pool, "Test Type 2".to_string()).await.unwrap();
        
        // Create items of different types
        let vocab_item = create_item(
            &pool, 
            &vocab_type.get_id(), 
            "Vocab Item".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        let grammar_item = create_item(
            &pool, 
            &grammar_type.get_id(), 
            "Grammar Item".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).await.unwrap();
        
        // Get items by type
        let vocab_items = get_items_by_type(&pool, &vocab_type.get_id()).unwrap();
        let grammar_items = get_items_by_type(&pool, &grammar_type.get_id()).unwrap();
        
        // Verify that the lists contain the correct items
        assert_eq!(vocab_items.len(), 1);
        assert_eq!(vocab_items[0].get_id(), vocab_item.get_id());
        
        assert_eq!(grammar_items.len(), 1);
        assert_eq!(grammar_items[0].get_id(), grammar_item.get_id());
    }
    
    
    #[tokio::test]
    async fn test_create_item_with_data() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item with complex JSON data
        let data = json!({
            "front": {
                "text": "Hello",
                "image_url": "https://example.com/hello.jpg",
                "audio_url": "https://example.com/hello.mp3"
            },
            "back": {
                "text": "World",
                "examples": [
                    "Hello, world!",
                    "Hello there, friend."
                ],
                "notes": "A common greeting."
            }
        });
        
        let item = create_item(&pool, &item_type.get_id(), "Complex Item".to_string(), data.clone()).await.unwrap();
        
        // Retrieve the item
        let retrieved_item = get_item(&pool, &item.get_id()).unwrap().unwrap();
        
        // Verify that the complex data was stored and retrieved correctly
        assert_eq!(retrieved_item.get_data().0, data);
    }
    

    #[tokio::test]
    async fn test_update_item_title() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item
        let title = "Original Title".to_string();
        let data = json!({
            "front": "Hello",
            "back": "World"
        });
        
        let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();
        
        // Update only the title
        let new_title = "Updated Title".to_string();
        let updated_item = update_item(&pool, &created_item.get_id(), Some(new_title.clone()), None).await.unwrap();
        
        // Verify that the title was updated but the data remained the same
        assert_eq!(updated_item.get_title(), new_title);
        assert_eq!(updated_item.get_data().0, data);
        assert_eq!(updated_item.get_id(), created_item.get_id());
        assert_eq!(updated_item.get_item_type(), item_type.get_id());
    }
    

    #[tokio::test]
    async fn test_update_item_data() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item
        let title = "Original Title".to_string();
        let data = json!({
            "front": "Hello",
            "back": "World"
        });
        
        let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();
        
        // Update only the data
        let new_data = json!({
            "front": "Bonjour",
            "back": "Monde"
        });
        
        let updated_item = update_item(&pool, &created_item.get_id(), None, Some(new_data.clone())).await.unwrap();
        
        // Verify that the data was updated but the title remained the same
        assert_eq!(updated_item.get_title(), title);
        assert_eq!(updated_item.get_data().0, new_data);
        assert_eq!(updated_item.get_id(), created_item.get_id());
        assert_eq!(updated_item.get_item_type(), item_type.get_id());
    }
    

    #[tokio::test]
    async fn test_update_item_both_fields() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item
        let title = "Original Title".to_string();
        let data = json!({
            "front": "Hello",
            "back": "World"
        });
        
        let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();
        
        // Update both title and data
        let new_title = "Updated Title".to_string();
        let new_data = json!({
            "front": "Hola",
            "back": "Mundo",
            "notes": "Spanish greeting"
        });
        
        let updated_item = update_item(
            &pool, 
            &created_item.get_id(), 
            Some(new_title.clone()), 
            Some(new_data.clone())
        ).await.unwrap();
        
        // Verify that both title and data were updated
        assert_eq!(updated_item.get_title(), new_title);
        assert_eq!(updated_item.get_data().0, new_data);
        assert_eq!(updated_item.get_id(), created_item.get_id());
        assert_eq!(updated_item.get_item_type(), item_type.get_id());
    }
    

    #[tokio::test]
    async fn test_update_complex_item_data() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item with simple data
        let title = "Complex Item".to_string();
        let data = json!({
            "front": "Hello",
            "back": "World"
        });
        
        let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();
        
        // Update with complex nested JSON data
        let complex_data = json!({
            "front": {
                "text": "Hello",
                "image_url": "https://example.com/hello.jpg",
                "audio_url": "https://example.com/hello.mp3"
            },
            "back": {
                "text": "World",
                "examples": [
                    "Hello, world!",
                    "Hello there, friend."
                ],
                "notes": "A common greeting."
            }
        });
        
        let updated_item = update_item(&pool, &created_item.get_id(), None, Some(complex_data.clone())).await.unwrap();
        
        // Verify that the complex data was stored and retrieved correctly
        assert_eq!(updated_item.get_data().0, complex_data);
    }
    

    #[tokio::test]
    async fn test_update_nonexistent_item() {
        let pool = setup_test_db();
        
        // Try to update a non-existent item
        let result = update_item(
            &pool, 
            "nonexistent-id", 
            Some("New Title".to_string()), 
            Some(json!({"front": "New", "back": "Content"}))
        ).await;
        
        // Verify that the update failed with an error
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }
    
    
    #[tokio::test]
    async fn test_update_with_empty_changes() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item
        let title = "Original Title".to_string();
        let data = json!({
            "front": "Hello",
            "back": "World"
        });
        
        let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();
        
        // Update with no changes (None for both fields)
        // Only the updated_at timestamp should change
        let updated_item = update_item(&pool, &created_item.get_id(), None, None).await.unwrap();
        
        // Verify that the item's content remains unchanged
        assert_eq!(updated_item.get_title(), title);
        assert_eq!(updated_item.get_data().0, data);
        assert_eq!(updated_item.get_id(), created_item.get_id());
        
        // The updated_at timestamp should be different, but we can't easily test for that
        // without mocking time or introducing complex test logic
    }
} 
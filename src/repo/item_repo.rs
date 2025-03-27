use crate::db::DbPool;
use crate::models::{Item, JsonValue};
use crate::schema::items;
use diesel::prelude::*;
use anyhow::Result;

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
    let result = items::table
        .load::<Item>(conn)?;
    
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
pub fn get_items_by_type(pool: &DbPool, item_type_id: &str) -> Result<Vec<Item>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all items of the specified type
    let result = items::table
        .filter(items::item_type.eq(item_type_id))
        .load::<Item>(conn)?;
    
    // Return the list of items
    Ok(result)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::tests::setup_test_db;
    use crate::repo::create_item_type;
    use serde_json::json;
    
    #[test]
    fn test_create_item() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).unwrap();
        
        // Create an item of that type
        let title = "Example Item".to_string();
        let data = json!({
            "front": "Hello",
            "back": "World"
        });
        
        let item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).unwrap();
        
        assert_eq!(item.get_title(), title);
        assert_eq!(item.get_item_type(), item_type.get_id());
        assert_eq!(item.get_data().0, data);
    }
    
    #[test]
    fn test_get_item() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).unwrap();
        
        // Create an item
        let title = "Example Item".to_string();
        let data = json!({
            "front": "Hello",
            "back": "World"
        });
        
        let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).unwrap();
        
        // Retrieve the item
        let retrieved_item = get_item(&pool, &created_item.get_id()).unwrap().unwrap();
        
        assert_eq!(retrieved_item.get_id(), created_item.get_id());
        assert_eq!(retrieved_item.get_title(), title);
        assert_eq!(retrieved_item.get_item_type(), item_type.get_id());
        assert_eq!(retrieved_item.get_data().0, data);
    }
    
    #[test]
    fn test_get_nonexistent_item() {
        let pool = setup_test_db();
        
        // Try to retrieve a non-existent item
        let result = get_item(&pool, "nonexistent-id").unwrap();
        
        assert!(result.is_none());
    }
    
    #[test]
    fn test_list_items() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).unwrap();
        
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).unwrap();
        
        // List all items
        let items = list_items(&pool).unwrap();
        
        // Verify that the list contains the created items
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.get_id() == item1.get_id()));
        assert!(items.iter().any(|i| i.get_id() == item2.get_id()));
    }
    
    #[test]
    fn test_get_items_by_type() {
        let pool = setup_test_db();
        
        // Create two item types
        let vocab_type = create_item_type(&pool, "Test Type 1".to_string()).unwrap();
        let grammar_type = create_item_type(&pool, "Test Type 2".to_string()).unwrap();
        
        // Create items of different types
        let vocab_item = create_item(
            &pool, 
            &vocab_type.get_id(), 
            "Vocab Item".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).unwrap();
        
        let grammar_item = create_item(
            &pool, 
            &grammar_type.get_id(), 
            "Grammar Item".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).unwrap();
        
        // Get items by type
        let vocab_items = get_items_by_type(&pool, &vocab_type.get_id()).unwrap();
        let grammar_items = get_items_by_type(&pool, &grammar_type.get_id()).unwrap();
        
        // Verify that the lists contain the correct items
        assert_eq!(vocab_items.len(), 1);
        assert_eq!(vocab_items[0].get_id(), vocab_item.get_id());
        
        assert_eq!(grammar_items.len(), 1);
        assert_eq!(grammar_items[0].get_id(), grammar_item.get_id());
    }
    
    
    #[test]
    fn test_create_item_with_data() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Type".to_string()).unwrap();
        
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
        
        let item = create_item(&pool, &item_type.get_id(), "Complex Item".to_string(), data.clone()).unwrap();
        
        // Retrieve the item
        let retrieved_item = get_item(&pool, &item.get_id()).unwrap().unwrap();
        
        // Verify that the complex data was stored and retrieved correctly
        assert_eq!(retrieved_item.get_data().0, data);
    }
} 
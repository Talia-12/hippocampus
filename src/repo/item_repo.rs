use crate::db::DbPool;
use crate::models::{Item, JsonValue, Tag};
use crate::schema::{items, item_tags, tags};
use diesel::prelude::*;
use anyhow::{Result, anyhow};

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

/// Add a tag to an item
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `tag_id` - The ID of the tag to add
/// * `item_id` - The ID of the item to tag
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
/// - The item or tag does not exist (this will cause the database to return an error)
pub fn add_tag_to_item(pool: &DbPool, tag_id: &str, item_id: &str) -> Result<()> {
    use crate::models::ItemTag;
    
    let conn = &mut pool.get()?;
    
    // Create the association
    let item_tag = ItemTag::new(item_id.to_string(), tag_id.to_string());
    
    // Check if the association already exists to avoid duplicates
    let exists: bool = item_tags::table
        .filter(
            item_tags::item_id.eq(item_id)
                .and(item_tags::tag_id.eq(tag_id))
        )
        .count()
        .get_result::<i64>(conn)? > 0;
    
    if !exists {
        // Insert the association
        diesel::insert_into(item_tags::table)
            .values(&item_tag)
            .execute(conn)?;
    }
    
    Ok(())
}

/// Remove a tag from an item
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `tag_id` - The ID of the tag to remove
/// * `item_id` - The ID of the item to untag
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
/// - The item or tag does not exist
pub fn remove_tag_from_item(pool: &DbPool, tag_id: &str, item_id: &str) -> Result<()> {
    let conn = &mut pool.get()?;
    
    // Delete the association
    let num_deleted = diesel::delete(
        item_tags::table
            .filter(
                item_tags::item_id.eq(item_id)
                    .and(item_tags::tag_id.eq(tag_id))
            )
    ).execute(conn)?;

    if num_deleted == 0 {
        return Err(anyhow::anyhow!("Tag not found"));
    }
    
    Ok(())
}

/// Lists all tags for a specific item
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to get tags for
///
/// ### Returns
///
/// A Result containing a vector of Tags associated with the item
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn list_tags_for_item(pool: &DbPool, item_id: &str) -> Result<Vec<Tag>> {
    let conn = &mut pool.get()?;
    
    let result = tags::table
        .inner_join(item_tags::table.on(tags::id.eq(item_tags::tag_id)))
        .filter(item_tags::item_id.eq(item_id))
        .select(tags::all_columns)
        .load::<Tag>(conn)?;
    
    Ok(result)
}

// This is declared as a helper since it's also used in this file.
// The full implementation is in tag_repo.rs.
fn get_tag(pool: &DbPool, tag_id: &str) -> Result<Tag> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for the tag with the specified ID
    let result = tags::table
        .find(tag_id)
        .first::<Tag>(conn)
        .map_err(|e| anyhow!("Failed to get tag: {}", e))?;
    
    // Return the tag
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::tests::setup_test_db;
    use crate::repo::{create_item_type, create_tag};
    use serde_json::json;
    
    #[test]
    fn test_create_item() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
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
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
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
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
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
        let vocab_type = create_item_type(&pool, "Test Vocabulary".to_string()).unwrap();
        let grammar_type = create_item_type(&pool, "Test Grammar".to_string()).unwrap();
        
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
    fn test_add_tag_to_item() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Tagged Item".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).unwrap();
        
        // Create a tag
        let tag = create_tag(&pool, "Important".to_string(), true).unwrap();
        
        // Add the tag to the item
        add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).unwrap();
        
        // Get the tags for the item
        let tags = list_tags_for_item(&pool, &item.get_id()).unwrap();
        
        // Verify that the item has the tag
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].get_id(), tag.get_id());
    }
    
    #[test]
    fn test_remove_tag_from_item() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Tagged Item".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).unwrap();
        
        // Create a tag
        let tag = create_tag(&pool, "Important".to_string(), true).unwrap();
        
        // Add the tag to the item
        add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).unwrap();
        
        // Verify that the item has the tag
        let tags_before = list_tags_for_item(&pool, &item.get_id()).unwrap();
        assert_eq!(tags_before.len(), 1);
        
        // Remove the tag from the item
        remove_tag_from_item(&pool, &tag.get_id(), &item.get_id()).unwrap();
        
        // Verify that the item no longer has the tag
        let tags_after = list_tags_for_item(&pool, &item.get_id()).unwrap();
        assert_eq!(tags_after.len(), 0);
    }
    
    #[test]
    fn test_create_item_with_data() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
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
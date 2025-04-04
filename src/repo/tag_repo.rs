use crate::db::{DbPool, ExecuteWithRetry};
use crate::models::Tag;
use crate::schema::{tags, item_tags};
use diesel::prelude::*;
use anyhow::{Result, anyhow};
use tracing::{instrument, debug, info};

/// Creates a new tag in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `name` - The name for the new tag
/// * `visible` - Whether the tag is visible to the user
///
/// ### Returns
///
/// A Result containing the newly created Tag if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
#[instrument(skip(pool), fields(name = %name, visible = %visible))]
pub async fn create_tag(pool: &DbPool, name: String, visible: bool) -> Result<Tag> {
    debug!("Creating new tag");
    
    let conn = &mut pool.get()?;
    
    // Create a new tag with the provided name and visibility
    let new_tag = Tag::new(name, visible);
    
    debug!("Inserting tag into database with id: {}", new_tag.get_id());
    
    // Insert the new tag into the database
    diesel::insert_into(tags::table)
        .values(new_tag.clone())
        .execute_with_retry(conn).await?;
    
    info!("Successfully created tag with id: {}", new_tag.get_id());
    
    // Return the newly created tag
    Ok(new_tag)
}


/// Retrieves a tag from the database by its ID
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
/// - The tag does not exist
#[instrument(skip(pool), fields(tag_id = %tag_id))]
pub fn get_tag(pool: &DbPool, tag_id: &str) -> Result<Tag> {
    debug!("Retrieving tag by id");
    
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for the tag with the specified ID
    let result = tags::table
        .find(tag_id)
        .first::<Tag>(conn)
        .map_err(|e| anyhow!("Failed to get tag: {}", e))?;
    
    debug!("Tag found with id: {}", result.get_id());
    
    // Return the tag
    Ok(result)
}


/// Lists all cards for a card
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to get tags for
///
/// ### Returns
///
/// A Result containing a vector of Tags associated with the card's item
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
/// - The card does not exist
#[instrument(skip(pool), fields(card_id = %card_id))]
pub fn list_tags_for_card(pool: &DbPool, card_id: &str) -> Result<Vec<Tag>> {  
    debug!("Listing tags for card");
    
    // Get the card to find its item_id
    let card = super::get_card(pool, card_id)?.ok_or_else(|| anyhow!("Card not found"))?;
    
    debug!("Card found, looking for tags on item: {}", card.get_item_id());
    
    let conn = &mut pool.get()?;

    // Use the item_id to get tags
    let results = tags::table
        .inner_join(item_tags::table.on(tags::id.eq(item_tags::tag_id)))
        .filter(item_tags::item_id.eq(card.get_item_id()))
        .select(tags::all_columns)
        .load::<Tag>(conn)?;
    
    info!("Retrieved {} tags for card {}", results.len(), card_id);
    
    Ok(results)
}


/// Lists all tags associated with a specific item
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
#[instrument(skip(pool), fields(item_id = %item_id))]
pub fn list_tags_for_item(pool: &DbPool, item_id: &str) -> Result<Vec<Tag>> {
    debug!("Listing tags for item");
    
    let conn = &mut pool.get()?;
    
    // Query for tags associated with the item
    let results = tags::table
        .inner_join(item_tags::table.on(tags::id.eq(item_tags::tag_id)))
        .filter(item_tags::item_id.eq(item_id))
        .select(tags::all_columns)
        .load::<Tag>(conn)?;
    
    info!("Retrieved {} tags for item {}", results.len(), item_id);
    
    Ok(results)
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
#[instrument(skip(pool))]
pub fn list_tags(pool: &DbPool) -> Result<Vec<Tag>> {
    debug!("Listing all tags");
    
    let conn = &mut pool.get()?;
    
    // Query the database for all tags
    let result = tags::table
        .load::<Tag>(conn)?;
    
    info!("Retrieved {} tags", result.len());
    
    // Return the list of tags
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
#[instrument(skip(pool), fields(tag_id = %tag_id, item_id = %item_id))]
pub async fn add_tag_to_item(pool: &DbPool, tag_id: &str, item_id: &str) -> Result<()> {
    debug!("Adding tag to item");
    
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
        debug!("Tag association does not exist, creating it");
        // Insert the association
        diesel::insert_into(item_tags::table)
            .values(item_tag.clone())
            .execute_with_retry(conn).await?;
        
        info!("Successfully added tag {} to item {}", tag_id, item_id);
    } else {
        debug!("Tag association already exists");
    }
    
    Ok(())
}


/// Remove a tag from an item
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `tag_id` - The ID of the tag to remove
/// * `item_id` - The ID of the item to remove the tag from
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
#[instrument(skip(pool), fields(tag_id = %tag_id, item_id = %item_id))]
pub async fn remove_tag_from_item(pool: &DbPool, tag_id: &str, item_id: &str) -> Result<()> {
    debug!("Removing tag from item");
        
    // Make sure the tag exists
    get_tag(pool, tag_id)?;
    
    let conn = &mut pool.get()?;

    // Delete the association
    let rows_deleted = diesel::delete(
        item_tags::table.filter(
            item_tags::item_id.eq(item_id.to_string())
                .and(item_tags::tag_id.eq(tag_id.to_string()))
        )
    ).execute_with_retry(conn).await?;
    
    if rows_deleted == 0 {
        debug!("No tag association found to remove");
        return Err(anyhow!("Tag not found on item"));
    }
    
    info!("Successfully removed tag {} from item {}", tag_id, item_id);
    
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::tests::setup_test_db;

    #[tokio::test]
    async fn test_create_tag() {
        let pool = setup_test_db();
        
        let name = "Important".to_string();
        let visible = true;
        
        let tag = create_tag(&pool, name.clone(), visible).await.unwrap();
        
        assert_eq!(tag.get_name(), name);
        assert_eq!(tag.get_visible(), visible);
    }
    
    #[tokio::test]
    async fn test_get_tag() {
        let pool = setup_test_db();
        
        let name = "Important".to_string();
        let visible = true;
        
        let created_tag = create_tag(&pool, name.clone(), visible).await.unwrap();
        let retrieved_tag = get_tag(&pool, &created_tag.get_id()).unwrap();
        
        assert_eq!(retrieved_tag.get_name(), name);
        assert_eq!(retrieved_tag.get_id(), created_tag.get_id());
        assert_eq!(retrieved_tag.get_visible(), visible);
    }
    
    #[tokio::test]
    async fn test_list_tags() {
        let pool = setup_test_db();
        
        // Create some tags
        let tag1 = create_tag(&pool, "Important".to_string(), true).await.unwrap();
        let tag2 = create_tag(&pool, "Difficult".to_string(), false).await.unwrap();
        
        // List all tags
        let tags = list_tags(&pool).unwrap();
        
        // Verify that the list contains the created tags
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().any(|t| t.get_id() == tag1.get_id()));
        assert!(tags.iter().any(|t| t.get_id() == tag2.get_id()));
    }
    
    #[tokio::test]
    async fn test_tag_error_handling() {
        let pool = setup_test_db();
        
        // Try to get a non-existent tag
        let result = get_tag(&pool, "nonexistent-id");
        
        // Verify that we got an error
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Failed to get tag"));
    }
    

    #[tokio::test]
    async fn test_list_tags_for_item() {
        let pool = setup_test_db();
        
        // Create necessary objects
        let item_type = crate::repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create some items
        let item1 = crate::repo::create_item(
            &pool,
            &item_type.get_id(),
            "Item 1".to_string(),
            serde_json::json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        let item2 = crate::repo::create_item(
            &pool,
            &item_type.get_id(),
            "Item 2".to_string(),
            serde_json::json!({"front": "Goodbye", "back": "World"}),
        ).await.unwrap();
        
        // Create some tags
        let tag1 = create_tag(&pool, "Important".to_string(), true).await.unwrap();
        let tag2 = create_tag(&pool, "Difficult".to_string(), false).await.unwrap();
        
        // Add tags to item1
        add_tag_to_item(&pool, &tag1.get_id(), &item1.get_id()).await.unwrap();
        add_tag_to_item(&pool, &tag2.get_id(), &item1.get_id()).await.unwrap();
        
        // Add only tag1 to item2
        add_tag_to_item(&pool, &tag1.get_id(), &item2.get_id()).await.unwrap();
        
        // Test list_tags_for_item with item1
        let item1_tags = list_tags_for_item(&pool, &item1.get_id()).unwrap();
        assert_eq!(item1_tags.len(), 2);
        assert!(item1_tags.iter().any(|t| t.get_id() == tag1.get_id()));
        assert!(item1_tags.iter().any(|t| t.get_id() == tag2.get_id()));
        
        // Test list_tags_for_item with item2
        let item2_tags = list_tags_for_item(&pool, &item2.get_id()).unwrap();
        assert_eq!(item2_tags.len(), 1);
        assert!(item2_tags.iter().any(|t| t.get_id() == tag1.get_id()));
        assert!(!item2_tags.iter().any(|t| t.get_id() == tag2.get_id()));
    }
    

    #[tokio::test]
    async fn test_list_tags_for_card() {
        let pool = setup_test_db();
        
        // Create necessary objects
        let item_type = crate::repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item
        let item = crate::repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            serde_json::json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Get the card created for the item
        let cards = crate::repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
        let card = &cards[0];
        
        // Create some tags
        let tag1 = create_tag(&pool, "Important".to_string(), true).await.unwrap();
        let tag2 = create_tag(&pool, "Difficult".to_string(), false).await.unwrap();
        
        // Add tags to the item
        add_tag_to_item(&pool, &tag1.get_id(), &item.get_id()).await.unwrap();
        add_tag_to_item(&pool, &tag2.get_id(), &item.get_id()).await.unwrap();
        
        // Test list_tags_for_card
        let card_tags = list_tags_for_card(&pool, &card.get_id()).unwrap();
        assert_eq!(card_tags.len(), 2);
        assert!(card_tags.iter().any(|t| t.get_id() == tag1.get_id()));
        assert!(card_tags.iter().any(|t| t.get_id() == tag2.get_id()));
    }



    #[tokio::test]
    async fn test_add_tag_to_item() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = crate::repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item
        let item = crate::repo::create_item(
            &pool, 
            &item_type.get_id(), 
            "Tagged Item".to_string(), 
            serde_json::json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        // Create a tag
        let tag = create_tag(&pool, "Important".to_string(), true).await.unwrap();
        
        // Add the tag to the item
        add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();
        
        // Get the tags for the item
        let tags = list_tags_for_item(&pool, &item.get_id()).unwrap();
        
        // Verify that the item has the tag
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].get_id(), tag.get_id());
    }
    

    #[tokio::test]
    async fn test_remove_tag_from_item() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = crate::repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item
        let item = crate::repo::create_item(
            &pool, 
            &item_type.get_id(), 
            "Tagged Item".to_string(), 
            serde_json::json!({"front": "F1", "back": "B1"})
        ).await.unwrap();
        
        // Create a tag
        let tag = create_tag(&pool, "Important".to_string(), true).await.unwrap();
        
        // Add the tag to the item
        add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();
        
        // Verify that the item has the tag
        let tags_before = list_tags_for_item(&pool, &item.get_id()).unwrap();
        assert_eq!(tags_before.len(), 1);
        
        // Remove the tag from the item
        remove_tag_from_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();
        
        // Verify that the item no longer has the tag
        let tags_after = list_tags_for_item(&pool, &item.get_id()).unwrap();
        assert_eq!(tags_after.len(), 0);
    }
} 
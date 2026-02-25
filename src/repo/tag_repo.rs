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
mod tests;
#[cfg(test)]
mod prop_tests;
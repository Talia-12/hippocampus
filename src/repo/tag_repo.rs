use crate::db::DbPool;
use crate::models::Tag;
use crate::schema::tags;
use diesel::prelude::*;
use anyhow::{Result, anyhow};

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
pub fn create_tag(pool: &DbPool, name: String, visible: bool) -> Result<Tag> {
    let conn = &mut pool.get()?;
    
    // Create a new tag with the provided name and visibility
    let new_tag = Tag::new(name, visible);
    
    // Insert the new tag into the database
    diesel::insert_into(tags::table)
        .values(&new_tag)
        .execute(conn)?;
    
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
pub fn get_tag(pool: &DbPool, tag_id: &str) -> Result<Tag> {
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
    let conn = &mut pool.get()?;
    
    // Query the database for all tags
    let result = tags::table
        .load::<Tag>(conn)?;
    
    // Return the list of tags
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::tests::setup_test_db;
    
    #[test]
    fn test_create_tag() {
        let pool = setup_test_db();
        let name = "Important".to_string();
        let visible = true;
        
        let tag = create_tag(&pool, name.clone(), visible).unwrap();
        
        assert_eq!(tag.get_name(), name);
        assert_eq!(tag.get_visible(), visible);
    }
    
    #[test]
    fn test_get_tag() {
        let pool = setup_test_db();
        let name = "Important".to_string();
        let visible = true;
        
        let created_tag = create_tag(&pool, name.clone(), visible).unwrap();
        let retrieved_tag = get_tag(&pool, &created_tag.get_id()).unwrap();
        
        assert_eq!(retrieved_tag.get_name(), name);
        assert_eq!(retrieved_tag.get_id(), created_tag.get_id());
        assert_eq!(retrieved_tag.get_visible(), visible);
    }
    
    #[test]
    fn test_list_tags() {
        let pool = setup_test_db();
        
        // Create some tags
        let tag1 = create_tag(&pool, "Important".to_string(), true).unwrap();
        let tag2 = create_tag(&pool, "Difficult".to_string(), false).unwrap();
        
        // List all tags
        let tags = list_tags(&pool).unwrap();
        
        // Verify that the list contains the created tags
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().any(|t| t.get_id() == tag1.get_id()));
        assert!(tags.iter().any(|t| t.get_id() == tag2.get_id()));
    }
    
    #[test]
    fn test_tag_error_handling() {
        let pool = setup_test_db();
        
        // Try to get a non-existent tag
        let result = get_tag(&pool, "nonexistent-id");
        
        // Verify that we got an error
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Failed to get tag"));
    }
} 
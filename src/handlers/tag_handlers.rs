use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use tracing::{instrument, debug, info};

use crate::db::DbPool;
use crate::dto::CreateTagDto;
use crate::errors::ApiError;
use crate::models::Tag;
use crate::repo;

/// Handler for creating a new tag
///
/// This function handles POST requests to `/tags`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `payload` - The request payload containing the tag name and visibility
///
/// ### Returns
///
/// The newly created tag as JSON
#[instrument(skip(pool), fields(name = %payload.name, visible = %payload.visible))]
pub async fn create_tag_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateTagDto>,
) -> Result<Json<Tag>, ApiError> {
    info!("Creating new tag");
    
    // Call the repository function to create the tag
    let tag = repo::create_tag(&pool, payload.name, payload.visible).await
        .map_err(ApiError::Database)?;

    info!("Successfully created tag with id: {}", tag.get_id());
    
    // Return the created tag as JSON
    Ok(Json(tag))
}

/// Handler for listing all tags
///
/// This function handles GET requests to `/tags`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
///
/// ### Returns
///
/// A list of all tags as JSON
#[instrument(skip(pool))]
pub async fn list_tags_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<Vec<Tag>>, ApiError> {
    debug!("Listing all tags");
    
    // Call the repository function to list all tags
    let tags = repo::list_tags(&pool)
        .map_err(ApiError::Database)?;
    
    info!("Retrieved {} tags", tags.len());
    
    // Return the list of tags as JSON
    Ok(Json(tags))
}

/// Handler for adding a tag to an item
///
/// This function handles POST requests to `/items/{item_id}/tags/{tag_id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `params` - The item ID and tag ID extracted from the URL path
///
/// ### Returns
///
/// A 204 No Content response if successful
#[instrument(skip(pool), fields(item_id = %item_id, tag_id = %tag_id))]
pub async fn add_tag_to_item_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item ID and tag ID from the URL path
    Path((item_id, tag_id)): Path<(String, String)>,
) -> Result<(), ApiError> {
    info!("Adding tag to item");
    
    // Call the repository function to add the tag to the item
    match repo::add_tag_to_item(&pool, &tag_id, &item_id).await {
        Ok(_) => {
            info!("Successfully added tag {} to item {}", tag_id, item_id);
            Ok(())
        },
        Err(e) => {
            // Check if the error is due to item or tag not found
            if e.to_string().contains("FOREIGN KEY constraint failed") {
                debug!("Failed to add tag: item or tag not found");
                Err(ApiError::NotFound)
            } else {
                Err(ApiError::Database(e))
            }
        }
    }
}

/// Handler for removing a tag from an item
///
/// This function handles DELETE requests to `/items/{item_id}/tags/{tag_id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `params` - The item ID and tag ID extracted from the URL path
///
/// ### Returns
///
/// A 204 No Content response if successful
#[instrument(skip(pool), fields(item_id = %item_id, tag_id = %tag_id))]
pub async fn remove_tag_from_item_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item ID and tag ID from the URL path
    Path((item_id, tag_id)): Path<(String, String)>,
) -> Result<(), ApiError> {
    info!("Removing tag from item");
    
    // Call the repository function to remove the tag from the item
    match repo::remove_tag_from_item(&pool, &tag_id, &item_id).await {
        Ok(_) => {
            info!("Successfully removed tag {} from item {}", tag_id, item_id);
            Ok(())
        },
        Err(e) => {
            // Check if the error is due to item or tag not found
            if e.to_string().contains("not found") {
                debug!("Failed to remove tag: item or tag not found");
                Err(ApiError::NotFound)
            } else {
                Err(ApiError::Database(e))    
            }
        }
    }
}

/// Handler for listing all tags for a card
///
/// This function handles GET requests to `/cards/{card_id}/tags`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `card_id` - The ID of the card to get tags for
///
/// ### Returns
///
/// A list of tags for the specified card as JSON
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn list_tags_for_card_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the card ID from the URL path
    Path(card_id): Path<String>,
) -> Result<Json<Vec<Tag>>, ApiError> {
    debug!("Listing tags for card");
    
    // Call the repository function to list tags for the card
    match crate::repo::list_tags_for_card(&pool, &card_id) {
        Ok(tags) => {
            info!("Retrieved {} tags for card {}", tags.len(), card_id);
            Ok(Json(tags))
        },
        Err(e) => {
            // Check if the error is due to card not found
            if e.to_string().contains("Card not found") {
                debug!("Card not found");
                Err(ApiError::NotFound)
            } else {
                Err(ApiError::Database(e))
            }
        }
    }
}

/// Handler for listing all tags for an item
///
/// This function handles GET requests to `/items/{item_id}/tags`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The ID of the item to get tags for
///
/// ### Returns
///
/// A list of tags for the specified item as JSON
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn list_tags_for_item_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item ID from the URL path
    Path(item_id): Path<String>,
) -> Result<Json<Vec<Tag>>, ApiError> {
    debug!("Listing tags for item");
    
    // First check if the item exists
    repo::get_item(&pool, &item_id)
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound)?;
    
    // Call the repository function to list tags for the item
    let tags = repo::list_tags_for_item(&pool, &item_id)
        .map_err(ApiError::Database)?;
    
    info!("Retrieved {} tags for item {}", tags.len(), item_id);
    
    // Return the list of tags as JSON
    Ok(Json(tags))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::setup_test_db;
    use crate::repo;
    use axum::extract::Path;
    use serde_json::json;
    

    #[tokio::test]
    async fn test_create_tag_handler() {
        let pool = setup_test_db();
        
        // Create a test payload
        let payload = CreateTagDto {
            name: "Important".to_string(),
            visible: true,
        };
        
        // Call the handler
        let result = create_tag_handler(
            State(pool.clone()),
            Json(payload),
        ).await.unwrap();
        
        // Check the result
        let tag = result.0;
        assert_eq!(tag.get_name(), "Important");
        assert_eq!(tag.get_visible(), true);
    }
    

    #[tokio::test]
    async fn test_list_tags_handler() {
        let pool = setup_test_db();
        
        // Create some tags
        let tag1 = repo::create_tag(&pool, "Important".to_string(), true).await.unwrap();
        let tag2 = repo::create_tag(&pool, "Difficult".to_string(), false).await.unwrap();
        
        // Call the handler
        let result = list_tags_handler(
            State(pool.clone()),
        ).await.unwrap();
        
        // Check the result
        let tags = result.0;
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().any(|t| t.get_id() == tag1.get_id()));
        assert!(tags.iter().any(|t| t.get_id() == tag2.get_id()));
    }
    

    #[tokio::test]
    async fn test_add_tag_to_item_handler() {
        let pool = setup_test_db();
        
        // Create test data
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Create a tag
        let tag = repo::create_tag(&pool, "Important".to_string(), true).await.unwrap();
        
        // Call the handler
        let result = add_tag_to_item_handler(
            State(pool.clone()),
            Path((item.get_id(), tag.get_id())),
        ).await;
        
        // Check that the operation succeeded
        assert!(result.is_ok());
        
        // Verify that the tag was added to the item
        let tags = repo::list_tags_for_item(&pool, &item.get_id()).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].get_id(), tag.get_id());
    }
    

    #[tokio::test]
    async fn test_add_tag_to_item_handler_not_found() {
        let pool = setup_test_db();
        
        // Call the handler with non-existent IDs
        let result = add_tag_to_item_handler(
            State(pool.clone()),
            Path(("nonexistent-item".to_string(), "nonexistent-tag".to_string())),
        ).await;
        
        // Check that we got a NotFound error
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ApiError::NotFound), "Expected NotFound error, got {:?}", err);
    }
    

    #[tokio::test]
    async fn test_remove_tag_from_item_handler() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        // Create an item
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Create a tag
        let tag = repo::create_tag(&pool, "Important".to_string(), true).await.unwrap();
        
        // Add the tag to the item
        repo::add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();
        
        // Verify the tag was added
        let tags_before = repo::list_tags_for_item(&pool, &item.get_id()).unwrap();
        assert_eq!(tags_before.len(), 1);
        
        // Call the handler to remove the tag
        let result = remove_tag_from_item_handler(
            State(pool.clone()),
            Path((item.get_id(), tag.get_id())),
        ).await;
        
        // Check that the operation succeeded
        assert!(result.is_ok());
        
        // Verify that the tag was removed
        let tags_after = repo::list_tags_for_item(&pool, &item.get_id()).unwrap();
        assert_eq!(tags_after.len(), 0);
    }
    

    #[tokio::test]
    async fn test_remove_tag_from_item_handler_not_found() {
        let pool = setup_test_db();
        
        // Call the handler with non-existent IDs
        let result = remove_tag_from_item_handler(
            State(pool.clone()),
            Path(("nonexistent-item".to_string(), "nonexistent-tag".to_string())),
        ).await;
        
        // Check that we got a NotFound error
        assert!(result.is_err());
        
        let err = result.unwrap_err();
        assert!(matches!(err, ApiError::NotFound), "Expected NotFound error, got {:?}", err);
    }
    

    #[tokio::test]
    async fn test_list_tags_for_item_handler() {
        let pool = setup_test_db();
        
        // Create test data
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        let tag1 = repo::create_tag(&pool, "Important".to_string(), true).await.unwrap();
        let tag2 = repo::create_tag(&pool, "Difficult".to_string(), false).await.unwrap();
        
        // Add the tags to the item
        repo::add_tag_to_item(&pool, &tag1.get_id(), &item.get_id()).await.unwrap();
        repo::add_tag_to_item(&pool, &tag2.get_id(), &item.get_id()).await.unwrap();
        
        // Call the handler
        let result = list_tags_for_item_handler(
            State(pool.clone()),
            Path(item.get_id()),
        ).await.unwrap();
        
        // Check the result
        let tags = result.0;
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().any(|t| t.get_id() == tag1.get_id()));
        assert!(tags.iter().any(|t| t.get_id() == tag2.get_id()));
    }
    

    #[tokio::test]
    async fn test_list_tags_for_item_handler_not_found() {
        let pool = setup_test_db();
        
        // Call the handler with a non-existent item ID
        let result = list_tags_for_item_handler(
            State(pool.clone()),
            Path("nonexistent".to_string()),
        ).await;
        
        // Check that we got a NotFound error
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ApiError::NotFound), "Expected NotFound error, got {:?}", err);
    }
    
    #[tokio::test]
    async fn test_list_tags_for_card_handler() {
        let pool = setup_test_db();
        
        // Create test data
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Get the card created for the item
        let cards = repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
        let card = &cards[0];
        
        // Create some tags
        let tag1 = repo::create_tag(&pool, "Important".to_string(), true).await.unwrap();
        let tag2 = repo::create_tag(&pool, "Difficult".to_string(), false).await.unwrap();
        
        // Add tags to the item
        repo::add_tag_to_item(&pool, &tag1.get_id(), &item.get_id()).await.unwrap();
        repo::add_tag_to_item(&pool, &tag2.get_id(), &item.get_id()).await.unwrap();
        
        // Call the handler
        let result = list_tags_for_card_handler(
            State(pool.clone()),
            Path(card.get_id()),
        ).await.unwrap();
        
        // Check the result
        let tags = result.0;
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().any(|t| t.get_id() == tag1.get_id()));
        assert!(tags.iter().any(|t| t.get_id() == tag2.get_id()));
    }
    
    #[tokio::test]
    async fn test_list_tags_for_card_handler_not_found() {
        let pool = setup_test_db();
        
        // Call the handler with a non-existent card ID
        let result = list_tags_for_card_handler(
            State(pool.clone()),
            Path("nonexistent".to_string()),
        ).await;
        
        // Check that we got a NotFound error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound));
    }
} 

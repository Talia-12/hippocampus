use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use tracing::{instrument, debug, info};

use crate::db::DbPool;
use crate::dto::CreateItemDto;
use crate::errors::ApiError;
use crate::models::Item;
use crate::repo;

/// Handler for creating a new item
///
/// This function handles POST requests to `/items`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `payload` - The request payload containing the item title
///
/// ### Returns
///
/// The newly created item as JSON
#[instrument(skip(pool, payload), fields(item_type_id = %payload.item_type_id, title = %payload.title))]
pub async fn create_item_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateItemDto>,
) -> Result<Json<Item>, ApiError> {
    info!("Creating new item");
    
    // Call the repository function to create the item
    let item = repo::create_item(&pool, &payload.item_type_id, payload.title, payload.item_data).await
        .map_err(ApiError::Database)?;
    // TODO: make unique constraint errors map to an ApiError duplicate

    info!("Successfully created item with id: {}", item.get_id());
    
    // Return the created item as JSON
    Ok(Json(item))
}

/// Handler for retrieving a specific item
///
/// This function handles GET requests to `/items/{id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The ID of the item to retrieve, extracted from the URL path
///
/// ### Returns
///
/// The requested item as JSON, or null if not found
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn get_item_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item ID from the URL path
    Path(item_id): Path<String>,
) -> Result<Json<Option<Item>>, ApiError> {
    debug!("Retrieving item");
    
    // Call the repository function to get the item
    let item = repo::get_item(&pool, &item_id)
        .map_err(ApiError::Database)?;
    
    if let Some(ref item) = item {
        debug!("Item found with id: {}", item.get_id());
    } else {
        debug!("Item not found");
    }
    
    // Return the item (or None) as JSON
    Ok(Json(item))
}

/// Handler for listing all items
///
/// This function handles GET requests to `/items`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
///
/// ### Returns
///
/// A list of all items as JSON
#[instrument(skip(pool))]
pub async fn list_items_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<Vec<Item>>, ApiError> {
    debug!("Listing all items");
    
    // Call the repository function to list all items
    let items = repo::list_items(&pool)
        .map_err(ApiError::Database)?;
    
    info!("Retrieved {} items", items.len());
    
    // Return the list of items as JSON
    Ok(Json(items))
}

/// Handler for listing items by item type
///
/// This function handles GET requests to `/item-types/{id}/items`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `id` - The ID of the item type to filter by
///
/// ### Returns
///
/// A list of items with the specified item type as JSON
#[instrument(skip(pool), fields(item_type_id = %item_type_id))]
pub async fn list_items_by_item_type_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item type ID from the URL path
    Path(item_type_id): Path<String>,
) -> Result<Json<Vec<Item>>, ApiError> {
    debug!("Listing items by item type");
    
    // Verify that the item type exists
    let item_type = repo::get_item_type(&pool, &item_type_id)
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound)?;
    
    // Call the repository function to list items by type
    let items = repo::get_items_by_type(&pool, &item_type.get_id())
        .map_err(ApiError::Database)?;
    
    info!("Retrieved {} items for item type {}", items.len(), item_type_id);
    
    // Return the list of items as JSON
    Ok(Json(items))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo;
    use crate::tests::setup_test_db;
    use axum::extract::Path;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_create_item_handler() {
        let pool = setup_test_db();
        
        // Create an item type first
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create a test payload
        let payload = CreateItemDto {
            item_type_id: item_type.get_id(),
            title: "Test Item".to_string(),
            item_data: json!({
                "front": "Hello",
                "back": "World"
            }),
            priority: 0.5,
        };
        
        // Call the handler
        let result = create_item_handler(
            State(pool.clone()),
            Json(payload),
        ).await.unwrap();
        
        // Check the result
        let item = result.0;
        assert_eq!(item.get_title(), "Test Item");
        assert_eq!(item.get_item_type(), item_type.get_id());
    }
    
    #[tokio::test]
    async fn test_list_items_handler() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create some items
        let item1 = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Item 1".to_string(),
            json!({"front": "F1", "back": "B1"}),
        ).await.unwrap();
        
        let item2 = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Item 2".to_string(),
            json!({"front": "F2", "back": "B2"}),
        ).await.unwrap();
        
        // Call the handler
        let result = list_items_handler(
            State(pool.clone()),
        ).await.unwrap();
        
        // Check the result
        let items = result.0;
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.get_id() == item1.get_id()));
        assert!(items.iter().any(|i| i.get_id() == item2.get_id()));
    }
    
    #[tokio::test]
    async fn test_get_item_handler() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Create an item
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Call the handler
        let result = get_item_handler(
            State(pool.clone()),
            Path(item.get_id()),
        ).await.unwrap();
        
        // Check the result
        let retrieved_item = result.0.unwrap();
        assert_eq!(retrieved_item.get_id(), item.get_id());
        assert_eq!(retrieved_item.get_title(), "Test Item");
    }
    
    #[tokio::test]
    async fn test_list_items_by_item_type_handler() {
        let pool = setup_test_db();
        
        // Create two item types
        let type1 = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let type2 = repo::create_item_type(&pool, "Test Type 2".to_string()).await.unwrap();
        
        // Create items of different types
        let type1_item1 = repo::create_item(
            &pool,
            &type1.get_id(),
            "Type 1 Item 1".to_string(),
            json!({"front": "F1", "back": "B1"}),
        ).await.unwrap();
        
        let type1_item2 = repo::create_item(
            &pool,
            &type1.get_id(),
            "Type 1 Item 2".to_string(),
            json!({"front": "F2", "back": "B2"}),
        ).await.unwrap();
        
        let type2_item = repo::create_item(
            &pool,
            &type2.get_id(),
            "Type 2 Item".to_string(),
            json!({"front": "F3", "back": "B3"}),
        ).await.unwrap();
        
        // Call the handler for vocabulary items
        let result = list_items_by_item_type_handler(
            State(pool.clone()),
            Path(type1.get_id()),
        ).await.unwrap();
        
        // Check the result
        let items = result.0;
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.get_id() == type1_item1.get_id()));
        assert!(items.iter().any(|i| i.get_id() == type1_item2.get_id()));
        assert!(!items.iter().any(|i| i.get_id() == type2_item.get_id()));
    }
    
    #[tokio::test]
    async fn test_list_items_by_item_type_handler_not_found() {
        let pool = setup_test_db();
        
        // Call the handler with a non-existent item type ID
        let result = list_items_by_item_type_handler(
            State(pool.clone()),
            Path("nonexistent".to_string()),
        ).await;
        
        // Check that we got a NotFound error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound));
    }
}

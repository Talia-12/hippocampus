use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use tracing::{instrument, debug, info};

use crate::db::DbPool;
use crate::dto::CreateItemTypeDto;
use crate::errors::ApiError;
use crate::models::ItemType;
use crate::repo;

/// Handler for creating a new item type
///
/// This function handles POST requests to `/item-types`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `payload` - The request payload containing the item type name
///
/// ### Returns
///
/// The newly created item type as JSON
#[instrument(skip(pool), fields(name = %payload.name))]
pub async fn create_item_type_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateItemTypeDto>,
) -> Result<Json<ItemType>, ApiError> {
    info!("Creating new item type");
    
    // Call the repository function to create the item type
    let item_type = repo::create_item_type(&pool, payload.name).await
        .map_err(ApiError::Database)?;

    info!("Successfully created item type with id: {}", item_type.get_id());
    
    // Return the created item type as JSON
    Ok(Json(item_type))
}

/// Handler for retrieving a specific item type
///
/// This function handles GET requests to `/item-types/{id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `id` - The ID of the item type to retrieve, extracted from the URL path
///
/// ### Returns
///
/// The requested item type as JSON, or null if not found
#[instrument(skip(pool), fields(item_type_id = %item_type_id))]
pub async fn get_item_type_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item type ID from the URL path
    Path(item_type_id): Path<String>,
) -> Result<Json<ItemType>, ApiError> {
    debug!("Retrieving item type");
    
    // Call the repository function to get the item type
    let item_type = repo::get_item_type(&pool, &item_type_id)
        .map_err(ApiError::Database)?;
    
    // Return a NotFound error if the item type doesn't exist
    match item_type {
        Some(item_type) => {
            debug!("Item type found with id: {}", item_type.get_id());
            Ok(Json(item_type))
        },
        None => {
            debug!("Item type not found");
            Err(ApiError::NotFound)
        }
    }
}

/// Handler for listing all item types
///
/// This function handles GET requests to `/item-types`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
///
/// ### Returns
///
/// A list of all item types as JSON
#[instrument(skip(pool))]
pub async fn list_item_types_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<Vec<ItemType>>, ApiError> {
    debug!("Listing all item types");
    
    // Call the repository function to list all item types
    let item_types = repo::list_item_types(&pool)
        .map_err(ApiError::Database)?;
    
    info!("Retrieved {} item types", item_types.len());
    
    // Return the list of item types as JSON
    Ok(Json(item_types))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    
    #[tokio::test]
    async fn test_create_item_type_handler() {
        let pool = setup_test_db();
        
        // Create a payload
        let payload = CreateItemTypeDto {
            name: "Type 1".to_string(),
        };
        
        // Call the handler
        let result = create_item_type_handler(
            State(pool.clone()),
            Json(payload),
        ).await.unwrap();
        
        // Check the result
        let item_type = result.0;
        assert_eq!(item_type.get_name(), "Type 1");
    }
    
    #[tokio::test]
    async fn test_get_item_type_handler() {
        let pool = setup_test_db();
        
        // Create an item type to get
        let created_item_type = repo::create_item_type(&pool, "Type 1".to_string()).await.unwrap();
        
        // Call the handler
        let result = get_item_type_handler(
            State(pool.clone()),
            Path(created_item_type.get_id()),
        ).await.unwrap();
        
        // Check the result
        let retrieved_item_type = result.0;
        assert_eq!(retrieved_item_type.get_name(), "Type 1");
    }
    
    #[tokio::test]
    async fn test_get_item_type_handler_not_found() {
        let pool = setup_test_db();
        
        // Call the handler with a non-existent ID
        let result = get_item_type_handler(
            State(pool.clone()),
            Path("nonexistent".to_string()),
        ).await.unwrap_err();
        
        // Check that we got a NotFound error
        assert!(matches!(result, ApiError::NotFound));
    }
    
    #[tokio::test]
    async fn test_list_item_types_handler() {
        let pool = setup_test_db();
        
        // Create some item types
        let item_type1 = repo::create_item_type(&pool, "Type 1".to_string()).await.unwrap();
        let item_type2 = repo::create_item_type(&pool, "Type 2".to_string()).await.unwrap();
        
        // Call the handler
        let result = list_item_types_handler(
            State(pool.clone()),
        ).await.unwrap();
        
        // Check the result
        let item_types = result.0;
        assert_eq!(item_types.len(), 2);
        assert!(item_types.iter().any(|it| it.get_id() == item_type1.get_id()));
        assert!(item_types.iter().any(|it| it.get_id() == item_type2.get_id()));
    }
}

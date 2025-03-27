use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

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
pub async fn create_item_type_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateItemTypeDto>,
) -> Result<Json<ItemType>, ApiError> {
    // Call the repository function to create the item type
    let item_type = repo::create_item_type(&pool, payload.name)
        .map_err(ApiError::Database)?;

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
pub async fn get_item_type_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item type ID from the URL path
    Path(id): Path<String>,
) -> Result<Json<Option<ItemType>>, ApiError> {
    // Call the repository function to get the item type
    let item_type = repo::get_item_type(&pool, &id)
        .map_err(ApiError::Database)?;
    
    // Return the item type (or None) as JSON
    Ok(Json(item_type))
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
pub async fn list_item_types_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<Vec<ItemType>>, ApiError> {
    // Call the repository function to list all item types
    let item_types = repo::list_item_types(&pool)
        .map_err(ApiError::Database)?;
    
    // Return the list of item types as JSON
    Ok(Json(item_types))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::setup_test_db;
    
    #[tokio::test]
    async fn test_create_item_type_handler() {
        let pool = setup_test_db();
        
        // Create a test payload
        let payload = CreateItemTypeDto {
            name: "Vocabulary".to_string(),
        };
        
        // Call the handler
        let result = create_item_type_handler(
            State(pool.clone()),
            Json(payload),
        ).await.unwrap();
        
        // Check the result
        let item_type = result.0;
        assert_eq!(item_type.get_name(), "Vocabulary");
    }
    
    #[tokio::test]
    async fn test_get_item_type_handler() {
        let pool = setup_test_db();
        
        // Create an item type
        let created_item_type = repo::create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
        // Call the handler
        let result = get_item_type_handler(
            State(pool.clone()),
            Path(created_item_type.get_id()),
        ).await.unwrap();
        
        // Check the result
        let retrieved_item_type = result.0.unwrap();
        assert_eq!(retrieved_item_type.get_id(), created_item_type.get_id());
        assert_eq!(retrieved_item_type.get_name(), "Vocabulary");
    }
    
    #[tokio::test]
    async fn test_get_item_type_handler_not_found() {
        let pool = setup_test_db();
        
        // Call the handler with a non-existent ID
        let result = get_item_type_handler(
            State(pool.clone()),
            Path("nonexistent".to_string()),
        ).await.unwrap();
        
        // Check that we got None
        assert!(result.0.is_none());
    }
    
    #[tokio::test]
    async fn test_list_item_types_handler() {
        let pool = setup_test_db();
        
        // Create some item types
        let item_type1 = repo::create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        let item_type2 = repo::create_item_type(&pool, "Grammar".to_string()).unwrap();
        
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
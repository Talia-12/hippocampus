use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

use crate::db::DbPool;
use crate::dto::{CreateCardDto, GetQueryDto};
use crate::errors::ApiError;
use crate::models::Card;
use crate::repo;

/// Handler for creating a new card for an item
///
/// This function handles POST requests to `/items/{item_id}/cards`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The ID of the item to create a card for
/// * `payload` - The request payload containing the card creation data
///
/// ### Returns
///
/// The newly created card as JSON
pub async fn create_card_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item ID from the URL path
    Path(item_id): Path<String>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateCardDto>,
) -> Result<Json<Card>, ApiError> {
    // First check if the item exists
    let item = repo::get_item(&pool, &item_id)
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound)?;
    
    // Call the repository function to create the card
    let card = repo::create_card(&pool, &item.get_id(), payload.card_index, payload.priority)
        .map_err(ApiError::Database)?;

    // Return the created card as JSON
    Ok(Json(card))
}


/// Handler for retrieving a specific card
///
/// This function handles GET requests to `/cards/{id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `id` - The ID of the card to retrieve, extracted from the URL path
///
/// ### Returns
///
/// The requested card as JSON, or null if not found
pub async fn get_card_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the card ID from the URL path
    Path(id): Path<String>,
) -> Result<Json<Option<Card>>, ApiError> {
    // Call the repository function to get the card
    let card = repo::get_card(&pool, &id)
        .map_err(ApiError::Database)?;
    
    // Return the card (or None) as JSON
    Ok(Json(card))
}


/// Handler for listing all cards with optional filtering
///
/// This function handles GET requests to `/cards`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `query` - Query parameters for filtering the results
///
/// ### Returns
///
/// A list of cards matching the filter criteria as JSON
pub async fn list_cards_handler(
    // Extract the database connection pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract and parse query parameters
    Query(query): Query<GetQueryDto>,
) -> Result<Json<Vec<Card>>, ApiError> {
    // Call the repository function to list cards with the specified filters
    let cards = repo::list_cards_with_filters(&pool, &query)
        .map_err(ApiError::Database)?;
    
    // Return the list of cards as JSON
    Ok(Json(cards))
}

/// Handler for listing cards for a specific item
///
/// This function handles GET requests to `/items/{item_id}/cards`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The ID of the item to get cards for
///
/// ### Returns
///
/// A list of cards for the specified item as JSON
pub async fn list_cards_by_item_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item ID from the URL path
    Path(item_id): Path<String>,
) -> Result<Json<Vec<Card>>, ApiError> {
    // First check if the item exists
    repo::get_item(&pool, &item_id)
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound)?;
    
    // Call the repository function to get all cards for the item
    let cards = repo::get_cards_for_item(&pool, &item_id)
        .map_err(ApiError::Database)?;
    
    // Return the list of cards as JSON
    Ok(Json(cards))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::setup_test_db;
    use crate::repo;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_create_card_handler() {
        let pool = setup_test_db();
        
        // First create an item type
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).unwrap();
        
        // Then create an item of that type
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).unwrap();
        
        // Create a payload for the card
        let payload = CreateCardDto {
            card_index: 3,
            priority: 0.5,
        };
        
        // Call the handler
        let result = create_card_handler(
            State(pool.clone()),
            Path(item.get_id()),
            Json(payload),
        ).await.unwrap();
        
        // Check the result
        let card = result.0;
        assert_eq!(card.get_item_id(), item.get_id());
        assert_eq!(card.get_card_index(), 3);
    }
    

    #[tokio::test]
    async fn test_create_card_handler_not_found() {
        let pool = setup_test_db();
        
        // Create a payload for the card
        let payload = CreateCardDto {
            card_index: 1,
            priority: 0.5,
        };
        
        // Call the handler with a non-existent item ID
        let result = create_card_handler(
            State(pool.clone()),
            Path("nonexistent".to_string()),
            Json(payload),
        ).await;
        
        // Check that we got a NotFound error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound));
    }
    
    #[tokio::test]
    async fn test_get_card_handler() {
        let pool = setup_test_db();
        
        // First create an item type
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).unwrap();
        
        // Then create an item of that type
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).unwrap();
        
        // Create a card for the item
        let card = repo::create_card(&pool, &item.get_id(), 3, 0.5).unwrap();
        
        // Call the handler
        let result = get_card_handler(
            State(pool.clone()),
            Path(card.get_id()),
        ).await.unwrap();
        
        // Check the result
        let retrieved_card = result.0.unwrap();
        assert_eq!(retrieved_card.get_id(), card.get_id());
        assert_eq!(retrieved_card.get_item_id(), item.get_id());
    }
    
    #[tokio::test]
    async fn test_get_card_handler_not_found() {
        let pool = setup_test_db();
        
        // Call the handler with a non-existent card ID
        let result = get_card_handler(
            State(pool.clone()),
            Path("nonexistent".to_string()),
        ).await.unwrap();
        
        // Check that we got None
        assert!(result.0.is_none());
    }
    
    #[tokio::test]
    async fn test_list_cards_handler() {
        let pool = setup_test_db();
        
        // Set up some test data
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).unwrap();
        
        // Create some cards
        let card1 = repo::create_card(&pool, &item.get_id(), 3, 0.5).unwrap();
        let card2 = repo::create_card(&pool, &item.get_id(), 4, 0.5).unwrap();
        
        // Call the handler with no filters
        let result = list_cards_handler(
            State(pool.clone()),
            Query(GetQueryDto::default()),
        ).await.unwrap();
        
        // Check the result
        let cards = result.0;
        assert_eq!(cards.len(), 4);
        assert!(cards.iter().any(|c| c.get_id() == card1.get_id()));
        assert!(cards.iter().any(|c| c.get_id() == card2.get_id()));
    }
    
    #[tokio::test]
    async fn test_list_cards_by_item_handler() {
        let pool = setup_test_db();
        
        // Set up some test data
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).unwrap();
        let item1 = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Item 1".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).unwrap();
        
        let item2 = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Item 2".to_string(),
            json!({"front": "Goodbye", "back": "World"}),
        ).unwrap();
        
        // Create cards for the items
        let card1 = repo::create_card(&pool, &item1.get_id(), 3, 0.5).unwrap();
        let card2 = repo::create_card(&pool, &item2.get_id(), 3, 0.5).unwrap();
        
        // Call the handler
        let result = list_cards_by_item_handler(
            State(pool.clone()),
            Path(item1.get_id()),
        ).await.unwrap();
        
        // Check the result
        let cards = result.0;
        assert_eq!(cards.len(), 3);
        assert!(cards.iter().any(|c| c.get_id() == card1.get_id()), "item 1's cards not found in list");
        assert!(!cards.iter().any(|c| c.get_id() == card2.get_id()), "item 2's cards found in list");
    }
    
    #[tokio::test]
    async fn test_list_cards_by_item_handler_not_found() {
        let pool = setup_test_db();
        
        // Call the handler with a non-existent item ID
        let result = list_cards_by_item_handler(
            State(pool.clone()),
            Path("nonexistent".to_string()),
        ).await;
        
        // Check that we got a NotFound error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound));
    }
} 
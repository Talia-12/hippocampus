use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use tracing::{instrument, debug, info};

use crate::db::DbPool;
use crate::dto::{CreateCardDto, GetQueryDto, UpdateCardPriorityDto};
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
#[instrument(skip(pool), fields(item_id = %item_id, card_index = %payload.card_index, priority = %payload.priority))]
pub async fn create_card_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item ID from the URL path
    Path(item_id): Path<String>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateCardDto>,
) -> Result<Json<Card>, ApiError> {
    info!("Creating new card for item");
    
    // First check if the item exists
    let item = repo::get_item(&pool, &item_id)
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound)?;
    
    // Call the repository function to create the card
    let card = repo::create_card(&pool, &item.get_id(), payload.card_index, payload.priority).await
        .map_err(ApiError::Database)?;

    info!("Successfully created card with id: {}", card.get_id());
    
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
#[instrument(skip(pool), fields(card_id = %id))]
pub async fn get_card_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the card ID from the URL path
    Path(id): Path<String>,
) -> Result<Json<Option<Card>>, ApiError> {
    debug!("Getting card");
    
    // Call the repository function to get the card
    let card = repo::get_card(&pool, &id)
        .map_err(ApiError::Database)?;
    
    if let Some(ref card) = card {
        debug!("Card found with id: {}", card.get_id());
    } else {
        debug!("Card not found");
    }
    
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
#[instrument(skip(pool, query))]
pub async fn list_cards_handler(
    // Extract the database connection pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract and parse query parameters
    Query(query): Query<GetQueryDto>,
) -> Result<Json<Vec<Card>>, ApiError> {
    debug!("Listing cards with filters: {:?}", query);
    
    // Call the repository function to list cards with the specified filters
    let cards = repo::list_cards_with_filters(&pool, &query)
        .map_err(ApiError::Database)?;
    
    info!("Retrieved {} cards", cards.len());
    
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
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn list_cards_by_item_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the item ID from the URL path
    Path(item_id): Path<String>,
) -> Result<Json<Vec<Card>>, ApiError> {
    debug!("Listing cards for item");
    
    // First check if the item exists
    repo::get_item(&pool, &item_id)
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound)?;
    
    // Call the repository function to get all cards for the item
    let cards = repo::get_cards_for_item(&pool, &item_id)
        .map_err(ApiError::Database)?;
    
    info!("Retrieved {} cards for item {}", cards.len(), item_id);
    
    // Return the list of cards as JSON
    Ok(Json(cards))
}


/// Handler for updating a card's priority
///
/// This function handles PUT requests to `/cards/{id}/priority`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `id` - The ID of the card to update
/// * `payload` - The request payload containing the new priority
///
/// ### Returns
///
/// The updated card as JSON
#[instrument(skip(pool), fields(card_id = %id, priority = %payload.priority))]
pub async fn update_card_priority_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the card ID from the URL path
    Path(id): Path<String>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<UpdateCardPriorityDto>,
) -> Result<Json<Card>, ApiError> {
    info!("Updating card priority");
    
    // Check if the priority is valid
    if payload.priority < 0.0 || payload.priority > 1.0 {
        return Err(ApiError::InvalidPriority(format!("Priority must be between 0 and 1, got {}", payload.priority)));
    }

    // Check if the card exists
    let _card = repo::get_card(&pool, &id)
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound)?;

    // Call the repository function to update the card's priority
    let card = repo::update_card_priority(&pool, &id, payload.priority).await
        .map_err(ApiError::Database)?;
    
    info!("Successfully updated card priority to {}", payload.priority);

    // Return the updated card as JSON
    Ok(Json(card))
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
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Then create an item of that type
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
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
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        
        // Then create an item of that type
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Create a card for the item
        let card = repo::create_card(&pool, &item.get_id(), 3, 0.5).await.unwrap();
        
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
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Create some cards
        let card1 = repo::create_card(&pool, &item.get_id(), 3, 0.5).await.unwrap();
        let card2 = repo::create_card(&pool, &item.get_id(), 4, 0.5).await.unwrap();
        
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
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let item1 = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Item 1".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        let item2 = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Item 2".to_string(),
            json!({"front": "Goodbye", "back": "World"}),
        ).await.unwrap();
        
        // Create cards for the items
        let card1 = repo::create_card(&pool, &item1.get_id(), 3, 0.5).await.unwrap();
        let card2 = repo::create_card(&pool, &item2.get_id(), 3, 0.5).await.unwrap();
        
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


    #[tokio::test]
    async fn test_update_card_priority_handler_success() {
        let pool = setup_test_db();
        
        // Create test data
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Create a card with initial priority
        let initial_priority = 0.5;
        let card = repo::create_card(&pool, &item.get_id(), 2, initial_priority).await.unwrap();
        
        // Update the card's priority
        let new_priority = 0.8;
        let payload = UpdateCardPriorityDto { priority: new_priority };
        
        let result = update_card_priority_handler(
            State(pool.clone()),
            Path(card.get_id()),
            Json(payload),
        ).await.unwrap();
        
        // Check the result
        let updated_card = result.0;
        assert!((updated_card.get_priority() - new_priority).abs() < 0.0001, "Priority not updated correctly, should be {}, but is {}", new_priority, updated_card.get_priority());
    }
    

    #[tokio::test]
    async fn test_update_card_priority_handler_boundary_values() {
        let pool = setup_test_db();
        
        // Create test data
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        let card = repo::create_card(&pool, &item.get_id(), 2, 0.5).await.unwrap();
        
        // Test minimum valid priority (0.0)
        let min_priority = 0.0;
        let payload = UpdateCardPriorityDto { priority: min_priority };
        
        let result = update_card_priority_handler(
            State(pool.clone()),
            Path(card.get_id()),
            Json(payload),
        ).await.unwrap();
        
        let updated_card = result.0;
        assert!((updated_card.get_priority() - min_priority).abs() < 0.0001);
        
        // Test maximum valid priority (1.0)
        let max_priority = 1.0;
        let payload = UpdateCardPriorityDto { priority: max_priority };
        
        let result = update_card_priority_handler(
            State(pool.clone()),
            Path(card.get_id()),
            Json(payload),
        ).await.unwrap();
        
        let updated_card = result.0;
        assert!((updated_card.get_priority() - max_priority).abs() < 0.0001);
    }
    

    #[tokio::test]
    async fn test_update_card_priority_handler_invalid_priority_too_low() {
        let pool = setup_test_db();
        
        // Create test data
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        let card = repo::create_card(&pool, &item.get_id(), 2, 0.5).await.unwrap();
        
        // Test priority below valid range
        let below_min_priority = -0.1;
        let payload = UpdateCardPriorityDto { priority: below_min_priority };
        
        let result = update_card_priority_handler(
            State(pool.clone()),
            Path(card.get_id()),
            Json(payload),
        ).await;
        
        // Should return an error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::InvalidPriority(_)));
    }
    

    #[tokio::test]
    async fn test_update_card_priority_handler_invalid_priority_too_high() {
        let pool = setup_test_db();
        
        // Create test data
        let item_type = repo::create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        let card = repo::create_card(&pool, &item.get_id(), 2, 0.5).await.unwrap();
        
        // Test priority above valid range
        let above_max_priority = 1.1;
        let payload = UpdateCardPriorityDto { priority: above_max_priority };
        
        let result = update_card_priority_handler(
            State(pool.clone()),
            Path(card.get_id()),
            Json(payload),
        ).await;
        
        // Should return an error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::InvalidPriority(_)));
    }
    
    
    #[tokio::test]
    async fn test_update_card_priority_handler_nonexistent_card() {
        let pool = setup_test_db();
        
        // Try to update a card that doesn't exist
        let nonexistent_card_id = "00000000-0000-0000-0000-000000000000";
        let payload = UpdateCardPriorityDto { priority: 0.5 };
        
        let result = update_card_priority_handler(
            State(pool.clone()),
            Path(nonexistent_card_id.to_string()),
            Json(payload),
        ).await;
        
        // Should return an error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound));
    }
}
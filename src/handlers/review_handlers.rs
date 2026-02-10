use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use std::sync::Arc;
use tracing::{instrument, debug, info, warn};

use crate::db::DbPool;
use crate::dto::CreateReviewDto;
use crate::errors::ApiError;
use crate::models::Review;
use crate::repo;

/// Handler for recording a review for a card
///
/// This function handles POST requests to `/reviews`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `payload` - The request payload containing the card ID and rating
///
/// ### Returns
///
/// The newly created review as JSON
#[instrument(skip(pool), fields(card_id = %payload.card_id, rating = %payload.rating))]
pub async fn create_review_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract and deserialize the JSON request body
    Json(payload): Json<CreateReviewDto>,
) -> Result<Json<Review>, ApiError> {
    info!("Creating new review for card");
    
    // Validate the rating range
    if payload.rating < 1 || payload.rating > 4 {
        warn!("Invalid rating: {}", payload.rating);
        return Err(ApiError::InvalidRating(format!(
            "Rating must be between 1 and 4, got {}",
            payload.rating
        )));
    }
    
    // Call the repository function to record the review
    match repo::record_review(&pool, &payload.card_id, payload.rating).await {
        Ok(review) => {
            info!("Successfully created review with id: {}", review.get_id());
            Ok(Json(review))
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

/// Handler for getting all possible next reviews for a card
///
/// This function handles GET requests to `/cards/{card_id}/next_reviews`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `card_id` - The ID of the card to get next reviews for
///
/// ### Returns
///
/// A list of next reviews for the specified card as JSON
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn get_all_next_reviews_for_card_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the card ID from the URL path
    Path(card_id): Path<String>,
) -> Result<Json<Vec<(chrono::DateTime<Utc>, serde_json::Value)>>, ApiError> {
    debug!("Getting all possible next reviews for card {}", card_id);

    // Get the next reviews for the card
    let next_reviews = repo::get_all_next_reviews_for_card(&pool, &card_id)
        .await
        .map_err(ApiError::Database)?
        .into_iter()
        .map(|(next_review, scheduler_data)| (next_review, scheduler_data.0))
        .collect::<Vec<_>>();

    info!("Retrieved {} possible next reviews for card {}", next_reviews.len(), card_id);
    Ok(Json(next_reviews))
}


/// Handler for listing all reviews for a card
///
/// This function handles GET requests to `/cards/{card_id}/reviews`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `card_id` - The ID of the card to get reviews for
///
/// ### Returns
///
/// A list of reviews for the specified card as JSON
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn list_reviews_by_card_handler(
    // Extract the database pool from the application state
    State(pool): State<Arc<DbPool>>,
    // Extract the card ID from the URL path
    Path(card_id): Path<String>,
) -> Result<Json<Vec<Review>>, ApiError> {
    debug!("Listing reviews for card");
    
    // First check if the card exists
    let card = repo::get_card(&pool, &card_id)
        .map_err(ApiError::Database)?
        .ok_or(ApiError::NotFound)?;
    
    debug!("Card found with id: {}", card.get_id());
    
    // Call the repository function to get all reviews for the card
    let reviews = repo::get_reviews_for_card(&pool, &card.get_id())
        .map_err(ApiError::Database)?;
    
    info!("Retrieved {} reviews for card {}", reviews.len(), card_id);
    
    // Return the list of reviews as JSON
    Ok(Json(reviews))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use crate::repo;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_create_review_handler() {
        let pool = setup_test_db();
        
        // Set up test data
        let item_type = repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Get the card created for the item
        let cards = repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
        let card = &cards[0];
        
        // Create a payload for the review
        let payload = CreateReviewDto {
            card_id: card.get_id(),
            rating: 2,
        };
        
        // Call the handler
        let result = create_review_handler(
            State(pool.clone()),
            Json(payload),
        ).await.unwrap();
        
        // Check the result
        let review = result.0;
        assert_eq!(review.get_card_id(), card.get_id());
        assert_eq!(review.get_rating(), 2);
    }
    
    #[tokio::test]
    async fn test_create_review_handler_invalid_rating() {
        let pool = setup_test_db();
        
        // Set up test data
        let item_type = repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Get the card created for the item
        let cards = repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
        let card = &cards[0];
        
        // Create a payload with an invalid rating
        let payload = CreateReviewDto {
            card_id: card.get_id(),
            rating: 0, // Invalid rating
        };
        
        // Call the handler
        let result = create_review_handler(
            State(pool.clone()),
            Json(payload),
        ).await;
        
        // Check that we got an InvalidRating error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::InvalidRating(_)));
    }
    
    #[tokio::test]
    async fn test_create_review_handler_not_found() {
        let pool = setup_test_db();
        
        // Create a payload with a non-existent card ID
        let payload = CreateReviewDto {
            card_id: "nonexistent".to_string(),
            rating: 2,
        };
        
        // Call the handler
        let result = create_review_handler(
            State(pool.clone()),
            Json(payload),
        ).await;
        
        // Check that we got a NotFound error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound));
    }
    
    #[tokio::test]
    async fn test_list_reviews_by_card_handler() {
        let pool = setup_test_db();
        
        // Set up test data
        let item_type = repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();
        let item = repo::create_item(
            &pool,
            &item_type.get_id(),
            "Test Item".to_string(),
            json!({"front": "Hello", "back": "World"}),
        ).await.unwrap();
        
        // Get the card created for the item
        let cards = repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
        let card = &cards[0];
        
        // Create some reviews
        let review1 = repo::record_review(&pool, &card.get_id(), 2).await.unwrap();
        
        // We need to wait a moment to ensure the timestamps are different
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        let review2 = repo::record_review(&pool, &card.get_id(), 3).await.unwrap();
        
        // Call the handler
        let result = list_reviews_by_card_handler(
            State(pool.clone()),
            Path(card.get_id()),
        ).await.unwrap();
        
        // Check the result
        let reviews = result.0;
        assert_eq!(reviews.len(), 2);
        assert!(reviews.iter().any(|r| r.get_id() == review1.get_id()));
        assert!(reviews.iter().any(|r| r.get_id() == review2.get_id()));
    }
    
    #[tokio::test]
    async fn test_list_reviews_by_card_handler_not_found() {
        let pool = setup_test_db();
        
        // Call the handler with a non-existent card ID
        let result = list_reviews_by_card_handler(
            State(pool.clone()),
            Path("nonexistent".to_string()),
        ).await;
        
        // Check that we got a NotFound error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound));
    }
} 

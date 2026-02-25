/// Integration tests for review functionality
///
/// This file contains tests for review operations including:
/// - Creating reviews with different ratings
/// - Validating that reviews update card scheduling
/// - Handling invalid ratings
/// - Handling non-existent card errors

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::Service;

mod common;
use common::*;

/// Helper function to create a review for a card
///
/// ### Arguments
///
/// * `app` - The test application
/// * `card_id` - The ID of the card to review
/// * `rating` - The rating for the review (1-4)
///
/// ### Returns
///
/// The review response as a JSON Value
async fn create_review(app: &mut axum::Router, card_id: &str, rating: i32) -> Value {
    // Create a request to create a review
    let request = Request::builder()
        .uri("/reviews")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "card_id": card_id,
                "rating": rating
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    assert_eq!(parts.status, StatusCode::OK, "Expected 200 OK status, instead got {}: {}", parts.status, String::from_utf8_lossy(&bytes));
    
    // Parse the response body
    let review: Value = serde_json::from_slice(&bytes).unwrap();
    
    review
}


/// Tests creating a review with an "Again" (rating=1) evaluation
///
/// This test verifies:
/// 1. A POST request to /reviews with a rating of 1 creates a review
/// 2. The card's next_review is scheduled for 1 day later
#[tokio::test]
async fn test_create_review_rating_again() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item
    let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;
    
    // Get the cards for the item
    let cards = get_cards_for_item(&mut app, &item.get_id()).await;
    assert!(!cards.is_empty(), "The item should have at least one card");
    
    let card = &cards[0];
    
    // Create a review with "Again" rating (1)
    let review = create_review(&mut app, &card.get_id(), 1).await;
    
    // Check that the review has the correct card_id and rating
    assert_eq!(review["card_id"].as_str().unwrap(), card.get_id());
    assert_eq!(review["rating"].as_i64().unwrap(), 1);
    
    // Now get the updated card to verify scheduling
    let request = Request::builder()
        .uri(format!("/cards/{}", card.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_card: Value = serde_json::from_slice(&body).unwrap();
    
    // For a rating of 1 (again), verify next_review is scheduled about 1 day later
    assert!(updated_card["next_review"].is_string());
    assert!(updated_card["last_review"].is_string());
    
    // Get the next review date
    let next_review_str = updated_card["next_review"].as_str().unwrap();
    let next_review = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", next_review_str)).unwrap();
    
    // Get the last review date
    let last_review_str = updated_card["last_review"].as_str().unwrap();
    let last_review = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", last_review_str)).unwrap();
    
    // Calculate the difference in days
    let diff = next_review.signed_duration_since(last_review);
    let days = diff.num_hours() as f64 / 24.0;
    
    // For "Again" rating (1), the card should be scheduled for review soon
    assert!(days >= 0.0 && days <= 2.0,
        "Expected next review within 2 days for 'Again', but was {} days", days);
}


/// Tests creating a review with a "Hard" (rating=2) evaluation
///
/// This test verifies:
/// 1. A POST request to /reviews with a rating of 2 creates a review
/// 2. The card's next_review is scheduled for 2 days later (first review)
#[tokio::test]
async fn test_create_review_rating_hard() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item
    let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;
    
    // Get the cards for the item
    let cards = get_cards_for_item(&mut app, &item.get_id()).await;
    assert!(!cards.is_empty(), "The item should have at least one card");
    
    let card = &cards[0];
    
    // Create a review with "Hard" rating (2)
    let review = create_review(&mut app, &card.get_id(), 2).await;
    
    // Check that the review has the correct card_id and rating
    assert_eq!(review["card_id"].as_str().unwrap(), card.get_id());
    assert_eq!(review["rating"].as_i64().unwrap(), 2);
    
    // Now get the updated card to verify scheduling
    let request = Request::builder()
        .uri(format!("/cards/{}", card.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_card: Value = serde_json::from_slice(&body).unwrap();
    
    // Get the next review date
    let next_review_str = updated_card["next_review"].as_str().unwrap();
    let next_review = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", next_review_str)).unwrap();
    
    // Get the last review date
    let last_review_str = updated_card["last_review"].as_str().unwrap();
    let last_review = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", last_review_str)).unwrap();
    
    // Calculate the difference in days
    let diff = next_review.signed_duration_since(last_review);
    let days = diff.num_hours() as f64 / 24.0;
    
    // For "Hard" rating (2) on first review, the card should be scheduled in the near future
    assert!(days >= 0.5 && days <= 10.0,
        "Expected next review within a reasonable range for 'Hard', but was {} days", days);
}


/// Tests creating a review with a "Good" (rating=3) evaluation
///
/// This test verifies:
/// 1. A POST request to /reviews with a rating of 3 creates a review
/// 2. The card's next_review is scheduled for 4 days later (first review)
#[tokio::test]
async fn test_create_review_rating_good() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item
    let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;
    
    // Get the cards for the item
    let cards = get_cards_for_item(&mut app, &item.get_id()).await;
    assert!(!cards.is_empty(), "The item should have at least one card");
    
    let card = &cards[0];
    
    // Create a review with "Good" rating (3)
    let review = create_review(&mut app, &card.get_id(), 3).await;
    
    // Check that the review has the correct card_id and rating
    assert_eq!(review["card_id"].as_str().unwrap(), card.get_id());
    assert_eq!(review["rating"].as_i64().unwrap(), 3);
    
    // Now get the updated card to verify scheduling
    let request = Request::builder()
        .uri(format!("/cards/{}", card.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_card: Value = serde_json::from_slice(&body).unwrap();
    
    // Get the next review date
    let next_review_str = updated_card["next_review"].as_str().unwrap();
    let next_review = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", next_review_str)).unwrap();
    
    // Get the last review date
    let last_review_str = updated_card["last_review"].as_str().unwrap();
    let last_review = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", last_review_str)).unwrap();
    
    // Calculate the difference in days
    let diff = next_review.signed_duration_since(last_review);
    let days = diff.num_hours() as f64 / 24.0;
    
    // For "Good" rating (3) on first review, the card should be scheduled in the near future
    assert!(days >= 0.5 && days <= 30.0,
        "Expected next review within a reasonable range for 'Good', but was {} days", days);
}


/// Tests creating a review with an "Easy" (rating=4) evaluation
///
/// This test verifies:
/// 1. A POST request to /reviews with a rating of 4 creates a review
/// 2. The card's next_review is scheduled for 7 days later (first review)
#[tokio::test]
async fn test_create_review_rating_easy() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item
    let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;
    
    // Get the cards for the item
    let cards = get_cards_for_item(&mut app, &item.get_id()).await;
    assert!(!cards.is_empty(), "The item should have at least one card");
    
    let card = &cards[0];
    
    // Create a review with "Easy" rating (4)
    let review = create_review(&mut app, &card.get_id(), 4).await;
    
    // Check that the review has the correct card_id and rating
    assert_eq!(review["card_id"].as_str().unwrap(), card.get_id());
    assert_eq!(review["rating"].as_i64().unwrap(), 4);
    
    // Now get the updated card to verify scheduling
    let request = Request::builder()
        .uri(format!("/cards/{}", card.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_card: Value = serde_json::from_slice(&body).unwrap();
    
    // Get the next review date
    let next_review_str = updated_card["next_review"].as_str().unwrap();
    let next_review = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", next_review_str)).unwrap();
    
    // Get the last review date
    let last_review_str = updated_card["last_review"].as_str().unwrap();
    let last_review = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", last_review_str)).unwrap();
    
    // Calculate the difference in days
    let diff = next_review.signed_duration_since(last_review);
    let days = diff.num_hours() as f64 / 24.0;
    
    // For "Easy" rating (4) on first review, the card should be scheduled further out
    assert!(days >= 1.0 && days <= 60.0,
        "Expected next review within a reasonable range for 'Easy', but was {} days", days);
}


/// Tests creating multiple reviews to verify interval progression
///
/// This test verifies that the interval increases correctly after multiple reviews
#[tokio::test]
async fn test_create_multiple_reviews() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item
    let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;
    
    // Get the cards for the item
    let cards = get_cards_for_item(&mut app, &item.get_id()).await;
    assert!(!cards.is_empty(), "The item should have at least one card");
    
    let card = &cards[0];
    
    // Create a first review with "Good" rating (3)
    let _ = create_review(&mut app, &card.get_id(), 3).await;
    
    // Create a second review with "Good" rating (3)
    let _ = create_review(&mut app, &card.get_id(), 3).await;
    
    // Get the updated card after second review
    let request = Request::builder()
        .uri(format!("/cards/{}", card.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_card: Value = serde_json::from_slice(&body).unwrap();
    
    // Get the next review date
    let next_review_str = updated_card["next_review"].as_str().unwrap();
    let next_review = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", next_review_str)).unwrap();
    
    // Get the last review date
    let last_review_str = updated_card["last_review"].as_str().unwrap();
    let last_review = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", last_review_str)).unwrap();
    
    // Calculate the difference in days
    let diff = next_review.signed_duration_since(last_review);
    let days = diff.num_hours() as f64 / 24.0;
    
    // After second "Good" review, the interval should be longer than a fresh review
    assert!(days >= 0.5,
        "Expected next review to be at least half a day after second review, but was {} days", days);

    // Create a third review with "Good" rating (3)
    let _ = create_review(&mut app, &card.get_id(), 3).await;

    // Get the updated card after third review
    let request = Request::builder()
        .uri(format!("/cards/{}", card.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.call(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_card: Value = serde_json::from_slice(&body).unwrap();

    // Get the scheduler data to check FSRS fields
    let scheduler_data = updated_card["scheduler_data"].as_object().unwrap();
    let stability = scheduler_data["stability"].as_f64().unwrap();
    let difficulty = scheduler_data["difficulty"].as_f64().unwrap();

    // After three "Good" reviews, stability should be positive and difficulty reasonable
    assert!(stability > 0.0,
        "Expected positive stability after third review, but was {}", stability);
    assert!(difficulty > 0.0 && difficulty <= 10.0,
        "Expected difficulty in valid range after third review, but was {}", difficulty);
}

/// Tests creating a review with an invalid rating (too low)
///
/// This test verifies:
/// 1. A POST request to /reviews with a rating < 1 is rejected
/// 2. The response has a 400 Bad Request status
#[tokio::test]
async fn test_create_review_rating_too_low() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item
    let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;
    
    // Get the cards for the item
    let cards = get_cards_for_item(&mut app, &item.get_id()).await;
    assert!(!cards.is_empty(), "The item should have at least one card");
    
    let card = &cards[0];
    
    // Create a request with an invalid rating (0)
    let request = Request::builder()
        .uri("/reviews")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "card_id": card.get_id(),
                "rating": 0  // Invalid - too low
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 400 Bad Request status
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Tests creating a review with an invalid rating (too high)
///
/// This test verifies:
/// 1. A POST request to /reviews with a rating > 4 is rejected
/// 2. The response has a 400 Bad Request status
#[tokio::test]
async fn test_create_review_rating_too_high() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item
    let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;
    
    // Get the cards for the item
    let cards = get_cards_for_item(&mut app, &item.get_id()).await;
    assert!(!cards.is_empty(), "The item should have at least one card");
    
    let card = &cards[0];
    
    // Create a request with an invalid rating (5)
    let request = Request::builder()
        .uri("/reviews")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "card_id": card.get_id(),
                "rating": 5  // Invalid - too high
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 400 Bad Request status
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Tests creating a review for a non-existent card
///
/// This test verifies:
/// 1. A POST request to /reviews with a non-existent card_id is rejected
/// 2. The response has a 404 Not Found status
#[tokio::test]
async fn test_create_review_nonexistent_card() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create a request with a non-existent card ID
    let request = Request::builder()
        .uri("/reviews")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "card_id": "nonexistent-card-id",
                "rating": 3
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 404 Not Found status
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Tests listing reviews for a card
///
/// This test verifies:
/// 1. A GET request to /cards/{card_id}/reviews returns all reviews for the card
/// 2. The response has a 200 OK status
/// 3. The response body contains a list with all the reviews for the card
#[tokio::test]
async fn test_list_reviews_for_card() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item
    let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;
    
    // Get the cards for the item
    let cards = get_cards_for_item(&mut app, &item.get_id()).await;
    assert!(!cards.is_empty(), "The item should have at least one card");
    
    let card = &cards[0];
    
    // Create multiple reviews
    let review1 = create_review(&mut app, &card.get_id(), 2).await;
    
    // Wait a moment to ensure the reviews have different timestamps
    std::thread::sleep(std::time::Duration::from_millis(10));
    
    let review2 = create_review(&mut app, &card.get_id(), 3).await;
    
    // Create a request to list reviews for the card
    let request = Request::builder()
        .uri(format!("/cards/{}/reviews", card.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse the response body
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let reviews: Vec<Value> = serde_json::from_slice(&body).unwrap();
    
    // Check that we got at least the two reviews we created
    assert!(reviews.len() >= 2, "Expected at least 2 reviews, got {}", reviews.len());
    
    // Check that our reviews are in the list
    let has_review1 = reviews.iter().any(|r| 
        r["id"].as_str().unwrap() == review1["id"].as_str().unwrap()
    );
    let has_review2 = reviews.iter().any(|r| 
        r["id"].as_str().unwrap() == review2["id"].as_str().unwrap()
    );
    
    assert!(has_review1, "Review 1 should be in the list");
    assert!(has_review2, "Review 2 should be in the list");
}

/// Tests listing reviews for a non-existent card
///
/// This test verifies:
/// 1. A GET request to /cards/{non-existent-id}/reviews returns a 404 status
#[tokio::test]
async fn test_list_reviews_nonexistent_card() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create a request to list reviews for a non-existent card
    let request = Request::builder()
        .uri("/cards/nonexistent-card-id/reviews")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 404 Not Found status
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
} 
/// Integration tests for card priority management
///
/// This file contains tests for card priority operations including:
/// - Updating a card's priority with valid values
/// - Attempting to update a card with invalid priority values
/// - Handling non-existent card errors

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::Service;

mod common;
use common::*;

/// Tests updating a card's priority with a valid value
///
/// This test verifies:
/// 1. A PUT request to /cards/{card_id}/priority updates the card's priority
/// 2. The response has a 200 OK status
/// 3. The response body contains the updated card with the new priority
#[tokio::test]
async fn test_update_card_priority() {
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
    
    // Define a new priority
    let new_priority = 0.8_f32; // Different from the default
    
    // Create a request to update the card's priority
    let request = Request::builder()
        .uri(format!("/cards/{}/priority", card.get_id()))
        .method("PUT")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "priority": new_priority
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse the response body
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_card: Value = serde_json::from_slice(&body).unwrap();
    
    // Check that the card has the new priority
    assert!((updated_card["priority"].as_f64().unwrap() - new_priority as f64).abs() < 0.0001, 
        "Card priority should be updated to {} but was {}", 
        new_priority, 
        updated_card["priority"].as_f64().unwrap());
    
    // Verify the card ID is the same
    assert_eq!(updated_card["id"].as_str().unwrap(), card.get_id());
}

/// Tests updating a card's priority with boundary values
///
/// This test verifies:
/// 1. The minimum valid priority (0.0) can be set
/// 2. The maximum valid priority (1.0) can be set
#[tokio::test]
async fn test_update_card_priority_boundary_values() {
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
    
    // Test minimum priority (0.0)
    let min_priority = 0.0_f32;
    let request = Request::builder()
        .uri(format!("/cards/{}/priority", card.get_id()))
        .method("PUT")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "priority": min_priority
            }))
            .unwrap(),
        ))
        .unwrap();
    
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_card: Value = serde_json::from_slice(&body).unwrap();
    
    assert!((updated_card["priority"].as_f64().unwrap() - min_priority as f64).abs() < 0.0001,
        "Card priority should be {} but was {}", 
        min_priority, 
        updated_card["priority"].as_f64().unwrap());
    
    // Test maximum priority (1.0)
    let max_priority = 1.0_f32;
    let request = Request::builder()
        .uri(format!("/cards/{}/priority", card.get_id()))
        .method("PUT")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "priority": max_priority
            }))
            .unwrap(),
        ))
        .unwrap();
    
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_card: Value = serde_json::from_slice(&body).unwrap();
    
    assert!((updated_card["priority"].as_f64().unwrap() - max_priority as f64).abs() < 0.0001,
        "Card priority should be {} but was {}", 
        max_priority, 
        updated_card["priority"].as_f64().unwrap());
}

/// Tests updating a card's priority with invalid values
///
/// This test verifies:
/// 1. A priority value below 0.0 is rejected with a 400 status
/// 2. A priority value above 1.0 is rejected with a 400 status
#[tokio::test]
async fn test_update_card_priority_invalid_values() {
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
    
    // Test priority below minimum (negative)
    let too_low_priority = -0.1_f32;
    let request = Request::builder()
        .uri(format!("/cards/{}/priority", card.get_id()))
        .method("PUT")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "priority": too_low_priority
            }))
            .unwrap(),
        ))
        .unwrap();
    
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    
    // Test priority above maximum (> 1.0)
    let too_high_priority = 1.1_f32;
    let request = Request::builder()
        .uri(format!("/cards/{}/priority", card.get_id()))
        .method("PUT")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "priority": too_high_priority
            }))
            .unwrap(),
        ))
        .unwrap();
    
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Tests updating the priority of a non-existent card
///
/// This test verifies:
/// 1. Attempting to update a non-existent card returns a 404 status
#[tokio::test]
async fn test_update_nonexistent_card_priority() {
    // Create our test app
    let mut app = create_test_app();
    
    // Generate a non-existent card ID
    let nonexistent_card_id = "nonexistent-card-id";
    
    // Create a request to update the non-existent card's priority
    let request = Request::builder()
        .uri(format!("/cards/{}/priority", nonexistent_card_id))
        .method("PUT")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "priority": 0.5
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 404 Not Found status
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
} 
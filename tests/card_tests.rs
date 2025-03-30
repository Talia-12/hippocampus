/// Integration tests for card functionality
///
/// This file contains tests for card operations:
/// - Getting cards by ID
/// - Listing cards for an item
/// - Creating cards

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::Service;
use hippocampus::models::Card;

mod common;
use common::*;

/// Tests creating a new card for an item via the API
///
/// This test verifies:
/// 1. A POST request to /items/{id}/cards creates a new card
/// 2. The response has a 200 OK status
/// 3. The response body contains the created card with the correct fields
#[tokio::test]
async fn test_create_card() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item
    let item = create_item(
        &mut app,
        &item_type.get_id(),
        "Item with Cards".to_string(),
        None
    ).await;
    
    // Define card parameters
    let card_index = 2;
    let priority = 0.7_f32;
    
    // Create a request to create a card
    let request = Request::builder()
        .uri(format!("/items/{}/cards", item.get_id()))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "card_index": card_index,
                "priority": priority
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into a Card
    let card: Card = serde_json::from_slice(&body).unwrap();
    
    // Check that the card has the correct fields
    assert_eq!(card.get_item_id(), item.get_id());
    assert_eq!(card.get_card_index(), card_index);
    assert!((card.get_priority() - priority).abs() < 0.0001);
    assert!(!card.get_id().is_empty());
}

/// Tests getting a card by ID via the API
///
/// This test verifies:
/// 1. A GET request to /cards/{id} returns the correct card
/// 2. The response has a 200 OK status
/// 3. The response body contains the card with the correct fields
#[tokio::test]
async fn test_get_card() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item (which will automatically create a card)
    let item = create_item(
        &mut app,
        &item_type.get_id(),
        "Item for Card Get".to_string(),
        None
    ).await;
    
    // Get the cards for the item
    let cards = get_cards_for_item(&mut app, &item.get_id()).await;
    assert!(!cards.is_empty(), "The item should have at least one card");
    
    let card = &cards[0];
    
    // Create a request to get the card by ID
    let request = Request::builder()
        .uri(format!("/cards/{}", card.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into a card
    let retrieved_card: Option<Card> = serde_json::from_slice(&body).unwrap();
    
    // Check that the card exists
    assert!(retrieved_card.is_some(), "Card should exist");
    
    // Check that the retrieved card has the correct fields
    let retrieved_card = retrieved_card.unwrap();
    assert_eq!(retrieved_card.get_id(), card.get_id());
    assert_eq!(retrieved_card.get_item_id(), item.get_id());
}

/// Tests listing cards for an item via the API
///
/// This test verifies:
/// 1. A GET request to /items/{id}/cards returns all cards for the item
/// 2. The response has a 200 OK status
/// 3. The response body contains a list of cards
/// 4. All cards have the correct item_id
#[tokio::test]
async fn test_list_cards_for_item() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create an item (which will automatically create a card)
    let item = create_item(
        &mut app,
        &item_type.get_id(),
        "Item for Card List".to_string(),
        None
    ).await;
    
    // Create some additional cards for the item
    let card1 = create_card(&mut app, &item.get_id(), 2, 0.7).await;
    let card2 = create_card(&mut app, &item.get_id(), 3, 0.8).await;
    
    // Create a request to list cards for the item
    let request = Request::builder()
        .uri(format!("/items/{}/cards", item.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into a vector of cards
    let cards: Vec<Card> = serde_json::from_slice(&body).unwrap();
    
    // Check that we have at least 3 cards (1 default + 2 created)
    assert!(cards.len() >= 3, "Should have at least 3 cards");
    
    // Check that all cards have the correct item_id
    for card in &cards {
        assert_eq!(card.get_item_id(), item.get_id());
    }
    
    // Check that our created cards are in the list
    let has_card1 = cards.iter().any(|c| c.get_id() == card1.get_id());
    let has_card2 = cards.iter().any(|c| c.get_id() == card2.get_id());
    
    assert!(has_card1, "Card 1 should be in the list");
    assert!(has_card2, "Card 2 should be in the list");
}

/// Tests getting a non-existent card via the API
///
/// This test verifies:
/// 1. A GET request to /cards/{id} with a non-existent ID returns null
/// 2. The response has a 200 OK status
/// 3. The response body can be correctly deserialized as a null Option<Card>
#[tokio::test]
async fn test_get_nonexistent_card() {
    // Create our test app
    let mut app = create_test_app();
    
    // Generate a random UUID for a non-existent card
    let non_existent_id = uuid::Uuid::new_v4().to_string();
    
    // Create a request to get a non-existent card
    let request = Request::builder()
        .uri(format!("/cards/{}", non_existent_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into an Option<Card>
    let card: Option<Card> = serde_json::from_slice(&body).unwrap();
    
    // Check that the card is None (null in JSON)
    assert!(card.is_none(), "Non-existent card should return null");
}

/// Tests listing all cards via the API
///
/// This test verifies:
/// 1. A GET request to /cards returns all cards
/// 2. The response has a 200 OK status
/// 3. The response body contains a list of cards
#[tokio::test]
async fn test_list_all_cards() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Basic".to_string()).await;
    
    // Create a few items (each with a default card)
    let item1 = create_item(
        &mut app,
        &item_type.get_id(),
        "Item 1 for Card List".to_string(),
        None
    ).await;
    
    let item2 = create_item(
        &mut app,
        &item_type.get_id(),
        "Item 2 for Card List".to_string(),
        None
    ).await;
    
    // Create a request to list all cards
    let request = Request::builder()
        .uri("/cards")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into a vector of cards
    let cards: Vec<Card> = serde_json::from_slice(&body).unwrap();
    
    // Check that we have at least 2 cards (one for each item)
    assert!(cards.len() >= 2, "Should have at least 2 cards");
    
    // Check that there is at least one card for each item
    let has_item1_card = cards.iter().any(|c| c.get_item_id() == item1.get_id());
    let has_item2_card = cards.iter().any(|c| c.get_item_id() == item2.get_id());
    
    assert!(has_item1_card, "Should have a card for item 1");
    assert!(has_item2_card, "Should have a card for item 2");
} 
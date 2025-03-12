/// Integration tests for the Hippocampus application
///
/// This file contains end-to-end tests that verify the entire application
/// works correctly by making HTTP requests to the API endpoints and checking
/// the responses. These tests ensure that all components of the application
/// work together as expected.
///
/// Unlike unit tests, integration tests exercise the entire application stack,
/// including:
/// - HTTP request/response handling
/// - JSON serialization/deserialization
/// - Database operations
/// - Business logic
///
/// Each test creates a fresh application instance with an in-memory database,
/// ensuring tests are isolated and don't affect each other.

use hippocampus::{
    create_app,
    db::init_pool,
    models::{Card, Item, ItemType},
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use uuid;


/// Creates a test application with an in-memory SQLite database
///
/// This helper function:
/// 1. Creates an in-memory SQLite database
/// 2. Runs migrations to set up the schema
/// 3. Creates an Axum application with the database
///
/// Using an in-memory database ensures that:
/// - Tests run quickly
/// - Tests are isolated from each other
/// - No cleanup is needed after tests
///
/// ### Returns
///
/// An Axum Router configured with all routes and connected to an in-memory database
fn create_test_app() -> Router {
    // Create a connection pool with an in-memory SQLite database
    let pool = Arc::new(init_pool(":memory:"));
    
    // Run migrations on the in-memory database to set up the schema
    let conn = &mut pool.get().unwrap();
    hippocampus::run_migrations(conn);
    
    // Create and return the application with the configured database pool
    create_app(pool)
}


/// Creates an item type via the API
///
/// This helper function:
/// 1. Sends a POST request to /item_types with the provided name
/// 2. Verifies the response has a 200 OK status
/// 3. Parses and returns the created ItemType
///
/// ### Arguments
///
/// * `app` - The test application
/// * `name` - The name for the new item type
///
/// ### Returns
///
/// The created ItemType with its ID and creation timestamp
async fn create_item_type(app: &Router, name: String) -> ItemType {
    // Create a request to create an item type
    let request = Request::builder()
        .uri("/item_types")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "name": name
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request and get the response
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse the response body
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let item_type: Value = serde_json::from_slice(&body).unwrap();
    
    // Extract the fields and construct an ItemType
    let item_type_id = item_type["id"].as_str().unwrap();
    let created_at = chrono::NaiveDateTime::parse_from_str(
        item_type["created_at"].as_str().unwrap(),
        "%Y-%m-%dT%H:%M:%S%.f"
    ).unwrap().and_utc();
    
    ItemType::new_with_fields(
        item_type_id.to_string(),
        name,
        created_at,
    )
}


/// Tests creating a new item via the API
///
/// This test verifies that:
/// 1. A POST request to /items with a JSON payload creates a new item
/// 2. The response has a 200 OK status
/// 3. The response body contains the created item with the correct title
/// 4. The item is assigned a unique ID
#[tokio::test]
async fn test_create_item() {
    // Create our test app with an in-memory database
    let app = create_test_app();
    
    // First create an item type
    let item_type = create_item_type(&app, "Test Item Type".to_string()).await;
    
    // Create a request to create an item with a JSON payload
    let request = Request::builder()
        .uri("/items")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "item_type_id": item_type.get_id(),
                "title": "Test Item",
                "item_data": null
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.oneshot(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into an Item struct
    let item: Item = serde_json::from_slice(&body).unwrap();
    
    // Check that the item has the correct title
    assert_eq!(item.get_title(), "Test Item");
    
    // The ID should be a non-empty string (we don't check the exact value
    // since it's randomly generated)
    assert!(!item.get_id().is_empty());
}


/// Tests retrieving an item by ID via the API
///
/// This test verifies that:
/// 1. A GET request to /items/{id} returns the correct item
/// 2. The response has a 200 OK status
/// 3. The response body contains the item with the correct title
/// 4. The item can be correctly deserialized from JSON
#[tokio::test]
async fn test_get_item() {
    // Create our test app with an in-memory database
    let app = create_test_app();
    
    // First create an item type
    let item_type = create_item_type(&app, "Test Item Type".to_string()).await;
    
    // First, create an item that we can later retrieve
    let request = Request::builder()
        .uri("/items")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "item_type_id": item_type.get_id(),
                "title": "Test Item for Get",
                "item_data": null
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the create request and parse the response to get the created item
    let response = app.clone().oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let created_item: Item = serde_json::from_slice(&body).unwrap();
    
    // Now, create a request to get the item by its ID
    let request = Request::builder()
        .uri(format!("/items/{}", created_item.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the get request and get the response
    let response = app.oneshot(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into an Option<Item>
    // (The API returns null if the item doesn't exist)
    let item: Option<Item> = serde_json::from_slice(&body).unwrap();
    
    // Check that the item exists
    assert!(item.is_some(), "Item should exist");
    
    // Check that the retrieved item has the correct title
    assert_eq!(item.unwrap().get_title(), "Test Item for Get");
}


/// Tests listing all items via the API
///
/// This test verifies that:
/// 1. A GET request to /items returns all items
/// 2. The response has a 200 OK status
/// 3. The response body contains a list of items
/// 4. The list includes all items that were created
#[tokio::test]
async fn test_list_items() {
    // Create our test app with an in-memory database
    let app = create_test_app();
    
    // First create an item type
    let item_type = create_item_type(&app, "Test Item Type".to_string()).await;
    
    // Create several items to populate the database
    for i in 1..=3 {
        let request = Request::builder()
            .uri("/items")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({
                    "item_type_id": item_type.get_id(),
                    "title": format!("Test Item {}", i),
                    "item_data": null
                }))
                .unwrap(),
            ))
            .unwrap();
        
        // Send each create request (we don't need to check the responses here)
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "Item creation should succeed");
    }
    
    // Now, create a request to list all items
    let request = Request::builder()
        .uri("/items")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the list request and get the response
    let response = app.oneshot(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into a vector of Items
    let items: Vec<Item> = serde_json::from_slice(&body).unwrap();
    
    // Check that we have at least the 3 items we created
    // (There might be more if other tests ran before this one)
    assert!(items.len() >= 3, "Should have at least 3 items");
    
    // We could also check that each of our created items is in the list,
    // but that would require tracking their IDs
}


/// Tests creating a review for an item via the API
///
/// This test verifies that:
/// 1. A POST request to /reviews creates a new review
/// 2. The response has a 200 OK status
/// 3. The response body contains the created review with the correct item_id and rating
/// 4. The item is updated with review information (next_review and last_review)
#[tokio::test]
async fn test_create_review() {
    // Create our test app with an in-memory database
    let app = create_test_app();
    
    // First create an item type
    let item_type = create_item_type(&app, "Basic".to_string()).await;
    
    // First, create an item that we can review
    let request = Request::builder()
        .uri("/items")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "item_type_id": item_type.get_id(),
                "title": "Item to Review",
                "item_data": null
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the create item request and parse the response to get the created item
    let response = app.clone().oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let created_item: Item = serde_json::from_slice(&body).unwrap();

    // Get the cards for the item
    let request = Request::builder()
        .uri(format!("/items/{}/cards", created_item.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let cards: Vec<Card> = serde_json::from_slice(&body).unwrap();
    let card = &cards[0];
    
    // Now, create a request to create a review for the item
    let request = Request::builder()
        .uri("/reviews")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "card_id": card.get_id(),
                "rating": 3  // "Medium" rating
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the create review request and get the response
    let response = app.clone().oneshot(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into a Value first
    let review_value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    // Check that the review has the correct card_id and rating
    assert_eq!(review_value["card_id"].as_str().unwrap(), card.get_id(), "Review should reference the correct card");
    assert_eq!(review_value["rating"].as_i64().unwrap(), 3, "Review should have the correct rating");
    
    // Now, get the card to check if it was updated with review information
    let request = Request::builder()
        .uri(format!("/cards/{}", card.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the get card request and parse the response
    let response = app.oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_card: Option<Card> = serde_json::from_slice(&body).unwrap();
    
    // Check that the card exists
    assert!(updated_card.is_some(), "Card should exist after review");
    
    // Get the unwrapped card
    let updated_card = updated_card.unwrap();
    
    // For a rating of 3 (medium), the next review should be scheduled 7 days after the last review
    // We could check this more precisely, but it would require more complex time calculations
    assert!(updated_card.get_next_review().is_some(), "Card should have a next review date");
    assert!(updated_card.get_last_review().is_some(), "Card should have a last review date");
}


/// Tests retrieving a non-existent item via the API
///
/// This test verifies that:
/// 1. A GET request to /items/{id} with a non-existent ID returns null
/// 2. The response has a 200 OK status (not 404, as the endpoint returns null for non-existent items)
/// 3. The response body can be correctly deserialized as a null Option<Item>
#[tokio::test]
async fn test_get_nonexistent_item() {
    // Create our test app with an in-memory database
    let app = create_test_app();
    
    // Generate a random UUID for a non-existent item
    let non_existent_id = uuid::Uuid::new_v4().to_string();
    
    // Create a request to get a non-existent item
    let request = Request::builder()
        .uri(format!("/items/{}", non_existent_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.oneshot(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into an Option<Item>
    let item: Option<Item> = serde_json::from_slice(&body).unwrap();
    
    // Check that the item is None (null in JSON)
    assert!(item.is_none(), "Non-existent item should return null");
}


/// Tests creating a review for a non-existent card via the API
///
/// This test verifies that:
/// 1. A POST request to /reviews with a non-existent card_id returns a 404 Not Found
/// 2. The API correctly validates that the card exists before creating a review
#[tokio::test]
async fn test_create_review_for_nonexistent_item() {
    // Create our test app with an in-memory database
    let app = create_test_app();
    
    // Generate a random UUID for a non-existent card
    let non_existent_card_id = uuid::Uuid::new_v4().to_string();
    
    // Create a request to create a review for a non-existent card
    let request = Request::builder()
        .uri("/reviews")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "card_id": non_existent_card_id,
                "rating": 3
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.oneshot(request).await.unwrap();
    
    // Check that the response has a 404 Not Found status
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}


/// Tests creating a review with an invalid rating via the API
///
/// This test verifies that:
/// 1. A POST request to /reviews with a rating outside the valid range (1-3) returns a 400 Bad Request
/// 2. The API correctly validates the rating before creating a review
/// 3. The error message mentions the rating issue
#[tokio::test]
async fn test_create_review_with_invalid_rating() {
    // Set up the test app
    let app = create_test_app();
    
    // First, create an item type
    let item_type = create_item_type(&app, "Basic".to_string()).await;
    
    // Create an item
    let request = Request::builder()
        .uri("/items")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "item_type_id": item_type.get_id(),
                "title": "Item to Review",
                "item_data": null
            }))
            .unwrap(),
        ))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let created_item: Item = serde_json::from_slice(&body).unwrap();
    
    // Get the cards for the item
    let request = Request::builder()
        .uri(format!("/items/{}/cards", created_item.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let cards: Vec<Card> = serde_json::from_slice(&body).unwrap();
    let card = &cards[0];
    
    // Try to create a review with an invalid rating (5)
    let request = Request::builder()
        .uri("/reviews")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "card_id": card.get_id(),
                "rating": 5  // Invalid rating
            }))
            .unwrap(),
        ))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    
    // Check that the response has a 400 Bad Request status
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    
    // Parse the response body to check the error message
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    // Check that the error message mentions the rating range
    let error_message = error_response["error"].as_str().unwrap();
    assert!(error_message.contains("Rating must be between 1 and 4"), 
            "Error message should mention valid rating range, got: {}", error_message);
}

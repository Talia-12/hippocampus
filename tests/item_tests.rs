/// Integration tests for item functionality
///
/// This file contains tests for basic item operations:
/// - Creating items
/// - Getting items by ID
/// - Listing all items
/// - Handling non-existent items

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::Service;
use hippocampus::models::Item;

mod common;
use common::*;

/// Tests creating a new item via the API
///
/// This test verifies:
/// 1. A POST request to /items with a JSON payload creates a new item
/// 2. The response has a 200 OK status
/// 3. The response body contains the created item with the correct title
/// 4. The item is assigned a unique ID
#[tokio::test]
async fn test_create_item() {
    // Create our test app
    let mut app = create_test_app();
    
    // First create an item type
    let item_type = create_item_type(&mut app, "Test Item Type".to_string()).await;
    
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
    let response = app.call(request).await.unwrap();
    
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
/// This test verifies:
/// 1. A GET request to /items/{id} returns the correct item
/// 2. The response has a 200 OK status
/// 3. The response body contains the item with the correct title
/// 4. The item can be correctly deserialized from JSON
#[tokio::test]
async fn test_get_item() {
    // Create our test app
    let mut app = create_test_app();
    
    // First create an item type
    let item_type = create_item_type(&mut app, "Test Item Type".to_string()).await;
    
    // Create an item
    let item = create_item(
        &mut app, 
        &item_type.get_id(), 
        "Test Item for Get".to_string(),
        None
    ).await;
    
    // Now, create a request to get the item by its ID
    let request = Request::builder()
        .uri(format!("/items/{}", item.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the get request and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into an Option<Item>
    // (The API returns null if the item doesn't exist)
    let retrieved_item: Option<Item> = serde_json::from_slice(&body).unwrap();
    
    // Check that the item exists
    assert!(retrieved_item.is_some(), "Item should exist");
    
    // Check that the retrieved item has the correct title
    assert_eq!(retrieved_item.unwrap().get_title(), "Test Item for Get");
}

/// Tests listing all items via the API
///
/// This test verifies:
/// 1. A GET request to /items returns all items
/// 2. The response has a 200 OK status
/// 3. The response body contains a list of items
/// 4. The list includes all items that were created
#[tokio::test]
async fn test_list_items() {
    // Create our test app
    let mut app = create_test_app();
    
    // First create an item type
    let item_type = create_item_type(&mut app, "Test Item Type".to_string()).await;
    
    // Create several items to populate the database
    let mut created_items = Vec::new();
    for i in 1..=3 {
        let item = create_item(
            &mut app, 
            &item_type.get_id(), 
            format!("Test Item {}", i),
            None
        ).await;
        created_items.push(item);
    }
    
    // Now, create a request to list all items
    let request = Request::builder()
        .uri("/items")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the list request and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into a vector of Items
    let items: Vec<Item> = serde_json::from_slice(&body).unwrap();
    
    // Check that we have at least the 3 items we created
    // (There might be more if other tests ran before this one)
    assert!(items.len() >= 3, "Should have at least 3 items");
    
    // Check that all our created items are in the list
    for created_item in &created_items {
        let found = items.iter().any(|item| item.get_id() == created_item.get_id());
        assert!(found, "Created item with ID {} not found in list", created_item.get_id());
    }
}

/// Tests retrieving a non-existent item via the API
///
/// This test verifies:
/// 1. A GET request to /items/{id} with a non-existent ID returns null
/// 2. The response has a 200 OK status (not 404, as the endpoint returns null for non-existent items)
/// 3. The response body can be correctly deserialized as a null Option<Item>
#[tokio::test]
async fn test_get_nonexistent_item() {
    // Create our test app
    let mut app = create_test_app();
    
    // Generate a random UUID for a non-existent item
    let non_existent_id = uuid::Uuid::new_v4().to_string();
    
    // Create a request to get a non-existent item
    let request = Request::builder()
        .uri(format!("/items/{}", non_existent_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request to the application and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into an Option<Item>
    let item: Option<Item> = serde_json::from_slice(&body).unwrap();
    
    // Check that the item is None (null in JSON)
    assert!(item.is_none(), "Non-existent item should return null");
} 
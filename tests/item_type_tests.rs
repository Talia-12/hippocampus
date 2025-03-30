/// Integration tests for item type functionality
///
/// This file contains tests for item type operations:
/// - Creating item types
/// - Getting item types by ID
/// - Listing all item types
/// - Getting items by item type

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::{Service, ServiceExt};
use hippocampus::models::{Item, ItemType};

mod common;
use common::*;

/// Tests creating a new item type via the API
///
/// This test verifies:
/// 1. A POST request to /item_types with a JSON payload creates a new item type
/// 2. The response has a 200 OK status
/// 3. The response body contains the created item type with the correct name
/// 4. The item type is assigned a unique ID
#[tokio::test]
async fn test_create_item_type() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create a request to create an item type
    let request = Request::builder()
        .uri("/item_types")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "name": "Test Item Type"
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
    
    // Parse the body as JSON to check fields
    let item_type: Value = serde_json::from_slice(&body).unwrap();
    
    // Check that the item type has the correct name
    assert_eq!(item_type["name"].as_str().unwrap(), "Test Item Type");
    
    // The ID should be a non-empty string
    assert!(item_type["id"].as_str().unwrap().len() > 0);
    
    // Should have a created_at timestamp
    assert!(item_type["created_at"].is_string());
}

/// Tests retrieving a specific item type by ID via the API
///
/// This test verifies:
/// 1. A GET request to /item_types/{id} returns the correct item type
/// 2. The response has a 200 OK status
/// 3. The response body contains the item type with the correct name
#[tokio::test]
async fn test_get_item_type() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Test Item Type for Get".to_string()).await;
    
    // Create a request to get the item type by ID
    let request = Request::builder()
        .uri(format!("/item_types/{}", item_type.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into a Value
    let retrieved_item_type: Value = serde_json::from_slice(&body).unwrap();
    
    // Check that the item type has the correct ID and name
    assert_eq!(retrieved_item_type["id"].as_str().unwrap(), item_type.get_id());
    assert_eq!(retrieved_item_type["name"].as_str().unwrap(), "Test Item Type for Get");
}

/// Tests listing all item types via the API
///
/// This test verifies:
/// 1. A GET request to /item_types returns all item types
/// 2. The response has a 200 OK status
/// 3. The response body contains a list of item types
/// 4. The list includes the item types that were created
#[tokio::test]
async fn test_list_item_types() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Test Item Type for List".to_string()).await;
    
    // Create a request to list all item types
    let request = Request::builder()
        .uri("/item_types")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into a vector of Values
    let item_types: Vec<Value> = serde_json::from_slice(&body).unwrap();
    
    // Check that at least one item type exists (the one we created)
    assert!(!item_types.is_empty(), "Should have at least one item type");
    
    // Check that our created item type is in the list
    let found = item_types.iter().any(|it| {
        it["id"].as_str().unwrap() == item_type.get_id()
    });
    assert!(found, "Created item type should be in the list");
}

/// Tests retrieving a non-existent item type via the API
///
/// This test verifies:
/// 1. A GET request to /item_types/{id} with a non-existent ID returns a 404
/// 2. The API correctly validates that the item type exists
#[tokio::test]
async fn test_get_nonexistent_item_type() {
    // Create our test app
    let mut app = create_test_app();
    
    // Generate a random UUID for a non-existent item type
    let non_existent_id = uuid::Uuid::new_v4().to_string();
    
    // Create a request to get a non-existent item type
    let request = Request::builder()
        .uri(format!("/item_types/{}", non_existent_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 404 Not Found status
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Tests getting all items of a specific item type via the API
///
/// This test verifies:
/// 1. A GET request to /item_types/{id}/items returns all items of that type
/// 2. The response has a 200 OK status
/// 3. The response body contains a list of items
/// 4. All items in the list have the correct item_type_id
#[tokio::test]
async fn test_get_items_by_item_type() {
    // Create our test app
    let mut app = create_test_app();
    
    // Create an item type
    let item_type = create_item_type(&mut app, "Test Item Type for Items".to_string()).await;
    
    // Create several items with the item type
    let mut created_items = Vec::new();
    for i in 1..=3 {
        let item = create_item(
            &mut app,
            &item_type.get_id(),
            format!("Test Item {} for Type", i),
            None
        ).await;
        created_items.push(item);
    }
    
    // Create a request to get all items for the item type
    let request = Request::builder()
        .uri(format!("/item_types/{}/items", item_type.get_id()))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes for parsing
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON into a vector of Items
    let items: Vec<Item> = serde_json::from_slice(&body).unwrap();
    
    // Check that we have at least the 3 items we created
    assert!(items.len() >= 3, "Should have at least 3 items");
    
    // Check that all items have the correct item_type_id
    for item in &items {
        assert_eq!(item.get_item_type(), item_type.get_id());
    }
    
    // Check that all our created items are in the list
    for created_item in &created_items {
        let found = items.iter().any(|item| item.get_id() == created_item.get_id());
        assert!(found, "Created item with ID {} not found in list", created_item.get_id());
    }
} 
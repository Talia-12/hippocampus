/// Common test utilities for Hippocampus integration tests
///
/// This file contains shared functions and utilities for all integration tests,
/// including test application setup, helper functions for creating common test objects,
/// and other shared functionality.

use hippocampus::{
    create_app,
    db::init_pool,
    models::{Card, Item, ItemType, Tag},
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::Service;
use chrono;

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
pub fn create_test_app() -> Router {
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
pub async fn create_item_type(app: &mut Router, name: String) -> ItemType {
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
    let response = app.call(request).await.unwrap();
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


/// Creates a tag via the API
///
/// This helper function:
/// 1. Sends a POST request to /tags with the provided name
/// 2. Verifies the response has a 200 OK status
/// 3. Parses and returns the created tag ID
///
/// ### Arguments
///
/// * `app` - The test application
/// * `name` - The name for the new tag
///
/// ### Returns
///
/// The ID of the created tag
pub async fn create_tag(app: &mut Router, name: String) -> Tag {
    create_tag_with_visibility(app, name, true).await
}


/// Creates a tag with specified visibility via the API
///
/// This enhanced version of create_tag allows specifying whether the tag is visible.
///
/// ### Arguments
///
/// * `app` - The test application
/// * `name` - The name for the new tag
/// * `visible` - Whether the tag should be visible
///
/// ### Returns
///
/// The created Tag with its ID, name and visibility
pub async fn create_tag_with_visibility(app: &mut Router, name: String, visible: bool) -> Tag {
    // Create a request to create a tag
    let request = Request::builder()
        .uri("/tags")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "name": name,
                "visible": visible
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse the response body
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let tag_value: Value = serde_json::from_slice(&body).unwrap();
    
    // Extract the tag data
    let tag_id = tag_value["id"].as_str().unwrap().to_string();
    let tag_name = tag_value["name"].as_str().unwrap().to_string();
    let tag_visible = tag_value["visible"].as_bool().unwrap();
    let created_at = chrono::NaiveDateTime::parse_from_str(
        tag_value["created_at"].as_str().unwrap(),
        "%Y-%m-%dT%H:%M:%S%.f"
    ).unwrap().and_utc();
    
    // Return a Tag struct
    Tag::new_with_fields(tag_id, tag_name, tag_visible, created_at)
}

/// Creates an item via the API
///
/// This helper function:
/// 1. Sends a POST request to /items with the provided title and type
/// 2. Verifies the response has a 200 OK status
/// 3. Parses and returns the created Item
///
/// ### Arguments
///
/// * `app` - The test application
/// * `item_type_id` - The ID of the item type
/// * `title` - The title for the new item
/// * `item_data` - Optional JSON data for the item
///
/// ### Returns
///
/// The created Item with its ID and fields
pub async fn create_item(
    app: &mut Router, 
    item_type_id: &str, 
    title: String, 
    item_data: Option<serde_json::Value>
) -> Item {
    // Create a request to create an item
    let request = Request::builder()
        .uri("/items")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "item_type_id": item_type_id,
                "title": title,
                "item_data": item_data
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse the response body
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let item: Item = serde_json::from_slice(&body).unwrap();
    
    item
}


/// Gets the cards for an item via the API
///
/// This helper function:
/// 1. Sends a GET request to /items/{item_id}/cards
/// 2. Verifies the response has a 200 OK status
/// 3. Parses and returns the cards
///
/// ### Arguments
///
/// * `app` - The test application
/// * `item_id` - The ID of the item
///
/// ### Returns
///
/// A vector of Cards associated with the item
pub async fn get_cards_for_item(app: &mut Router, item_id: &str) -> Vec<Card> {
    // Create a request to get cards for the item
    let request = Request::builder()
        .uri(format!("/items/{}/cards", item_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse the response body
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let cards: Vec<Card> = serde_json::from_slice(&body).unwrap();
    
    cards
}


/// Creates a card for an item via the API
///
/// This helper function:
/// 1. Sends a POST request to /items/{item_id}/cards
/// 2. Verifies the response has a 200 OK status
/// 3. Parses and returns the created Card
///
/// ### Arguments
///
/// * `app` - The test application
/// * `item_id` - The ID of the item
/// * `card_index` - The index of the card
/// * `priority` - The priority of the card (0.0 to 1.0)
///
/// ### Returns
///
/// The created Card with its ID and fields
pub async fn create_card(
    app: &mut Router,
    item_id: &str,
    card_index: i32,
    priority: f32
) -> Card {
    // Create a request to create a card
    let request = Request::builder()
        .uri(format!("/items/{}/cards", item_id))
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
    
    // Send the request and get the response
    let response = app.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse the response body
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let card: Card = serde_json::from_slice(&body).unwrap();
    
    card
} 
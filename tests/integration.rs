use hippocampus::{
    db::init_pool,
    models::{Item, Review},
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

// Helper function to create a test app with an in-memory SQLite database
fn create_test_app() -> Router {
    let pool = Arc::new(init_pool(":memory:"));
    
    // Run migrations on the in-memory database
    let conn = &mut pool.get().unwrap();
    hippocampus::run_migrations(conn);
    
    hippocampus::create_app(pool)
}

#[allow(unexpected_cfgs)]
#[tokio::test]
async fn test_create_item() {
    // Create our test app
    let app = create_test_app();
    
    // Create a request to create an item
    let request = Request::builder()
        .uri("/items")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "title": "Test Item"
            }))
            .unwrap(),
        ))
        .unwrap();
    
    // Send the request and get a response
    let response = app.oneshot(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON
    let item: Item = serde_json::from_slice(&body).unwrap();
    
    // Check that the item has the correct title
    assert_eq!(item.title, "Test Item");
}

#[allow(unexpected_cfgs)]
#[tokio::test]
async fn test_get_item() {
    // Create our test app
    let app = create_test_app();
    
    // First, create an item
    let request = Request::builder()
        .uri("/items")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "title": "Test Item for Get"
            }))
            .unwrap(),
        ))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let created_item: Item = serde_json::from_slice(&body).unwrap();
    
    // Now, get the item
    let request = Request::builder()
        .uri(format!("/items/{}", created_item.id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON
    let item: Option<Item> = serde_json::from_slice(&body).unwrap();
    
    // Check that the item exists and has the correct title
    assert!(item.is_some());
    assert_eq!(item.unwrap().title, "Test Item for Get");
}

#[allow(unexpected_cfgs)]
#[tokio::test]
async fn test_list_items() {
    // Create our test app
    let app = create_test_app();
    
    // Create a few items
    for i in 1..=3 {
        let request = Request::builder()
            .uri("/items")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({
                    "title": format!("Test Item {}", i)
                }))
                .unwrap(),
            ))
            .unwrap();
        
        let _ = app.clone().oneshot(request).await.unwrap();
    }
    
    // Now, list all items
    let request = Request::builder()
        .uri("/items")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON
    let items: Vec<Item> = serde_json::from_slice(&body).unwrap();
    
    // Check that we have at least 3 items
    assert!(items.len() >= 3);
}

#[allow(unexpected_cfgs)]
#[tokio::test]
async fn test_create_review() {
    // Create our test app
    let app = create_test_app();
    
    // First, create an item
    let request = Request::builder()
        .uri("/items")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "title": "Item to Review"
            }))
            .unwrap(),
        ))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let created_item: Item = serde_json::from_slice(&body).unwrap();
    
    // Now, create a review for the item
    let request = Request::builder()
        .uri("/reviews")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "item_id": created_item.id,
                "rating": 3
            }))
            .unwrap(),
        ))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    
    // Check that the response has a 200 OK status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Convert the response body into bytes
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Parse the body as JSON
    let review: Review = serde_json::from_slice(&body).unwrap();
    
    // Check that the review has the correct item_id and rating
    assert_eq!(review.item_id, created_item.id);
    assert_eq!(review.rating, 3);
    
    // Now, get the item to check if next_review was updated
    let request = Request::builder()
        .uri(format!("/items/{}", created_item.id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_item: Option<Item> = serde_json::from_slice(&body).unwrap();
    
    // Check that the item exists and has a next_review date
    assert!(updated_item.is_some());
    let updated_item = updated_item.unwrap();
    assert!(updated_item.next_review.is_some());
    assert!(updated_item.last_review.is_some());
} 
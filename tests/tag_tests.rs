/// Integration tests for the tags functionality
///
/// This file contains tests for tag operations including:
/// - Creating tags
/// - Listing tags
/// - Adding tags to items
/// - Removing tags from items
/// - Listing tags for items
/// - Listing tags for cards
/// - Error cases
use axum::{
	body::{Body, to_bytes},
	http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::Service;

mod common;
use common::*;

/// Tests creating a new tag via the API
///
/// This test verifies:
/// 1. A POST request to /tags with a valid payload creates a new tag
/// 2. The response has a 200 OK status
/// 3. The response body contains the tag with the correct name and visibility
#[tokio::test]
async fn test_create_tag() {
	// Create our test app
	let mut app = create_test_app();

	// Create a request to create a tag
	let request = Request::builder()
		.uri("/tags")
		.method("POST")
		.header("Content-Type", "application/json")
		.body(Body::from(
			serde_json::to_string(&json!({
				"name": "Important",
				"visible": true
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
	let tag: Value = serde_json::from_slice(&body).unwrap();

	// Check that the tag has the correct fields
	assert_eq!(tag["name"], "Important");
	assert_eq!(tag["visible"], true);
	assert!(tag["id"].is_string());
	assert!(tag["created_at"].is_string());
}

/// Tests creating a tag with visibility set to false
///
/// This test verifies:
/// 1. A POST request to /tags can create an invisible tag
/// 2. The visibility field is correctly set to false in the response
#[tokio::test]
async fn test_create_invisible_tag() {
	// Create our test app
	let mut app = create_test_app();

	// Create a request to create an invisible tag
	let request = Request::builder()
		.uri("/tags")
		.method("POST")
		.header("Content-Type", "application/json")
		.body(Body::from(
			serde_json::to_string(&json!({
				"name": "System",
				"visible": false
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
	let tag: Value = serde_json::from_slice(&body).unwrap();

	// Check that the tag has the correct visibility
	assert_eq!(tag["name"], "System");
	assert_eq!(tag["visible"], false);
}

/// Tests listing all tags via the API
///
/// This test verifies:
/// 1. A GET request to /tags returns all tags
/// 2. The response has a 200 OK status
/// 3. The response body contains a list with all the created tags
#[tokio::test]
async fn test_list_tags() {
	// Create our test app
	let mut app = create_test_app();

	// Create some tags
	let tag1 = create_tag(&mut app, "Important".to_string()).await;
	let tag2 = create_tag(&mut app, "Difficult".to_string()).await;

	// Create a request to list all tags
	let request = Request::builder()
		.uri("/tags")
		.method("GET")
		.body(Body::empty())
		.unwrap();

	// Send the request to the application and get the response
	let response = app.call(request).await.unwrap();

	// Check that the response has a 200 OK status
	assert_eq!(response.status(), StatusCode::OK);

	// Parse the response body
	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let tags: Vec<Value> = serde_json::from_slice(&body).unwrap();

	// Check that the list contains at least the two tags we created
	assert!(tags.len() >= 2);

	// Check that our tags are in the list
	let tag1_found = tags
		.iter()
		.any(|t| t["id"].as_str().unwrap() == tag1.get_id());
	let tag2_found = tags
		.iter()
		.any(|t| t["id"].as_str().unwrap() == tag2.get_id());

	assert!(tag1_found, "Tag 'Important' should be in the list");
	assert!(tag2_found, "Tag 'Difficult' should be in the list");
}

/// Tests adding a tag to an item via the API
///
/// This test verifies:
/// 1. A PUT request to /items/{item_id}/tags/{tag_id} adds the tag to the item
/// 2. The tag appears when listing tags for the item
#[tokio::test]
async fn test_add_tag_to_item() {
	// Create our test app
	let mut app = create_test_app();

	// Create an item type
	let item_type = create_item_type(&mut app, "Basic".to_string()).await;

	// Create an item
	let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;

	// Create a tag
	let tag = create_tag(&mut app, "Important".to_string()).await;

	// Create a request to add the tag to the item
	let request = Request::builder()
		.uri(format!("/items/{}/tags/{}", item.get_id(), tag.get_id()))
		.method("POST")
		.body(Body::empty())
		.unwrap();

	// Send the request to the application and get the response
	let response = app.call(request).await.unwrap();

	// Check that the response has a 200 OK status (or 204 No Content)
	assert!(response.status().is_success());

	// Now check that the tag is associated with the item
	let request = Request::builder()
		.uri(format!("/items/{}/tags", item.get_id()))
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::OK);

	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let tags: Vec<Value> = serde_json::from_slice(&body).unwrap();

	// Check that our tag is in the list
	assert_eq!(tags.len(), 1);
	assert_eq!(tags[0]["id"].as_str().unwrap(), tag.get_id());
}

/// Tests removing a tag from an item via the API
///
/// This test verifies:
/// 1. A DELETE request to /items/{item_id}/tags/{tag_id} removes the tag from the item
/// 2. The tag no longer appears when listing tags for the item
#[tokio::test]
async fn test_remove_tag_from_item() {
	// Create our test app
	let mut app = create_test_app();

	// Create an item type
	let item_type = create_item_type(&mut app, "Basic".to_string()).await;

	// Create an item
	let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;

	// Create a tag
	let tag = create_tag(&mut app, "Important".to_string()).await;

	// First add the tag to the item
	let request = Request::builder()
		.uri(format!("/items/{}/tags/{}", item.get_id(), tag.get_id()))
		.method("POST")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert!(response.status().is_success());

	// Now remove the tag from the item
	let request = Request::builder()
		.uri(format!("/items/{}/tags/{}", item.get_id(), tag.get_id()))
		.method("DELETE")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert!(response.status().is_success());

	// Check that the tag is no longer associated with the item
	let request = Request::builder()
		.uri(format!("/items/{}/tags", item.get_id()))
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::OK);

	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let tags: Vec<Value> = serde_json::from_slice(&body).unwrap();

	// Check that the item has no tags
	assert_eq!(tags.len(), 0);
}

/// Tests listing tags for an item via the API
///
/// This test verifies:
/// 1. A GET request to /items/{item_id}/tags returns all tags for the item
/// 2. The response has a 200 OK status
/// 3. The response body contains only the tags associated with the item
#[tokio::test]
async fn test_list_tags_for_item() {
	// Create our test app
	let mut app = create_test_app();

	// Create an item type
	let item_type = create_item_type(&mut app, "Basic".to_string()).await;

	// Create two items
	let item1 = create_item(&mut app, &item_type.get_id(), "Item 1".to_string(), None).await;
	let item2 = create_item(&mut app, &item_type.get_id(), "Item 2".to_string(), None).await;

	// Create two tags
	let tag1 = create_tag(&mut app, "Important".to_string()).await;
	let tag2 = create_tag(&mut app, "Difficult".to_string()).await;

	// Add both tags to item1
	let request = Request::builder()
		.uri(format!("/items/{}/tags/{}", item1.get_id(), tag1.get_id()))
		.method("POST")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	assert!(response.status().is_success());

	let request = Request::builder()
		.uri(format!("/items/{}/tags/{}", item1.get_id(), tag2.get_id()))
		.method("POST")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	assert!(response.status().is_success());

	// Add only tag1 to item2
	let request = Request::builder()
		.uri(format!("/items/{}/tags/{}", item2.get_id(), tag1.get_id()))
		.method("POST")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	assert!(response.status().is_success());

	// Check tags for item1
	let request = Request::builder()
		.uri(format!("/items/{}/tags", item1.get_id()))
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::OK);

	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let tags: Vec<Value> = serde_json::from_slice(&body).unwrap();

	// Check that item1 has both tags
	assert_eq!(tags.len(), 2);
	let has_tag1 = tags
		.iter()
		.any(|t| t["id"].as_str().unwrap() == tag1.get_id());
	let has_tag2 = tags
		.iter()
		.any(|t| t["id"].as_str().unwrap() == tag2.get_id());
	assert!(has_tag1, "Item 1 should have tag 'Important'");
	assert!(has_tag2, "Item 1 should have tag 'Difficult'");

	// Check tags for item2
	let request = Request::builder()
		.uri(format!("/items/{}/tags", item2.get_id()))
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::OK);

	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let tags: Vec<Value> = serde_json::from_slice(&body).unwrap();

	// Check that item2 has only tag1
	assert_eq!(tags.len(), 1);
	assert_eq!(tags[0]["id"].as_str().unwrap(), tag1.get_id());
}

/// Tests listing tags for a card via the API
///
/// This test verifies:
/// 1. A GET request to /cards/{card_id}/tags returns all tags for the card
/// 2. The response has a 200 OK status
/// 3. The tags for a card match the tags of its parent item
#[tokio::test]
async fn test_list_tags_for_card() {
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

	// Create two tags
	let tag1 = create_tag(&mut app, "Important".to_string()).await;
	let tag2 = create_tag(&mut app, "Difficult".to_string()).await;

	// Add both tags to the item
	let request = Request::builder()
		.uri(format!("/items/{}/tags/{}", item.get_id(), tag1.get_id()))
		.method("POST")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	assert!(response.status().is_success());

	let request = Request::builder()
		.uri(format!("/items/{}/tags/{}", item.get_id(), tag2.get_id()))
		.method("POST")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	assert!(response.status().is_success());

	// Check tags for the card
	let request = Request::builder()
		.uri(format!("/cards/{}/tags", card.get_id()))
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::OK);

	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let tags: Vec<Value> = serde_json::from_slice(&body).unwrap();

	// Check that the card has both tags (same as its parent item)
	assert_eq!(tags.len(), 2);
	let has_tag1 = tags
		.iter()
		.any(|t| t["id"].as_str().unwrap() == tag1.get_id());
	let has_tag2 = tags
		.iter()
		.any(|t| t["id"].as_str().unwrap() == tag2.get_id());
	assert!(has_tag1, "Card should have tag 'Important'");
	assert!(has_tag2, "Card should have tag 'Difficult'");
}

/// Tests error case: adding a tag to a non-existent item
///
/// This test verifies that attempting to add a tag to a non-existent item
/// results in a 404 Not Found response
#[tokio::test]
async fn test_add_tag_to_nonexistent_item() {
	// Create our test app
	let mut app = create_test_app();

	// Create a tag
	let tag = create_tag(&mut app, "Important".to_string()).await;

	// Try to add the tag to a non-existent item
	let request = Request::builder()
		.uri(format!("/items/nonexistent-item-id/tags/{}", tag.get_id()))
		.method("POST")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Tests error case: adding a non-existent tag to an item
///
/// This test verifies that attempting to add a non-existent tag to an item
/// results in a 404 Not Found response
#[tokio::test]
async fn test_add_nonexistent_tag_to_item() {
	// Create our test app
	let mut app = create_test_app();

	// Create an item type
	let item_type = create_item_type(&mut app, "Basic".to_string()).await;

	// Create an item
	let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;

	// Try to add a non-existent tag to the item
	let request = Request::builder()
		.uri(format!("/items/{}/tags/nonexistent-tag-id", item.get_id()))
		.method("POST")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Tests error case: removing a tag from a non-existent item
///
/// This test verifies that attempting to remove a tag from a non-existent item
/// results in a 404 Not Found response
#[tokio::test]
async fn test_remove_tag_from_nonexistent_item() {
	// Create our test app
	let mut app = create_test_app();

	// Create a tag
	let tag = create_tag(&mut app, "Important".to_string()).await;

	// Try to remove the tag from a non-existent item
	let request = Request::builder()
		.uri(format!("/items/nonexistent-item-id/tags/{}", tag.get_id()))
		.method("DELETE")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(
		response.status(),
		StatusCode::NOT_FOUND,
		"Expected 404 Not Found (error message: {:?})",
		axum::body::to_bytes(response.into_body(), usize::MAX)
			.await
			.unwrap()
	);
}

/// Tests error case: removing a non-existent tag from an item
///
/// This test verifies that attempting to remove a non-existent tag from an item
/// results in a 404 Not Found response
#[tokio::test]
async fn test_remove_nonexistent_tag_from_item() {
	// Create our test app
	let mut app = create_test_app();

	// Create an item type
	let item_type = create_item_type(&mut app, "Basic".to_string()).await;

	// Create an item
	let item = create_item(&mut app, &item_type.get_id(), "Test Item".to_string(), None).await;

	// Try to remove a non-existent tag from the item
	let request = Request::builder()
		.uri(format!("/items/{}/tags/nonexistent-tag-id", item.get_id()))
		.method("DELETE")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Tests error case: listing tags for a non-existent item
///
/// This test verifies that attempting to list tags for a non-existent item
/// results in a 404 Not Found response
#[tokio::test]
async fn test_list_tags_for_nonexistent_item() {
	// Create our test app
	let mut app = create_test_app();

	// Try to list tags for a non-existent item
	let request = Request::builder()
		.uri("/items/nonexistent-item-id/tags")
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Tests filtering cards by a single tag_ids query parameter
///
/// This test verifies that GET /cards?tag_ids=<id> correctly filters
/// cards to only those whose parent item has the specified tag.
/// This is a regression test for the axum_extra::extract::Query fix.
#[tokio::test]
async fn test_list_cards_filtered_by_tag_id() {
	let mut app = create_test_app();

	// Create an item type and two items
	let item_type = create_item_type(&mut app, "Basic".to_string()).await;
	let item1 = create_item(&mut app, &item_type.get_id(), "Item 1".to_string(), None).await;
	let item2 = create_item(&mut app, &item_type.get_id(), "Item 2".to_string(), None).await;

	// Create a tag and add it only to item1
	let tag = create_tag(&mut app, "Special".to_string()).await;
	let request = Request::builder()
		.uri(format!("/items/{}/tags/{}", item1.get_id(), tag.get_id()))
		.method("POST")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	assert!(response.status().is_success());

	// Query cards filtered by that tag
	let request = Request::builder()
		.uri(format!("/cards?tag_ids={}", tag.get_id()))
		.method("GET")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::OK);

	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let cards: Vec<Value> = serde_json::from_slice(&body).unwrap();

	// All returned cards should belong to item1
	assert!(!cards.is_empty(), "Should return cards for the tagged item");
	for card in &cards {
		assert_eq!(
			card["item_id"].as_str().unwrap(),
			item1.get_id(),
			"All cards should belong to item1"
		);
	}

	// Verify item2's cards are not included
	let item2_cards = get_cards_for_item(&mut app, &item2.get_id()).await;
	for item2_card in &item2_cards {
		assert!(
			!cards
				.iter()
				.any(|c| c["id"].as_str().unwrap() == item2_card.get_id()),
			"Item2's cards should not appear in tag-filtered results"
		);
	}
}

/// Tests filtering items by multiple tag_ids query parameters
///
/// This test verifies that GET /items?tag_ids=<id1>&tag_ids=<id2> correctly
/// deserializes repeated query params into a Vec and filters items accordingly.
/// The filter uses intersection semantics: items must have ALL specified tags.
#[tokio::test]
async fn test_list_items_filtered_by_multiple_tag_ids() {
	let mut app = create_test_app();

	// Create an item type and three items
	let item_type = create_item_type(&mut app, "Basic".to_string()).await;
	let item1 = create_item(&mut app, &item_type.get_id(), "Item 1".to_string(), None).await;
	let item2 = create_item(&mut app, &item_type.get_id(), "Item 2".to_string(), None).await;
	let item3 = create_item(&mut app, &item_type.get_id(), "Item 3".to_string(), None).await;

	// Create two tags
	let tag_a = create_tag(&mut app, "TagA".to_string()).await;
	let tag_b = create_tag(&mut app, "TagB".to_string()).await;

	// item1 gets both tags, item2 gets only tag_a, item3 gets neither
	for (item_id, tag_id) in [
		(item1.get_id(), tag_a.get_id()),
		(item1.get_id(), tag_b.get_id()),
		(item2.get_id(), tag_a.get_id()),
	] {
		let request = Request::builder()
			.uri(format!("/items/{}/tags/{}", item_id, tag_id))
			.method("POST")
			.body(Body::empty())
			.unwrap();
		let response = app.call(request).await.unwrap();
		assert!(response.status().is_success());
	}

	// Query items filtered by both tags (intersection: must have ALL)
	let request = Request::builder()
		.uri(format!(
			"/items?tag_ids={}&tag_ids={}",
			tag_a.get_id(),
			tag_b.get_id()
		))
		.method("GET")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::OK);

	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let items: Vec<Value> = serde_json::from_slice(&body).unwrap();

	// Only item1 has both tags
	let item_ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
	assert!(
		item_ids.contains(&item1.get_id().as_str()),
		"Item1 (has both tags) should be in results"
	);
	assert!(
		!item_ids.contains(&item2.get_id().as_str()),
		"Item2 (only TagA) should not be in results"
	);
	assert!(
		!item_ids.contains(&item3.get_id().as_str()),
		"Item3 (no tags) should not be in results"
	);
}

/// Tests error case: listing tags for a non-existent card
///
/// This test verifies that attempting to list tags for a non-existent card
/// results in a 404 Not Found response
#[tokio::test]
async fn test_list_tags_for_nonexistent_card() {
	// Create our test app
	let mut app = create_test_app();

	// Try to list tags for a non-existent card
	let request = Request::builder()
		.uri("/cards/nonexistent-card-id/tags")
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

use super::*;
use crate::repo;
use crate::test_utils::*;
use serde_json::json;

#[tokio::test]
async fn test_create_card_handler() {
	let pool = setup_test_db();

	// First create an item type
	let item_type = repo::create_item_type(&pool, "Test Type 1".to_string(), "fsrs".to_string())
		.await
		.unwrap();

	// Then create an item of that type
	let item = repo::create_item(
		&pool,
		&item_type.get_id(),
		"Test Item".to_string(),
		json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap();

	// Create a payload for the card
	let payload = CreateCardDto {
		card_index: 3,
		priority: 0.5,
	};

	// Call the handler
	let result = create_card_handler(State(pool.clone()), Path(item.get_id()), Json(payload))
		.await
		.unwrap();

	// Check the result
	let card = result.0;
	assert_eq!(card.get_item_id(), item.get_id());
	assert_eq!(card.get_card_index(), 3);
}

#[tokio::test]
async fn test_create_card_handler_not_found() {
	let pool = setup_test_db();

	// Create a payload for the card
	let payload = CreateCardDto {
		card_index: 1,
		priority: 0.5,
	};

	// Call the handler with a non-existent item ID
	let result = create_card_handler(
		State(pool.clone()),
		Path(ItemId("nonexistent".to_string())),
		Json(payload),
	)
	.await;

	// Check that we got a NotFound error
	assert!(result.is_err());
	assert!(matches!(result.unwrap_err(), ApiError::NotFound));
}

#[tokio::test]
async fn test_get_card_handler() {
	let pool = setup_test_db();

	// First create an item type
	let item_type = repo::create_item_type(&pool, "Test Type 1".to_string(), "fsrs".to_string())
		.await
		.unwrap();

	// Then create an item of that type
	let item = repo::create_item(
		&pool,
		&item_type.get_id(),
		"Test Item".to_string(),
		json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap();

	// Create a card for the item
	let card = repo::create_card(&pool, &item.get_id(), 3, 0.5)
		.await
		.unwrap();

	// Call the handler
	let result = get_card_handler(
		State(pool.clone()),
		Path(card.get_id()),
		Query(GetQueryDto::default()),
	)
	.await
	.unwrap();

	// Check the result
	let retrieved_card = &result.0;
	assert!(!retrieved_card.is_null());
	assert_eq!(retrieved_card["id"], card.get_id().0);
	assert_eq!(retrieved_card["item_id"], item.get_id().0);
}

#[tokio::test]
async fn test_get_card_handler_not_found() {
	let pool = setup_test_db();

	// Call the handler with a non-existent card ID
	let result = get_card_handler(
		State(pool.clone()),
		Path(CardId("nonexistent".to_string())),
		Query(GetQueryDto::default()),
	)
	.await
	.unwrap();

	// Check that we got None
	assert!(result.0.is_null());
}

#[tokio::test]
async fn test_list_cards_handler() {
	let pool = setup_test_db();

	// Set up some test data
	let item_type = repo::create_item_type(&pool, "Test Type 1".to_string(), "fsrs".to_string())
		.await
		.unwrap();
	let item = repo::create_item(
		&pool,
		&item_type.get_id(),
		"Test Item".to_string(),
		json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap();

	// Create some cards
	let card1 = repo::create_card(&pool, &item.get_id(), 3, 0.5)
		.await
		.unwrap();
	let card2 = repo::create_card(&pool, &item.get_id(), 4, 0.5)
		.await
		.unwrap();

	// Call the handler with no filters
	let result = list_cards_handler(State(pool.clone()), Query(GetQueryDto::default()))
		.await
		.unwrap();

	// Check the result
	let cards = result.0;
	assert_eq!(cards.len(), 4);
	assert!(cards.iter().any(|c| c["id"] == card1.get_id().0));
	assert!(cards.iter().any(|c| c["id"] == card2.get_id().0));
}

#[tokio::test]
async fn test_list_cards_by_item_handler() {
	let pool = setup_test_db();

	// Set up some test data
	let item_type = repo::create_item_type(&pool, "Test Type 1".to_string(), "fsrs".to_string())
		.await
		.unwrap();
	let item1 = repo::create_item(
		&pool,
		&item_type.get_id(),
		"Item 1".to_string(),
		json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap();

	let item2 = repo::create_item(
		&pool,
		&item_type.get_id(),
		"Item 2".to_string(),
		json!({"front": "Goodbye", "back": "World"}),
	)
	.await
	.unwrap();

	// Create cards for the items
	let card1 = repo::create_card(&pool, &item1.get_id(), 3, 0.5)
		.await
		.unwrap();
	let card2 = repo::create_card(&pool, &item2.get_id(), 3, 0.5)
		.await
		.unwrap();

	// Call the handler
	let result = list_cards_by_item_handler(
		State(pool.clone()),
		Path(item1.get_id()),
		Query(GetQueryDto::default()),
	)
	.await
	.unwrap();

	// Check the result
	let cards = result.0;
	assert_eq!(cards.len(), 3);
	assert!(
		cards.iter().any(|c| c["id"] == card1.get_id().0),
		"item 1's cards not found in list"
	);
	assert!(
		!cards.iter().any(|c| c["id"] == card2.get_id().0),
		"item 2's cards found in list"
	);
}

#[tokio::test]
async fn test_list_cards_by_item_handler_not_found() {
	let pool = setup_test_db();

	// Call the handler with a non-existent item ID
	let result = list_cards_by_item_handler(
		State(pool.clone()),
		Path(ItemId("nonexistent".to_string())),
		Query(GetQueryDto::default()),
	)
	.await;

	// Check that we got a NotFound error
	assert!(result.is_err());
	assert!(matches!(result.unwrap_err(), ApiError::NotFound));
}

#[tokio::test]
async fn test_update_card_priority_handler_success() {
	let pool = setup_test_db();

	// Create test data
	let item_type = repo::create_item_type(&pool, "Test Type 1".to_string(), "fsrs".to_string())
		.await
		.unwrap();
	let item = repo::create_item(
		&pool,
		&item_type.get_id(),
		"Test Item".to_string(),
		json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap();

	// Create a card with initial priority
	let initial_priority = 0.5;
	let card = repo::create_card(&pool, &item.get_id(), 2, initial_priority)
		.await
		.unwrap();

	// Update the card's priority
	let new_priority = 0.8;
	let payload = new_priority;

	let result =
		update_card_priority_handler(State(pool.clone()), Path(card.get_id()), Json(payload))
			.await
			.unwrap();

	// Check the result
	let updated_card = &result.0;
	assert!(
		(updated_card["priority"].as_f64().unwrap() as f32 - new_priority).abs() < 0.0001,
		"Priority not updated correctly, should be {}, but is {}",
		new_priority,
		updated_card["priority"]
	);
}

#[tokio::test]
async fn test_update_card_priority_handler_boundary_values() {
	let pool = setup_test_db();

	// Create test data
	let item_type = repo::create_item_type(&pool, "Test Type 1".to_string(), "fsrs".to_string())
		.await
		.unwrap();
	let item = repo::create_item(
		&pool,
		&item_type.get_id(),
		"Test Item".to_string(),
		json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap();

	let card = repo::create_card(&pool, &item.get_id(), 2, 0.5)
		.await
		.unwrap();

	// Test minimum valid priority (0.0)
	let min_priority = 0.0;
	let payload = min_priority;

	let result =
		update_card_priority_handler(State(pool.clone()), Path(card.get_id()), Json(payload))
			.await
			.unwrap();

	let updated_card = &result.0;
	assert!((updated_card["priority"].as_f64().unwrap() as f32 - min_priority).abs() < 0.0001);

	// Test maximum valid priority (1.0)
	let max_priority = 1.0;
	let payload = max_priority;

	let result =
		update_card_priority_handler(State(pool.clone()), Path(card.get_id()), Json(payload))
			.await
			.unwrap();

	let updated_card = &result.0;
	assert!((updated_card["priority"].as_f64().unwrap() as f32 - max_priority).abs() < 0.0001);
}

#[tokio::test]
async fn test_update_card_priority_handler_invalid_priority_too_low() {
	let pool = setup_test_db();

	// Create test data
	let item_type = repo::create_item_type(&pool, "Test Type 1".to_string(), "fsrs".to_string())
		.await
		.unwrap();
	let item = repo::create_item(
		&pool,
		&item_type.get_id(),
		"Test Item".to_string(),
		json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap();

	let card = repo::create_card(&pool, &item.get_id(), 2, 0.5)
		.await
		.unwrap();

	// Test priority below valid range
	let below_min_priority = -0.1;
	let payload = below_min_priority;

	let result =
		update_card_priority_handler(State(pool.clone()), Path(card.get_id()), Json(payload)).await;

	// Should return an error
	assert!(result.is_err());
	assert!(matches!(result.unwrap_err(), ApiError::InvalidPriority(_)));
}

#[tokio::test]
async fn test_update_card_priority_handler_invalid_priority_too_high() {
	let pool = setup_test_db();

	// Create test data
	let item_type = repo::create_item_type(&pool, "Test Type 1".to_string(), "fsrs".to_string())
		.await
		.unwrap();
	let item = repo::create_item(
		&pool,
		&item_type.get_id(),
		"Test Item".to_string(),
		json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap();

	let card = repo::create_card(&pool, &item.get_id(), 2, 0.5)
		.await
		.unwrap();

	// Test priority above valid range
	let above_max_priority = 1.1;
	let payload = above_max_priority;

	let result =
		update_card_priority_handler(State(pool.clone()), Path(card.get_id()), Json(payload)).await;

	// Should return an error
	assert!(result.is_err());
	assert!(matches!(result.unwrap_err(), ApiError::InvalidPriority(_)));
}

#[tokio::test]
async fn test_update_card_priority_handler_nonexistent_card() {
	let pool = setup_test_db();

	// Try to update a card that doesn't exist
	let nonexistent_card_id = CardId("00000000-0000-0000-0000-000000000000".to_string());
	let payload = 0.5;

	let result = update_card_priority_handler(
		State(pool.clone()),
		Path(nonexistent_card_id),
		Json(payload),
	)
	.await;

	// Should return an error
	assert!(result.is_err());
	assert!(matches!(result.unwrap_err(), ApiError::NotFound));
}

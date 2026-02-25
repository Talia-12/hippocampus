use super::*;
use crate::repo::tests::setup_test_db;
use crate::repo::create_item_type;
use serde_json::json;

#[tokio::test]
async fn test_create_item() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item of that type
    let title = "Example Item".to_string();
    let data = json!({
        "front": "Hello",
        "back": "World"
    });

    let item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();

    assert_eq!(item.get_title(), title);
    assert_eq!(item.get_item_type(), item_type.get_id());
    assert_eq!(item.get_data().0, data);
}

#[tokio::test]
async fn test_get_item() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item
    let title = "Example Item".to_string();
    let data = json!({
        "front": "Hello",
        "back": "World"
    });

    let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();

    // Retrieve the item
    let retrieved_item = get_item(&pool, &created_item.get_id()).unwrap().unwrap();

    assert_eq!(retrieved_item.get_id(), created_item.get_id());
    assert_eq!(retrieved_item.get_title(), title);
    assert_eq!(retrieved_item.get_item_type(), item_type.get_id());
    assert_eq!(retrieved_item.get_data().0, data);
}

#[tokio::test]
async fn test_get_nonexistent_item() {
    let pool = setup_test_db();

    // Try to retrieve a non-existent item
    let result = get_item(&pool, "nonexistent-id").unwrap();

    assert!(result.is_none());
}

#[tokio::test]
async fn test_list_items() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create some items
    let item1 = create_item(
        &pool,
        &item_type.get_id(),
        "Item 1".to_string(),
        json!({"front": "F1", "back": "B1"})
    ).await.unwrap();

    let item2 = create_item(
        &pool,
        &item_type.get_id(),
        "Item 2".to_string(),
        json!({"front": "F2", "back": "B2"})
    ).await.unwrap();

    // List all items
    let items = list_items(&pool).unwrap();

    // Verify that the list contains the created items
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|i| i.get_id() == item1.get_id()));
    assert!(items.iter().any(|i| i.get_id() == item2.get_id()));
}

#[tokio::test]
async fn test_get_items_by_type() {
    let pool = setup_test_db();

    // Create two item types
    let vocab_type = create_item_type(&pool, "Test Type 1".to_string(), "fsrs".to_string()).await.unwrap();
    let grammar_type = create_item_type(&pool, "Test Type 2".to_string(), "fsrs".to_string()).await.unwrap();

    // Create items of different types
    let vocab_item = create_item(
        &pool,
        &vocab_type.get_id(),
        "Vocab Item".to_string(),
        json!({"front": "F1", "back": "B1"})
    ).await.unwrap();

    let grammar_item = create_item(
        &pool,
        &grammar_type.get_id(),
        "Grammar Item".to_string(),
        json!({"front": "F2", "back": "B2"})
    ).await.unwrap();

    // Get items by type
    let vocab_items = get_items_by_type(&pool, &vocab_type.get_id()).unwrap();
    let grammar_items = get_items_by_type(&pool, &grammar_type.get_id()).unwrap();

    // Verify that the lists contain the correct items
    assert_eq!(vocab_items.len(), 1);
    assert_eq!(vocab_items[0].get_id(), vocab_item.get_id());

    assert_eq!(grammar_items.len(), 1);
    assert_eq!(grammar_items[0].get_id(), grammar_item.get_id());
}


#[tokio::test]
async fn test_create_item_with_data() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item with complex JSON data
    let data = json!({
        "front": {
            "text": "Hello",
            "image_url": "https://example.com/hello.jpg",
            "audio_url": "https://example.com/hello.mp3"
        },
        "back": {
            "text": "World",
            "examples": [
                "Hello, world!",
                "Hello there, friend."
            ],
            "notes": "A common greeting."
        }
    });

    let item = create_item(&pool, &item_type.get_id(), "Complex Item".to_string(), data.clone()).await.unwrap();

    // Retrieve the item
    let retrieved_item = get_item(&pool, &item.get_id()).unwrap().unwrap();

    // Verify that the complex data was stored and retrieved correctly
    assert_eq!(retrieved_item.get_data().0, data);
}


#[tokio::test]
async fn test_update_item_title() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item
    let title = "Original Title".to_string();
    let data = json!({
        "front": "Hello",
        "back": "World"
    });

    let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();

    // Update only the title
    let new_title = "Updated Title".to_string();
    let updated_item = update_item(&pool, &created_item.get_id(), Some(new_title.clone()), None).await.unwrap();

    // Verify that the title was updated but the data remained the same
    assert_eq!(updated_item.get_title(), new_title);
    assert_eq!(updated_item.get_data().0, data);
    assert_eq!(updated_item.get_id(), created_item.get_id());
    assert_eq!(updated_item.get_item_type(), item_type.get_id());
}


#[tokio::test]
async fn test_update_item_data() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item
    let title = "Original Title".to_string();
    let data = json!({
        "front": "Hello",
        "back": "World"
    });

    let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();

    // Update only the data
    let new_data = json!({
        "front": "Bonjour",
        "back": "Monde"
    });

    let updated_item = update_item(&pool, &created_item.get_id(), None, Some(new_data.clone())).await.unwrap();

    // Verify that the data was updated but the title remained the same
    assert_eq!(updated_item.get_title(), title);
    assert_eq!(updated_item.get_data().0, new_data);
    assert_eq!(updated_item.get_id(), created_item.get_id());
    assert_eq!(updated_item.get_item_type(), item_type.get_id());
}


#[tokio::test]
async fn test_update_item_both_fields() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item
    let title = "Original Title".to_string();
    let data = json!({
        "front": "Hello",
        "back": "World"
    });

    let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();

    // Update both title and data
    let new_title = "Updated Title".to_string();
    let new_data = json!({
        "front": "Hola",
        "back": "Mundo",
        "notes": "Spanish greeting"
    });

    let updated_item = update_item(
        &pool,
        &created_item.get_id(),
        Some(new_title.clone()),
        Some(new_data.clone())
    ).await.unwrap();

    // Verify that both title and data were updated
    assert_eq!(updated_item.get_title(), new_title);
    assert_eq!(updated_item.get_data().0, new_data);
    assert_eq!(updated_item.get_id(), created_item.get_id());
    assert_eq!(updated_item.get_item_type(), item_type.get_id());
}


#[tokio::test]
async fn test_update_complex_item_data() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item with simple data
    let title = "Complex Item".to_string();
    let data = json!({
        "front": "Hello",
        "back": "World"
    });

    let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();

    // Update with complex nested JSON data
    let complex_data = json!({
        "front": {
            "text": "Hello",
            "image_url": "https://example.com/hello.jpg",
            "audio_url": "https://example.com/hello.mp3"
        },
        "back": {
            "text": "World",
            "examples": [
                "Hello, world!",
                "Hello there, friend."
            ],
            "notes": "A common greeting."
        }
    });

    let updated_item = update_item(&pool, &created_item.get_id(), None, Some(complex_data.clone())).await.unwrap();

    // Verify that the complex data was stored and retrieved correctly
    assert_eq!(updated_item.get_data().0, complex_data);
}


#[tokio::test]
async fn test_update_nonexistent_item() {
    let pool = setup_test_db();

    // Try to update a non-existent item
    let result = update_item(
        &pool,
        "nonexistent-id",
        Some("New Title".to_string()),
        Some(json!({"front": "New", "back": "Content"}))
    ).await;

    // Verify that the update failed with an error
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"));
}


#[tokio::test]
async fn test_list_items_with_filters_default_returns_all() {
    let pool = setup_test_db();
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    let item1 = create_item(&pool, &item_type.get_id(), "Item 1".to_string(), json!({"front":"F1","back":"B1"})).await.unwrap();
    let item2 = create_item(&pool, &item_type.get_id(), "Item 2".to_string(), json!({"front":"F2","back":"B2"})).await.unwrap();

    let query = crate::dto::GetQueryDto::default();
    let items = list_items_with_filters(&pool, &query).unwrap();

    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|i| i.get_id() == item1.get_id()));
    assert!(items.iter().any(|i| i.get_id() == item2.get_id()));
}

#[tokio::test]
async fn test_list_items_with_filters_item_type_only() {
    let pool = setup_test_db();
    let type1 = create_item_type(&pool, "Test Type A".to_string(), "fsrs".to_string()).await.unwrap();
    let type2 = create_item_type(&pool, "Test Type B".to_string(), "fsrs".to_string()).await.unwrap();

    let item1 = create_item(&pool, &type1.get_id(), "Item 1".to_string(), json!({"front":"F1","back":"B1"})).await.unwrap();
    let _item2 = create_item(&pool, &type2.get_id(), "Item 2".to_string(), json!({"front":"F2","back":"B2"})).await.unwrap();

    let query = crate::dto::GetQueryDtoBuilder::new()
        .item_type_id(type1.get_id())
        .build();
    let items = list_items_with_filters(&pool, &query).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].get_id(), item1.get_id());
}

#[tokio::test]
async fn test_list_items_with_filters_item_type_no_match() {
    let pool = setup_test_db();
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();
    let _item = create_item(&pool, &item_type.get_id(), "Item 1".to_string(), json!({"front":"F1","back":"B1"})).await.unwrap();

    let query = crate::dto::GetQueryDtoBuilder::new()
        .item_type_id("nonexistent-type-id".to_string())
        .build();
    let items = list_items_with_filters(&pool, &query).unwrap();

    assert!(items.is_empty());
}

#[tokio::test]
async fn test_list_items_with_filters_next_review_before() {
    use chrono::Utc;

    let pool = setup_test_db();
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    let item1 = create_item(&pool, &item_type.get_id(), "Item 1".to_string(), json!({"front":"F1","back":"B1"})).await.unwrap();
    let item2 = create_item(&pool, &item_type.get_id(), "Item 2".to_string(), json!({"front":"F2","back":"B2"})).await.unwrap();

    // Both items have cards with next_review set to their creation time (past).
    // A far-future cutoff should return both; a past cutoff should return none.
    let far_future = Utc::now() + chrono::Duration::days(365 * 100);
    let query = crate::dto::GetQueryDtoBuilder::new()
        .next_review_before(far_future)
        .build();
    let items = list_items_with_filters(&pool, &query).unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|i| i.get_id() == item1.get_id()));
    assert!(items.iter().any(|i| i.get_id() == item2.get_id()));

    // A date in the distant past should match no cards
    let distant_past = Utc::now() - chrono::Duration::days(365 * 100);
    let query = crate::dto::GetQueryDtoBuilder::new()
        .next_review_before(distant_past)
        .build();
    let items = list_items_with_filters(&pool, &query).unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn test_list_items_with_filters_suspended_only() {
    let pool = setup_test_db();
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    let item1 = create_item(&pool, &item_type.get_id(), "Item 1".to_string(), json!({"front":"F1","back":"B1"})).await.unwrap();
    let _item2 = create_item(&pool, &item_type.get_id(), "Item 2".to_string(), json!({"front":"F2","back":"B2"})).await.unwrap();

    // Suspend all cards for item1
    let cards = crate::repo::get_cards_for_item(&pool, &item1.get_id()).unwrap();
    for card in &cards {
        crate::repo::set_card_suspended(&pool, &card.get_id(), true).await.unwrap();
    }

    // Query for suspended-only items
    let query = crate::dto::GetQueryDtoBuilder::new()
        .suspended_filter(crate::dto::SuspendedFilter::Only)
        .build();
    let items = list_items_with_filters(&pool, &query).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].get_id(), item1.get_id());
}

#[tokio::test]
async fn test_list_items_with_filters_by_tag() {
    let pool = setup_test_db();
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    let item1 = create_item(&pool, &item_type.get_id(), "Item 1".to_string(), json!({"front":"F1","back":"B1"})).await.unwrap();
    let _item2 = create_item(&pool, &item_type.get_id(), "Item 2".to_string(), json!({"front":"F2","back":"B2"})).await.unwrap();

    // Create a tag and attach it only to item1
    let tag = crate::repo::create_tag(&pool, "Special".to_string(), true).await.unwrap();
    crate::repo::add_tag_to_item(&pool, &tag.get_id(), &item1.get_id()).await.unwrap();

    let query = crate::dto::GetQueryDtoBuilder::new()
        .add_tag_id(tag.get_id())
        .build();
    let items = list_items_with_filters(&pool, &query).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].get_id(), item1.get_id());
}

#[tokio::test]
async fn test_list_items_with_filters_deduplicates_across_cards() {
    let pool = setup_test_db();
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // An item may have multiple cards. Even if multiple cards match, the item should appear once.
    let item = create_item(&pool, &item_type.get_id(), "Multi-card Item".to_string(), json!({"front":"F","back":"B"})).await.unwrap();
    let cards = crate::repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
    // Verify there's at least one card (could be more depending on item type config)
    assert!(!cards.is_empty());

    let far_future = chrono::Utc::now() + chrono::Duration::days(365 * 100);
    let query = crate::dto::GetQueryDtoBuilder::new()
        .next_review_before(far_future)
        .build();
    let items = list_items_with_filters(&pool, &query).unwrap();

    // The item should appear exactly once regardless of card count
    assert_eq!(items.iter().filter(|i| i.get_id() == item.get_id()).count(), 1);
}

#[tokio::test]
async fn test_list_items_with_filters_card_filter_no_match_returns_empty() {
    let pool = setup_test_db();
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();
    let _item = create_item(&pool, &item_type.get_id(), "Item".to_string(), json!({"front":"F","back":"B"})).await.unwrap();

    // Use a tag that doesn't exist on any item
    let query = crate::dto::GetQueryDtoBuilder::new()
        .add_tag_id("nonexistent-tag-id".to_string())
        .build();
    let items = list_items_with_filters(&pool, &query).unwrap();

    assert!(items.is_empty());
}

#[tokio::test]
async fn test_list_items_with_filters_item_type_and_card_filter() {
    use chrono::Utc;

    let pool = setup_test_db();
    let type1 = create_item_type(&pool, "Test Type A".to_string(), "fsrs".to_string()).await.unwrap();
    let type2 = create_item_type(&pool, "Test Type B".to_string(), "fsrs".to_string()).await.unwrap();

    let item1 = create_item(&pool, &type1.get_id(), "Item 1".to_string(), json!({"front":"F1","back":"B1"})).await.unwrap();
    let _item2 = create_item(&pool, &type2.get_id(), "Item 2".to_string(), json!({"front":"F2","back":"B2"})).await.unwrap();

    // Both items have cards due before the far future, but filter by type1
    let far_future = Utc::now() + chrono::Duration::days(365 * 100);
    let query = crate::dto::GetQueryDtoBuilder::new()
        .item_type_id(type1.get_id())
        .next_review_before(far_future)
        .build();
    let items = list_items_with_filters(&pool, &query).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].get_id(), item1.get_id());
}

#[tokio::test]
async fn test_list_items_with_filters_empty_db() {
    let pool = setup_test_db();

    let query = crate::dto::GetQueryDto::default();
    let items = list_items_with_filters(&pool, &query).unwrap();
    assert!(items.is_empty());

    // Also with a card-level filter on an empty db
    let query = crate::dto::GetQueryDtoBuilder::new()
        .suspended_filter(crate::dto::SuspendedFilter::Only)
        .build();
    let items = list_items_with_filters(&pool, &query).unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn test_update_with_empty_changes() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();

    // Create an item
    let title = "Original Title".to_string();
    let data = json!({
        "front": "Hello",
        "back": "World"
    });

    let created_item = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();

    // Update with no changes (None for both fields)
    // Only the updated_at timestamp should change
    let updated_item = update_item(&pool, &created_item.get_id(), None, None).await.unwrap();

    // Verify that the item's content remains unchanged
    assert_eq!(updated_item.get_title(), title);
    assert_eq!(updated_item.get_data().0, data);
    assert_eq!(updated_item.get_id(), created_item.get_id());

    // The updated_at timestamp should be different, but we can't easily test for that
    // without mocking time or introducing complex test logic
}

/// Regression: create_item with an item type whose name doesn't contain "Test"
/// (or match "Basic"/"Cloze"/"Todo") fails because create_cards_for_item
/// doesn't know how to construct cards for unknown item type names.
#[tokio::test]
async fn test_create_item_unknown_item_type_name_fails() {
    let pool = setup_test_db();

    let item_type = create_item_type(&pool, "0".to_string(), "fsrs".to_string()).await.unwrap();
    let result = create_item(
        &pool,
        &item_type.get_id(),
        "Title".to_string(),
        json!({"key": "value"}),
    ).await;

    assert!(result.is_err(), "create_item should fail for unknown item type name");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Unable to construct cards for unknown item type"),
        "Error should mention unknown item type, got: {}", err);
}

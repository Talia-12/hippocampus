use super::*;
use crate::repo::tests::setup_test_db;

#[tokio::test]
async fn test_create_tag() {
    let pool = setup_test_db();

    let name = "Important".to_string();
    let visible = true;

    let tag = create_tag(&pool, name.clone(), visible).await.unwrap();

    assert_eq!(tag.get_name(), name);
    assert_eq!(tag.get_visible(), visible);
}

#[tokio::test]
async fn test_get_tag() {
    let pool = setup_test_db();

    let name = "Important".to_string();
    let visible = true;

    let created_tag = create_tag(&pool, name.clone(), visible).await.unwrap();
    let retrieved_tag = get_tag(&pool, &created_tag.get_id()).unwrap();

    assert_eq!(retrieved_tag.get_name(), name);
    assert_eq!(retrieved_tag.get_id(), created_tag.get_id());
    assert_eq!(retrieved_tag.get_visible(), visible);
}

#[tokio::test]
async fn test_list_tags() {
    let pool = setup_test_db();

    // Create some tags
    let tag1 = create_tag(&pool, "Important".to_string(), true).await.unwrap();
    let tag2 = create_tag(&pool, "Difficult".to_string(), false).await.unwrap();

    // List all tags
    let tags = list_tags(&pool).unwrap();

    // Verify that the list contains the created tags
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().any(|t| t.get_id() == tag1.get_id()));
    assert!(tags.iter().any(|t| t.get_id() == tag2.get_id()));
}

#[tokio::test]
async fn test_tag_error_handling() {
    let pool = setup_test_db();

    // Try to get a non-existent tag
    let result = get_tag(&pool, "nonexistent-id");

    // Verify that we got an error
    assert!(result.is_err());
    let error = result.unwrap_err().to_string();
    assert!(error.contains("Failed to get tag"));
}


#[tokio::test]
async fn test_list_tags_for_item() {
    let pool = setup_test_db();

    // Create necessary objects
    let item_type = crate::repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();

    // Create some items
    let item1 = crate::repo::create_item(
        &pool,
        &item_type.get_id(),
        "Item 1".to_string(),
        serde_json::json!({"front": "Hello", "back": "World"}),
    ).await.unwrap();

    let item2 = crate::repo::create_item(
        &pool,
        &item_type.get_id(),
        "Item 2".to_string(),
        serde_json::json!({"front": "Goodbye", "back": "World"}),
    ).await.unwrap();

    // Create some tags
    let tag1 = create_tag(&pool, "Important".to_string(), true).await.unwrap();
    let tag2 = create_tag(&pool, "Difficult".to_string(), false).await.unwrap();

    // Add tags to item1
    add_tag_to_item(&pool, &tag1.get_id(), &item1.get_id()).await.unwrap();
    add_tag_to_item(&pool, &tag2.get_id(), &item1.get_id()).await.unwrap();

    // Add only tag1 to item2
    add_tag_to_item(&pool, &tag1.get_id(), &item2.get_id()).await.unwrap();

    // Test list_tags_for_item with item1
    let item1_tags = list_tags_for_item(&pool, &item1.get_id()).unwrap();
    assert_eq!(item1_tags.len(), 2);
    assert!(item1_tags.iter().any(|t| t.get_id() == tag1.get_id()));
    assert!(item1_tags.iter().any(|t| t.get_id() == tag2.get_id()));

    // Test list_tags_for_item with item2
    let item2_tags = list_tags_for_item(&pool, &item2.get_id()).unwrap();
    assert_eq!(item2_tags.len(), 1);
    assert!(item2_tags.iter().any(|t| t.get_id() == tag1.get_id()));
    assert!(!item2_tags.iter().any(|t| t.get_id() == tag2.get_id()));
}


#[tokio::test]
async fn test_list_tags_for_card() {
    let pool = setup_test_db();

    // Create necessary objects
    let item_type = crate::repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();

    // Create an item
    let item = crate::repo::create_item(
        &pool,
        &item_type.get_id(),
        "Test Item".to_string(),
        serde_json::json!({"front": "Hello", "back": "World"}),
    ).await.unwrap();

    // Get the card created for the item
    let cards = crate::repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
    let card = &cards[0];

    // Create some tags
    let tag1 = create_tag(&pool, "Important".to_string(), true).await.unwrap();
    let tag2 = create_tag(&pool, "Difficult".to_string(), false).await.unwrap();

    // Add tags to the item
    add_tag_to_item(&pool, &tag1.get_id(), &item.get_id()).await.unwrap();
    add_tag_to_item(&pool, &tag2.get_id(), &item.get_id()).await.unwrap();

    // Test list_tags_for_card
    let card_tags = list_tags_for_card(&pool, &card.get_id()).unwrap();
    assert_eq!(card_tags.len(), 2);
    assert!(card_tags.iter().any(|t| t.get_id() == tag1.get_id()));
    assert!(card_tags.iter().any(|t| t.get_id() == tag2.get_id()));
}



#[tokio::test]
async fn test_add_tag_to_item() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = crate::repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();

    // Create an item
    let item = crate::repo::create_item(
        &pool,
        &item_type.get_id(),
        "Tagged Item".to_string(),
        serde_json::json!({"front": "F1", "back": "B1"})
    ).await.unwrap();

    // Create a tag
    let tag = create_tag(&pool, "Important".to_string(), true).await.unwrap();

    // Add the tag to the item
    add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();

    // Get the tags for the item
    let tags = list_tags_for_item(&pool, &item.get_id()).unwrap();

    // Verify that the item has the tag
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].get_id(), tag.get_id());
}


#[tokio::test]
async fn test_remove_tag_from_item() {
    let pool = setup_test_db();

    // Create an item type
    let item_type = crate::repo::create_item_type(&pool, "Test Type".to_string()).await.unwrap();

    // Create an item
    let item = crate::repo::create_item(
        &pool,
        &item_type.get_id(),
        "Tagged Item".to_string(),
        serde_json::json!({"front": "F1", "back": "B1"})
    ).await.unwrap();

    // Create a tag
    let tag = create_tag(&pool, "Important".to_string(), true).await.unwrap();

    // Add the tag to the item
    add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();

    // Verify that the item has the tag
    let tags_before = list_tags_for_item(&pool, &item.get_id()).unwrap();
    assert_eq!(tags_before.len(), 1);

    // Remove the tag from the item
    remove_tag_from_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();

    // Verify that the item no longer has the tag
    let tags_after = list_tags_for_item(&pool, &item.get_id()).unwrap();
    assert_eq!(tags_after.len(), 0);
}

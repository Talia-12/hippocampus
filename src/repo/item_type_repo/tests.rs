use super::*;
use crate::repo::tests::setup_test_db;

#[tokio::test]
async fn test_create_item_type() {
    let pool = setup_test_db();
    let name = "Type 1".to_string();

    let item_type = create_item_type(&pool, name.clone(), "fsrs".to_string()).await.unwrap();

    assert_eq!(item_type.get_name(), name);
    assert_eq!(item_type.get_review_function(), "fsrs");
}

#[tokio::test]
async fn test_create_item_type_incremental_queue() {
    let pool = setup_test_db();
    let name = "Todo".to_string();

    let item_type = create_item_type(&pool, name.clone(), "incremental_queue".to_string()).await.unwrap();

    assert_eq!(item_type.get_name(), name);
    assert_eq!(item_type.get_review_function(), "incremental_queue");
}

#[tokio::test]
async fn test_get_item_type() {
    let pool = setup_test_db();
    let name = "Type 1".to_string();

    let created_item_type = create_item_type(&pool, name.clone(), "fsrs".to_string()).await.unwrap();
    let retrieved_item_type = get_item_type(&pool, &created_item_type.get_id()).unwrap().unwrap();

    assert_eq!(retrieved_item_type.get_name(), name);
    assert_eq!(retrieved_item_type.get_id(), created_item_type.get_id());
    assert_eq!(retrieved_item_type.get_review_function(), "fsrs");
}

#[tokio::test]
async fn test_list_item_types() {
    let pool = setup_test_db();

    // Create some item types
    let item_type1 = create_item_type(&pool, "Type 1".to_string(), "fsrs".to_string()).await.unwrap();
    let item_type2 = create_item_type(&pool, "Type 2".to_string(), "fsrs".to_string()).await.unwrap();

    // List all item types
    let item_types = list_item_types(&pool).unwrap();

    // Verify that the list contains the created item types
    assert_eq!(item_types.len(), 2);
    assert!(item_types.iter().any(|it| it.get_id() == item_type1.get_id()));
    assert!(item_types.iter().any(|it| it.get_id() == item_type2.get_id()));
}

#[tokio::test]
async fn test_update_item_type_review_function() {
    let pool = setup_test_db();

    let item_type = create_item_type(&pool, "Todo".to_string(), "fsrs".to_string()).await.unwrap();
    assert_eq!(item_type.get_review_function(), "fsrs");

    let updated = update_item_type_review_function(&pool, &item_type.get_id(), "incremental_queue".to_string()).await.unwrap();
    assert_eq!(updated.get_review_function(), "incremental_queue");
    assert_eq!(updated.get_name(), "Todo");
}

#[tokio::test]
async fn test_update_item_type_review_function_not_found() {
    let pool = setup_test_db();

    let result = update_item_type_review_function(&pool, "nonexistent", "fsrs".to_string()).await;
    assert!(result.is_err());
}

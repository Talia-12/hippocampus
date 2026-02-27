use super::*;
use crate::repo::tests::setup_test_db;

/// Helper to create an item type and item for testing
async fn create_test_item(pool: &DbPool, title: &str) -> crate::models::Item {
	let item_type =
		crate::repo::create_item_type(pool, "Test Type".to_string(), "fsrs".to_string())
			.await
			.unwrap();

	crate::repo::create_item(
		pool,
		&item_type.get_id(),
		title.to_string(),
		serde_json::json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap()
}

/// Helper to create an item reusing an existing item type
async fn create_test_item_with_type(
	pool: &DbPool,
	item_type_id: &str,
	title: &str,
) -> crate::models::Item {
	crate::repo::create_item(
		pool,
		item_type_id,
		title.to_string(),
		serde_json::json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap()
}

#[tokio::test]
async fn test_create_item_relation() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;

	let relation = create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();

	assert_eq!(relation.get_parent_item_id(), item_a.get_id());
	assert_eq!(relation.get_child_item_id(), item_b.get_id());
	assert_eq!(relation.get_relation_type(), "extract");
}

#[tokio::test]
async fn test_get_item_relation() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;

	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();

	let result = get_item_relation(&pool, &item_a.get_id(), &item_b.get_id()).unwrap();
	assert!(result.is_some());

	let relation = result.unwrap();
	assert_eq!(relation.get_parent_item_id(), item_a.get_id());
	assert_eq!(relation.get_child_item_id(), item_b.get_id());
}

#[tokio::test]
async fn test_get_item_relation_not_found() {
	let pool = setup_test_db();

	let result = get_item_relation(&pool, "nonexistent", "also-nonexistent").unwrap();
	assert!(result.is_none());
}

#[tokio::test]
async fn test_delete_item_relation() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;

	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();

	// Verify it exists
	assert!(
		get_item_relation(&pool, &item_a.get_id(), &item_b.get_id())
			.unwrap()
			.is_some()
	);

	// Delete it
	delete_item_relation(&pool, &item_a.get_id(), &item_b.get_id())
		.await
		.unwrap();

	// Verify it's gone
	assert!(
		get_item_relation(&pool, &item_a.get_id(), &item_b.get_id())
			.unwrap()
			.is_none()
	);
}

#[tokio::test]
async fn test_delete_item_relation_not_found() {
	let pool = setup_test_db();

	let result = delete_item_relation(&pool, "nonexistent", "also-nonexistent").await;
	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
async fn test_list_item_relations() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
	let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_a.get_id(), &item_c.get_id(), "cloze")
		.await
		.unwrap();

	// List all
	let all = list_item_relations(&pool, None, None, None).unwrap();
	assert_eq!(all.len(), 2);

	// Filter by parent
	let by_parent = list_item_relations(&pool, Some(&item_a.get_id()), None, None).unwrap();
	assert_eq!(by_parent.len(), 2);

	// Filter by child
	let by_child = list_item_relations(&pool, None, Some(&item_b.get_id()), None).unwrap();
	assert_eq!(by_child.len(), 1);
	assert_eq!(by_child[0].get_child_item_id(), item_b.get_id());

	// Filter by relation type
	let by_type = list_item_relations(&pool, None, None, Some("cloze")).unwrap();
	assert_eq!(by_type.len(), 1);
	assert_eq!(by_type[0].get_child_item_id(), item_c.get_id());
}

#[tokio::test]
async fn test_self_loop_rejected() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;

	let result = create_item_relation(&pool, &item_a.get_id(), &item_a.get_id(), "extract").await;
	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("cycle"));
}

#[tokio::test]
async fn test_cycle_detection_a_b_c_then_c_a() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
	let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

	// A -> B -> C
	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_b.get_id(), &item_c.get_id(), "extract")
		.await
		.unwrap();

	// C -> A should be rejected (would create cycle)
	let result = create_item_relation(&pool, &item_c.get_id(), &item_a.get_id(), "extract").await;
	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("cycle"));
}

#[tokio::test]
async fn test_cycle_detection_allows_valid_dag() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
	let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;
	let item_d = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item D").await;

	// Diamond: A -> B, A -> C, B -> D, C -> D (valid DAG)
	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_a.get_id(), &item_c.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_b.get_id(), &item_d.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_c.get_id(), &item_d.get_id(), "extract")
		.await
		.unwrap();

	// All four relations should exist
	let all = list_item_relations(&pool, None, None, None).unwrap();
	assert_eq!(all.len(), 4);
}

#[tokio::test]
async fn test_cascade_delete_cleanup() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;

	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();

	// Verify relation exists
	assert!(
		get_item_relation(&pool, &item_a.get_id(), &item_b.get_id())
			.unwrap()
			.is_some()
	);

	// Delete the parent item — should cascade delete the relation
	crate::repo::delete_item(&pool, &item_a.get_id())
		.await
		.unwrap();

	// Verify relation is gone
	assert!(
		get_item_relation(&pool, &item_a.get_id(), &item_b.get_id())
			.unwrap()
			.is_none()
	);
}

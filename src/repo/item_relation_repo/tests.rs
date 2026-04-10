use super::*;
use crate::{models::ItemTypeId, repo::tests::setup_test_db};

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
	item_type_id: &ItemTypeId,
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

	let result = get_item_relation(
		&pool,
		&ItemId("nonexistent".to_string()),
		&ItemId("also-nonexistent".to_string()),
	)
	.unwrap();
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

	let result = delete_item_relation(
		&pool,
		&ItemId("nonexistent".to_string()),
		&ItemId("also-nonexistent".to_string()),
	)
	.await;
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

#[tokio::test]
async fn test_get_all_descendants() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
	let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;
	let item_d = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item D").await;

	// A -> B -> C -> D
	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_b.get_id(), &item_c.get_id(), "cloze")
		.await
		.unwrap();
	create_item_relation(&pool, &item_c.get_id(), &item_d.get_id(), "simplify")
		.await
		.unwrap();

	let descendants = get_all_descendants(&pool, &item_a.get_id()).unwrap();

	// Should have 3 edges: A->B, B->C, C->D
	assert_eq!(descendants.len(), 3);

	// Collect all child IDs from the edges
	let child_ids: Vec<_> = descendants.iter().map(|e| &e.child_id).collect();
	assert!(child_ids.contains(&&item_b.get_id()));
	assert!(child_ids.contains(&&item_c.get_id()));
	assert!(child_ids.contains(&&item_d.get_id()));
}

#[tokio::test]
async fn test_get_all_descendants_empty() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;

	let descendants = get_all_descendants(&pool, &item_a.get_id()).unwrap();
	assert!(descendants.is_empty());
}

#[tokio::test]
async fn test_get_all_ancestors() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
	let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;
	let item_d = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item D").await;

	// A -> B -> C -> D
	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_b.get_id(), &item_c.get_id(), "cloze")
		.await
		.unwrap();
	create_item_relation(&pool, &item_c.get_id(), &item_d.get_id(), "simplify")
		.await
		.unwrap();

	let ancestors = get_all_ancestors(&pool, &item_d.get_id()).unwrap();

	// Should have 3 edges: C->D, B->C, A->B
	assert_eq!(ancestors.len(), 3);

	// Collect all parent IDs from the edges
	let parent_ids: Vec<_> = ancestors.iter().map(|e| &e.parent_id).collect();
	assert!(parent_ids.contains(&&item_a.get_id()));
	assert!(parent_ids.contains(&&item_b.get_id()));
	assert!(parent_ids.contains(&&item_c.get_id()));
}

#[tokio::test]
async fn test_get_all_ancestors_empty() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;

	let ancestors = get_all_ancestors(&pool, &item_a.get_id()).unwrap();
	assert!(ancestors.is_empty());
}

#[tokio::test]
async fn test_get_children_of() {
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

	let children = get_children_of(&pool, &item_a.get_id()).unwrap();
	assert_eq!(children.len(), 2);
	assert!(children.contains(&item_b.get_id()));
	assert!(children.contains(&item_c.get_id()));
}

#[tokio::test]
async fn test_get_parents_of() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
	let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

	// Both A and B are parents of C
	create_item_relation(&pool, &item_a.get_id(), &item_c.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_b.get_id(), &item_c.get_id(), "cloze")
		.await
		.unwrap();

	let parents = get_parents_of(&pool, &item_c.get_id()).unwrap();
	assert_eq!(parents.len(), 2);
	assert!(parents.contains(&item_a.get_id()));
	assert!(parents.contains(&item_b.get_id()));
}

#[tokio::test]
async fn test_get_children_graph_linear() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
	let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

	// A -> B -> C
	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_b.get_id(), &item_c.get_id(), "cloze")
		.await
		.unwrap();

	let graph = get_children_graph(&pool, &item_a).unwrap();

	assert_eq!(graph.item.get_id(), item_a.get_id());
	assert!(graph.relation_type.is_none());
	assert_eq!(graph.children.len(), 1);
	assert_eq!(graph.children[0].item.get_id(), item_b.get_id());
	assert_eq!(graph.children[0].relation_type.as_deref(), Some("extract"));
	assert_eq!(graph.children[0].children.len(), 1);
	assert_eq!(graph.children[0].children[0].item.get_id(), item_c.get_id());
	assert_eq!(
		graph.children[0].children[0].relation_type.as_deref(),
		Some("cloze")
	);
}

#[tokio::test]
async fn test_get_children_graph_empty() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;

	let graph = get_children_graph(&pool, &item_a).unwrap();

	assert_eq!(graph.item.get_id(), item_a.get_id());
	assert!(graph.children.is_empty());
}

#[tokio::test]
async fn test_get_children_graph_diamond() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
	let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;
	let item_d = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item D").await;

	// Diamond: A -> B, A -> C, B -> D, C -> D
	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_a.get_id(), &item_c.get_id(), "simplify")
		.await
		.unwrap();
	create_item_relation(&pool, &item_b.get_id(), &item_d.get_id(), "cloze")
		.await
		.unwrap();
	create_item_relation(&pool, &item_c.get_id(), &item_d.get_id(), "extract")
		.await
		.unwrap();

	let graph = get_children_graph(&pool, &item_a).unwrap();

	assert_eq!(graph.children.len(), 2);
	// D should appear in both subtrees
	let mut d_count = 0;
	for child in &graph.children {
		for grandchild in &child.children {
			if grandchild.item.get_id() == item_d.get_id() {
				d_count += 1;
			}
		}
	}
	assert_eq!(d_count, 2, "D should appear under both B and C");
}

#[tokio::test]
async fn test_get_parent_graph_linear() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
	let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

	// A -> B -> C
	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_b.get_id(), &item_c.get_id(), "cloze")
		.await
		.unwrap();

	let graph = get_parent_graph(&pool, &item_c).unwrap();

	assert_eq!(graph.item.get_id(), item_c.get_id());
	assert!(graph.relation_type.is_none());
	assert_eq!(graph.parents.len(), 1);
	assert_eq!(graph.parents[0].item.get_id(), item_b.get_id());
	assert_eq!(graph.parents[0].relation_type.as_deref(), Some("cloze"));
	assert_eq!(graph.parents[0].parents.len(), 1);
	assert_eq!(graph.parents[0].parents[0].item.get_id(), item_a.get_id());
	assert_eq!(
		graph.parents[0].parents[0].relation_type.as_deref(),
		Some("extract")
	);
}

#[tokio::test]
async fn test_get_parent_graph_empty() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;

	let graph = get_parent_graph(&pool, &item_a).unwrap();

	assert_eq!(graph.item.get_id(), item_a.get_id());
	assert!(graph.parents.is_empty());
}

#[tokio::test]
async fn test_get_parent_graph_diamond() {
	let pool = setup_test_db();
	let item_a = create_test_item(&pool, "Item A").await;
	let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
	let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;
	let item_d = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item D").await;

	// Diamond: A -> B, A -> C, B -> D, C -> D
	create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
		.await
		.unwrap();
	create_item_relation(&pool, &item_a.get_id(), &item_c.get_id(), "simplify")
		.await
		.unwrap();
	create_item_relation(&pool, &item_b.get_id(), &item_d.get_id(), "cloze")
		.await
		.unwrap();
	create_item_relation(&pool, &item_c.get_id(), &item_d.get_id(), "extract")
		.await
		.unwrap();

	let graph = get_parent_graph(&pool, &item_d).unwrap();

	assert_eq!(graph.parents.len(), 2);
	// A should appear as grandparent via both paths
	let mut a_count = 0;
	for parent in &graph.parents {
		for grandparent in &parent.parents {
			if grandparent.item.get_id() == item_a.get_id() {
				a_count += 1;
			}
		}
	}
	assert_eq!(
		a_count, 2,
		"A should appear as grandparent via both B and C"
	);
}

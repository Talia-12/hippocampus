mod common;

use axum::{
	body::{Body, to_bytes},
	http::{Request, StatusCode},
};
use common::{
	SERVER_ADDR, STARTUP_TIMEOUT, ServerGuard, create_item, create_item_type, create_test_app,
	get_cards_for_item, http_get, http_post, wait_for_server,
};
use hippocampus::{
	dto::{ItemChildGraphNode, ItemParentGraphNode},
	models::{Card, CardId, Item, ItemId, ItemRelation},
};
use serde_json::json;
use std::process::Command;
use tower::Service;

// ============================================================================
// Helpers
// ============================================================================

/// Creates an item relation via the API
async fn create_relation(
	app: &mut axum::Router,
	parent_id: &ItemId,
	child_id: &ItemId,
	relation_type: &str,
) -> (StatusCode, Vec<u8>) {
	let request = Request::builder()
		.uri(format!("/item_relations/{}/{}", parent_id, child_id))
		.method("POST")
		.header("Content-Type", "application/json")
		.body(Body::from(
			serde_json::to_string(&json!({
				"relation_type": relation_type
			}))
			.unwrap(),
		))
		.unwrap();

	let response = app.call(request).await.unwrap();
	let status = response.status();
	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	(status, body.to_vec())
}

/// Lists item relations via the API with optional filters
async fn list_relations(app: &mut axum::Router, query: &str) -> (StatusCode, Vec<u8>) {
	let uri = if query.is_empty() {
		"/item_relations".to_string()
	} else {
		format!("/item_relations?{}", query)
	};

	let request = Request::builder()
		.uri(&uri)
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	let status = response.status();
	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	(status, body.to_vec())
}

/// Deletes an item relation via the API
async fn delete_relation(
	app: &mut axum::Router,
	parent_id: &ItemId,
	child_id: &ItemId,
) -> StatusCode {
	let request = Request::builder()
		.uri(format!("/item_relations/{}/{}", parent_id, child_id))
		.method("DELETE")
		.body(Body::empty())
		.unwrap();

	app.call(request).await.unwrap().status()
}

/// Gets the children graph for an item via the API
async fn get_children_graph(app: &mut axum::Router, item_id: &ItemId) -> (StatusCode, Vec<u8>) {
	let request = Request::builder()
		.uri(format!("/items/{}/children_graph", item_id))
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	let status = response.status();
	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	(status, body.to_vec())
}

/// Gets the parent graph for an item via the API
async fn get_parent_graph(app: &mut axum::Router, item_id: &ItemId) -> (StatusCode, Vec<u8>) {
	let request = Request::builder()
		.uri(format!("/items/{}/parent_graph", item_id))
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	let status = response.status();
	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	(status, body.to_vec())
}

/// Lists items via the API with query parameters
async fn list_items(app: &mut axum::Router, query: &str) -> Vec<Item> {
	let uri = if query.is_empty() {
		"/items".to_string()
	} else {
		format!("/items?{}", query)
	};

	let request = Request::builder()
		.uri(&uri)
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	assert_eq!(response.status(), StatusCode::OK);
	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	serde_json::from_slice(&body).unwrap()
}

/// Lists cards via the API with query parameters
async fn list_cards(app: &mut axum::Router, query: &str) -> Vec<Card> {
	let uri = if query.is_empty() {
		"/cards".to_string()
	} else {
		format!("/cards?{}", query)
	};

	let request = Request::builder()
		.uri(&uri)
		.method("GET")
		.body(Body::empty())
		.unwrap();

	let response = app.call(request).await.unwrap();
	let status = response.status();
	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	assert_eq!(
		status,
		StatusCode::OK,
		"Expected 200 OK but got {} with body: {}",
		status,
		String::from_utf8_lossy(&body)
	);
	serde_json::from_slice(&body).unwrap()
}

/// Sets up a standard test scenario: item type + 3 items (A, B, C)
async fn setup_three_items(app: &mut axum::Router) -> (Item, Item, Item) {
	let item_type = create_item_type(app, "Basic".to_string()).await;
	let item_a = create_item(
		app,
		&item_type.get_id(),
		"Item A".to_string(),
		Some(json!({"front": "A front", "back": "A back"})),
	)
	.await;
	let item_b = create_item(
		app,
		&item_type.get_id(),
		"Item B".to_string(),
		Some(json!({"front": "B front", "back": "B back"})),
	)
	.await;
	let item_c = create_item(
		app,
		&item_type.get_id(),
		"Item C".to_string(),
		Some(json!({"front": "C front", "back": "C back"})),
	)
	.await;
	(item_a, item_b, item_c)
}

// ============================================================================
// CRUD Tests
// ============================================================================

#[tokio::test]
async fn test_create_item_relation() {
	let mut app = create_test_app();
	let (item_a, item_b, _) = setup_three_items(&mut app).await;

	let (status, body) =
		create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;

	assert_eq!(status, StatusCode::OK);
	let relation: ItemRelation = serde_json::from_slice(&body).unwrap();
	assert_eq!(relation.get_parent_item_id(), item_a.get_id());
	assert_eq!(relation.get_child_item_id(), item_b.get_id());
	assert_eq!(relation.get_relation_type(), "extract");
}

#[tokio::test]
async fn test_create_item_relation_nonexistent_parent() {
	let mut app = create_test_app();
	let (_, item_b, _) = setup_three_items(&mut app).await;

	let nonexistent = ItemId("nonexistent".to_string());
	let (status, _) = create_relation(&mut app, &nonexistent, &item_b.get_id(), "extract").await;

	assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_item_relation_nonexistent_child() {
	let mut app = create_test_app();
	let (item_a, _, _) = setup_three_items(&mut app).await;

	let nonexistent = ItemId("nonexistent".to_string());
	let (status, _) = create_relation(&mut app, &item_a.get_id(), &nonexistent, "extract").await;

	assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_item_relation_cycle_detected() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	// A -> B -> C
	let (status, _) =
		create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;
	assert_eq!(status, StatusCode::OK);
	let (status, _) =
		create_relation(&mut app, &item_b.get_id(), &item_c.get_id(), "extract").await;
	assert_eq!(status, StatusCode::OK);

	// C -> A would create a cycle
	let (status, _) =
		create_relation(&mut app, &item_c.get_id(), &item_a.get_id(), "extract").await;
	assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_create_item_relation_self_loop() {
	let mut app = create_test_app();
	let (item_a, _, _) = setup_three_items(&mut app).await;

	let (status, _) =
		create_relation(&mut app, &item_a.get_id(), &item_a.get_id(), "extract").await;
	assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_delete_item_relation() {
	let mut app = create_test_app();
	let (item_a, item_b, _) = setup_three_items(&mut app).await;

	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;

	let status = delete_relation(&mut app, &item_a.get_id(), &item_b.get_id()).await;
	assert_eq!(status, StatusCode::OK);

	// Verify it's gone
	let (_, body) = list_relations(&mut app, "").await;
	let relations: Vec<ItemRelation> = serde_json::from_slice(&body).unwrap();
	assert!(relations.is_empty());
}

#[tokio::test]
async fn test_delete_item_relation_not_found() {
	let mut app = create_test_app();

	let nonexistent = ItemId("nonexistent".to_string());
	let also_nonexistent = ItemId("also-nonexistent".to_string());
	let status = delete_relation(&mut app, &nonexistent, &also_nonexistent).await;
	assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// List / Filter Tests
// ============================================================================

#[tokio::test]
async fn test_list_item_relations_no_filter() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;
	create_relation(&mut app, &item_a.get_id(), &item_c.get_id(), "cloze").await;

	let (status, body) = list_relations(&mut app, "").await;
	assert_eq!(status, StatusCode::OK);
	let relations: Vec<ItemRelation> = serde_json::from_slice(&body).unwrap();
	assert_eq!(relations.len(), 2);
}

#[tokio::test]
async fn test_list_item_relations_parent_filter() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;
	create_relation(&mut app, &item_a.get_id(), &item_c.get_id(), "cloze").await;
	create_relation(&mut app, &item_b.get_id(), &item_c.get_id(), "simplify").await;

	let query = format!("parent_item_id={}", item_a.get_id());
	let (status, body) = list_relations(&mut app, &query).await;
	assert_eq!(status, StatusCode::OK);
	let relations: Vec<ItemRelation> = serde_json::from_slice(&body).unwrap();
	assert_eq!(relations.len(), 2);
	for rel in &relations {
		assert_eq!(rel.get_parent_item_id(), item_a.get_id());
	}
}

#[tokio::test]
async fn test_list_item_relations_child_filter() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	create_relation(&mut app, &item_a.get_id(), &item_c.get_id(), "extract").await;
	create_relation(&mut app, &item_b.get_id(), &item_c.get_id(), "cloze").await;

	let query = format!("child_item_id={}", item_c.get_id());
	let (status, body) = list_relations(&mut app, &query).await;
	assert_eq!(status, StatusCode::OK);
	let relations: Vec<ItemRelation> = serde_json::from_slice(&body).unwrap();
	assert_eq!(relations.len(), 2);
	for rel in &relations {
		assert_eq!(rel.get_child_item_id(), item_c.get_id());
	}
}

#[tokio::test]
async fn test_list_item_relations_type_filter() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;
	create_relation(&mut app, &item_a.get_id(), &item_c.get_id(), "cloze").await;

	let (status, body) = list_relations(&mut app, "relation_type=cloze").await;
	assert_eq!(status, StatusCode::OK);
	let relations: Vec<ItemRelation> = serde_json::from_slice(&body).unwrap();
	assert_eq!(relations.len(), 1);
	assert_eq!(relations[0].get_relation_type(), "cloze");
}

#[tokio::test]
async fn test_list_item_relations_nonexistent_parent_filter() {
	let mut app = create_test_app();
	let (item_a, item_b, _) = setup_three_items(&mut app).await;

	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;

	let (status, body) = list_relations(&mut app, "parent_item_id=nonexistent").await;
	assert_eq!(status, StatusCode::OK);
	let relations: Vec<ItemRelation> = serde_json::from_slice(&body).unwrap();
	assert!(relations.is_empty());
}

// ============================================================================
// Children Graph Tests
// ============================================================================

#[tokio::test]
async fn test_children_graph_linear_chain() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	// A -> B -> C
	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;
	create_relation(&mut app, &item_b.get_id(), &item_c.get_id(), "cloze").await;

	let (status, body) = get_children_graph(&mut app, &item_a.get_id()).await;
	assert_eq!(status, StatusCode::OK);

	let graph: ItemChildGraphNode = serde_json::from_slice(&body).unwrap();
	assert_eq!(graph.item.get_id(), item_a.get_id());
	assert!(graph.relation_type.is_none());
	assert_eq!(graph.children.len(), 1);

	let child_b = &graph.children[0];
	assert_eq!(child_b.item.get_id(), item_b.get_id());
	assert_eq!(child_b.relation_type.as_deref(), Some("extract"));
	assert_eq!(child_b.children.len(), 1);

	let child_c = &child_b.children[0];
	assert_eq!(child_c.item.get_id(), item_c.get_id());
	assert_eq!(child_c.relation_type.as_deref(), Some("cloze"));
	assert!(child_c.children.is_empty());
}

#[tokio::test]
async fn test_children_graph_diamond_dag() {
	let mut app = create_test_app();
	let item_type = create_item_type(&mut app, "Basic".to_string()).await;
	let item_a = create_item(
		&mut app,
		&item_type.get_id(),
		"A".to_string(),
		Some(json!({"front":"a","back":"a"})),
	)
	.await;
	let item_b = create_item(
		&mut app,
		&item_type.get_id(),
		"B".to_string(),
		Some(json!({"front":"b","back":"b"})),
	)
	.await;
	let item_c = create_item(
		&mut app,
		&item_type.get_id(),
		"C".to_string(),
		Some(json!({"front":"c","back":"c"})),
	)
	.await;
	let item_d = create_item(
		&mut app,
		&item_type.get_id(),
		"D".to_string(),
		Some(json!({"front":"d","back":"d"})),
	)
	.await;

	// Diamond: A -> B, A -> C, B -> D, C -> D
	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;
	create_relation(&mut app, &item_a.get_id(), &item_c.get_id(), "simplify").await;
	create_relation(&mut app, &item_b.get_id(), &item_d.get_id(), "cloze").await;
	create_relation(&mut app, &item_c.get_id(), &item_d.get_id(), "extract").await;

	let (status, body) = get_children_graph(&mut app, &item_a.get_id()).await;
	assert_eq!(status, StatusCode::OK);

	let graph: ItemChildGraphNode = serde_json::from_slice(&body).unwrap();
	assert_eq!(graph.children.len(), 2);

	// D should appear as grandchild via both paths
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
async fn test_children_graph_leaf_node() {
	let mut app = create_test_app();
	let (item_a, item_b, _) = setup_three_items(&mut app).await;

	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;

	// B is a leaf — should have empty children
	let (status, body) = get_children_graph(&mut app, &item_b.get_id()).await;
	assert_eq!(status, StatusCode::OK);
	let graph: ItemChildGraphNode = serde_json::from_slice(&body).unwrap();
	assert_eq!(graph.item.get_id(), item_b.get_id());
	assert!(graph.children.is_empty());
}

#[tokio::test]
async fn test_children_graph_nonexistent_item() {
	let mut app = create_test_app();

	let nonexistent = ItemId("nonexistent".to_string());
	let (status, _) = get_children_graph(&mut app, &nonexistent).await;
	assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Parent Graph Tests
// ============================================================================

#[tokio::test]
async fn test_parent_graph_linear_chain() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	// A -> B -> C
	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;
	create_relation(&mut app, &item_b.get_id(), &item_c.get_id(), "cloze").await;

	let (status, body) = get_parent_graph(&mut app, &item_c.get_id()).await;
	assert_eq!(status, StatusCode::OK);

	let graph: ItemParentGraphNode = serde_json::from_slice(&body).unwrap();
	assert_eq!(graph.item.get_id(), item_c.get_id());
	assert!(graph.relation_type.is_none());
	assert_eq!(graph.parents.len(), 1);

	let parent_b = &graph.parents[0];
	assert_eq!(parent_b.item.get_id(), item_b.get_id());
	assert_eq!(parent_b.relation_type.as_deref(), Some("cloze"));
	assert_eq!(parent_b.parents.len(), 1);

	let parent_a = &parent_b.parents[0];
	assert_eq!(parent_a.item.get_id(), item_a.get_id());
	assert_eq!(parent_a.relation_type.as_deref(), Some("extract"));
	assert!(parent_a.parents.is_empty());
}

#[tokio::test]
async fn test_parent_graph_diamond_dag() {
	let mut app = create_test_app();
	let item_type = create_item_type(&mut app, "Basic".to_string()).await;
	let item_a = create_item(
		&mut app,
		&item_type.get_id(),
		"A".to_string(),
		Some(json!({"front":"a","back":"a"})),
	)
	.await;
	let item_b = create_item(
		&mut app,
		&item_type.get_id(),
		"B".to_string(),
		Some(json!({"front":"b","back":"b"})),
	)
	.await;
	let item_c = create_item(
		&mut app,
		&item_type.get_id(),
		"C".to_string(),
		Some(json!({"front":"c","back":"c"})),
	)
	.await;
	let item_d = create_item(
		&mut app,
		&item_type.get_id(),
		"D".to_string(),
		Some(json!({"front":"d","back":"d"})),
	)
	.await;

	// Diamond: A -> B, A -> C, B -> D, C -> D
	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;
	create_relation(&mut app, &item_a.get_id(), &item_c.get_id(), "simplify").await;
	create_relation(&mut app, &item_b.get_id(), &item_d.get_id(), "cloze").await;
	create_relation(&mut app, &item_c.get_id(), &item_d.get_id(), "extract").await;

	let (status, body) = get_parent_graph(&mut app, &item_d.get_id()).await;
	assert_eq!(status, StatusCode::OK);

	let graph: ItemParentGraphNode = serde_json::from_slice(&body).unwrap();
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

#[tokio::test]
async fn test_parent_graph_root_node() {
	let mut app = create_test_app();
	let (item_a, item_b, _) = setup_three_items(&mut app).await;

	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;

	// A is a root — should have empty parents
	let (status, body) = get_parent_graph(&mut app, &item_a.get_id()).await;
	assert_eq!(status, StatusCode::OK);
	let graph: ItemParentGraphNode = serde_json::from_slice(&body).unwrap();
	assert_eq!(graph.item.get_id(), item_a.get_id());
	assert!(graph.parents.is_empty());
}

#[tokio::test]
async fn test_parent_graph_nonexistent_item() {
	let mut app = create_test_app();

	let nonexistent = ItemId("nonexistent".to_string());
	let (status, _) = get_parent_graph(&mut app, &nonexistent).await;
	assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Cascade Delete Tests
// ============================================================================

#[tokio::test]
async fn test_cascade_delete_parent_item() {
	let mut app = create_test_app();
	let (item_a, item_b, _) = setup_three_items(&mut app).await;

	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;

	// Delete the parent item
	let request = Request::builder()
		.uri(format!("/items/{}", item_a.get_id()))
		.method("DELETE")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	assert!(response.status().is_success());

	// Relation should be gone
	let (_, body) = list_relations(&mut app, "").await;
	let relations: Vec<ItemRelation> = serde_json::from_slice(&body).unwrap();
	assert!(relations.is_empty());
}

#[tokio::test]
async fn test_cascade_delete_child_item() {
	let mut app = create_test_app();
	let (item_a, item_b, _) = setup_three_items(&mut app).await;

	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;

	// Delete the child item
	let request = Request::builder()
		.uri(format!("/items/{}", item_b.get_id()))
		.method("DELETE")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	assert!(response.status().is_success());

	// Relation should be gone
	let (_, body) = list_relations(&mut app, "").await;
	let relations: Vec<ItemRelation> = serde_json::from_slice(&body).unwrap();
	assert!(relations.is_empty());
}

// ============================================================================
// Item Listing with Relation Filters
// ============================================================================

#[tokio::test]
async fn test_list_items_with_parent_item_id_filter() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	// A -> B, A -> C
	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;
	create_relation(&mut app, &item_a.get_id(), &item_c.get_id(), "cloze").await;

	let items = list_items(&mut app, &format!("parent_item_id={}", item_a.get_id())).await;

	assert_eq!(items.len(), 2);
	let item_ids: Vec<_> = items.iter().map(|i| i.get_id()).collect();
	assert!(item_ids.contains(&item_b.get_id()));
	assert!(item_ids.contains(&item_c.get_id()));
	// A itself should NOT be in results (it's the parent, not a child)
	assert!(!item_ids.contains(&item_a.get_id()));
}

#[tokio::test]
async fn test_list_items_with_child_item_id_filter() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	// A -> C, B -> C
	create_relation(&mut app, &item_a.get_id(), &item_c.get_id(), "extract").await;
	create_relation(&mut app, &item_b.get_id(), &item_c.get_id(), "cloze").await;

	let items = list_items(&mut app, &format!("child_item_id={}", item_c.get_id())).await;

	assert_eq!(items.len(), 2);
	let item_ids: Vec<_> = items.iter().map(|i| i.get_id()).collect();
	assert!(item_ids.contains(&item_a.get_id()));
	assert!(item_ids.contains(&item_b.get_id()));
	// C itself should NOT be in results (it's the child, not a parent)
	assert!(!item_ids.contains(&item_c.get_id()));
}

#[tokio::test]
async fn test_list_items_with_parent_filter_no_children() {
	let mut app = create_test_app();
	let (_, item_b, _) = setup_three_items(&mut app).await;

	// B has no children
	let items = list_items(&mut app, &format!("parent_item_id={}", item_b.get_id())).await;
	assert!(items.is_empty());
}

#[tokio::test]
async fn test_list_items_with_nonexistent_parent_filter() {
	let mut app = create_test_app();
	setup_three_items(&mut app).await;

	let items = list_items(&mut app, "parent_item_id=nonexistent").await;
	assert!(items.is_empty());
}

// ============================================================================
// Card Listing with Relation Filters
// ============================================================================

#[tokio::test]
async fn test_list_cards_with_parent_item_id_filter() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	// A -> B, A -> C
	create_relation(&mut app, &item_a.get_id(), &item_b.get_id(), "extract").await;
	create_relation(&mut app, &item_a.get_id(), &item_c.get_id(), "cloze").await;

	// Get the expected cards for B and C
	let cards_b = get_cards_for_item(&mut app, &item_b.get_id()).await;
	let cards_c = get_cards_for_item(&mut app, &item_c.get_id()).await;
	let expected_card_ids: std::collections::HashSet<_> = cards_b
		.iter()
		.chain(cards_c.iter())
		.map(|c| c.get_id())
		.collect();

	let cards = list_cards(&mut app, &format!("parent_item_id={}", item_a.get_id())).await;

	let result_card_ids: std::collections::HashSet<_> = cards.iter().map(|c| c.get_id()).collect();
	assert_eq!(result_card_ids, expected_card_ids);

	// Cards for A should NOT be in results
	let cards_a = get_cards_for_item(&mut app, &item_a.get_id()).await;
	for card in &cards_a {
		assert!(
			!result_card_ids.contains(&card.get_id()),
			"Parent's own cards should not appear in results"
		);
	}
}

#[tokio::test]
async fn test_list_cards_with_child_item_id_filter() {
	let mut app = create_test_app();
	let (item_a, item_b, item_c) = setup_three_items(&mut app).await;

	// A -> C, B -> C
	create_relation(&mut app, &item_a.get_id(), &item_c.get_id(), "extract").await;
	create_relation(&mut app, &item_b.get_id(), &item_c.get_id(), "cloze").await;

	// Get the expected cards for A and B (the parents)
	let cards_a = get_cards_for_item(&mut app, &item_a.get_id()).await;
	let cards_b = get_cards_for_item(&mut app, &item_b.get_id()).await;
	let expected_card_ids: std::collections::HashSet<_> = cards_a
		.iter()
		.chain(cards_b.iter())
		.map(|c| c.get_id())
		.collect();

	let cards = list_cards(&mut app, &format!("child_item_id={}", item_c.get_id())).await;

	let result_card_ids: std::collections::HashSet<_> = cards.iter().map(|c| c.get_id()).collect();
	assert_eq!(result_card_ids, expected_card_ids);

	// Cards for C should NOT be in results
	let cards_c = get_cards_for_item(&mut app, &item_c.get_id()).await;
	for card in &cards_c {
		assert!(
			!result_card_ids.contains(&card.get_id()),
			"Child's own cards should not appear in results"
		);
	}
}

#[tokio::test]
async fn test_list_cards_with_parent_filter_no_children() {
	let mut app = create_test_app();
	let (_, item_b, _) = setup_three_items(&mut app).await;

	// B has no children
	let cards = list_cards(&mut app, &format!("parent_item_id={}", item_b.get_id())).await;
	assert!(cards.is_empty());
}

#[tokio::test]
async fn test_list_cards_with_nonexistent_parent_filter() {
	let mut app = create_test_app();
	setup_three_items(&mut app).await;

	let cards = list_cards(&mut app, "parent_item_id=nonexistent").await;
	assert!(cards.is_empty());
}

// ============================================================================
// File-based Database Tests
// ============================================================================

/// Tests that relation-filtered card listing works with a file-based SQLite database.
///
/// This exercises the code path where `list_cards_with_filters` acquires one
/// connection and then `get_children_of` acquires a second concurrent connection
/// from the pool. With file-based SQLite (as opposed to in-memory), this verifies
/// that concurrent readers don't conflict.
#[test]
fn test_relation_filter_with_file_database() {
	let tmp_dir = tempfile::tempdir().expect("Failed to create temp directory");
	let db_path = tmp_dir.path().join("test_relation_filter.db");

	let bin_path = assert_cmd::cargo::cargo_bin!("hippocampus");
	let child = Command::new(bin_path)
		.arg("--database-url")
		.arg(&db_path)
		.spawn()
		.expect("Failed to spawn server process");

	let _guard = ServerGuard(child);

	assert!(
		wait_for_server(SERVER_ADDR, STARTUP_TIMEOUT),
		"Server did not start within {:?}",
		STARTUP_TIMEOUT
	);

	// Create an item type
	let (status, body) = http_post(SERVER_ADDR, "/item_types", r#"{"name": "Basic"}"#);
	assert_eq!(status, 200, "Failed to create item type: {}", body);
	let item_type: serde_json::Value = serde_json::from_str(&body).unwrap();
	let item_type_id = item_type["id"].as_str().unwrap();

	// Create two items
	let (status, body) = http_post(
		SERVER_ADDR,
		"/items",
		&format!(
			r#"{{"item_type_id": "{}", "title": "Parent Item", "item_data": {{"front": "p", "back": "p"}}}}"#,
			item_type_id
		),
	);
	assert_eq!(status, 200, "Failed to create parent item: {}", body);
	let parent_item: serde_json::Value = serde_json::from_str(&body).unwrap();
	let parent_id = parent_item["id"].as_str().unwrap();

	let (status, body) = http_post(
		SERVER_ADDR,
		"/items",
		&format!(
			r#"{{"item_type_id": "{}", "title": "Child Item", "item_data": {{"front": "c", "back": "c"}}}}"#,
			item_type_id
		),
	);
	assert_eq!(status, 200, "Failed to create child item: {}", body);
	let child_item: serde_json::Value = serde_json::from_str(&body).unwrap();
	let child_id = child_item["id"].as_str().unwrap();

	// Create a relation: parent -> child
	let (status, body) = http_post(
		SERVER_ADDR,
		&format!("/item_relations/{}/{}", parent_id, child_id),
		r#"{"relation_type": "extract"}"#,
	);
	assert_eq!(status, 200, "Failed to create relation: {}", body);

	// List cards with parent_item_id filter — this is the double-connection path
	let (status, body) = http_get(SERVER_ADDR, &format!("/cards?parent_item_id={}", parent_id));
	assert_eq!(
		status, 200,
		"Relation-filtered card listing returned {} (body: {})",
		status, body
	);

	let cards: serde_json::Value = serde_json::from_str(&body).unwrap();
	let cards = cards.as_array().expect("Expected JSON array of cards");
	for card in cards {
		assert_eq!(
			card["item_id"].as_str().unwrap(),
			child_id,
			"Card should belong to child item"
		);
	}
	assert!(
		!cards.is_empty(),
		"Should have at least one card for the child item"
	);

	// Also verify items listing with the same filter
	let (status, body) = http_get(SERVER_ADDR, &format!("/items?parent_item_id={}", parent_id));
	assert_eq!(
		status, 200,
		"Relation-filtered item listing returned {} (body: {})",
		status, body
	);

	let items: serde_json::Value = serde_json::from_str(&body).unwrap();
	let items = items.as_array().expect("Expected JSON array of items");
	assert_eq!(items.len(), 1);
	assert_eq!(items[0]["id"].as_str().unwrap(), child_id);
}

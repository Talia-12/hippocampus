use axum::{
	Json,
	extract::{Path, Query, State},
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument};

use crate::db::DbPool;
use crate::dto::{
	CreateItemRelationDto, ItemChildGraphNode, ItemParentGraphNode, ListItemRelationsQueryDto,
};
use crate::errors::ApiError;
use crate::models::{Item, ItemRelation};
use crate::repo;

/// Handler for creating a new item relation
///
/// This function handles POST requests to `/item_relations/{parent_id}/{child_id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `parent_id` - The parent item ID from the URL path
/// * `child_id` - The child item ID from the URL path
/// * `payload` - The request payload containing the relation type
///
/// ### Returns
///
/// The newly created item relation as JSON, or 404 if items don't exist,
/// or 409 if the relation would create a cycle.
#[instrument(skip(pool), fields(parent_id = %parent_id, child_id = %child_id))]
pub async fn create_item_relation_handler(
	State(pool): State<Arc<DbPool>>,
	Path((parent_id, child_id)): Path<(String, String)>,
	Json(payload): Json<CreateItemRelationDto>,
) -> Result<Json<ItemRelation>, ApiError> {
	info!("Creating item relation");

	// Validate both items exist
	repo::get_item(&pool, &parent_id)
		.map_err(ApiError::Database)?
		.ok_or(ApiError::NotFound)?;
	repo::get_item(&pool, &child_id)
		.map_err(ApiError::Database)?
		.ok_or(ApiError::NotFound)?;

	// Create the relation (cycle detection happens inside)
	match repo::create_item_relation(&pool, &parent_id, &child_id, &payload.relation_type).await {
		Ok(relation) => {
			info!("Created item relation: {} -> {}", parent_id, child_id);
			Ok(Json(relation))
		}
		Err(e) => {
			if e.to_string().contains("cycle") {
				debug!("Cycle detected");
				Err(ApiError::CycleDetected)
			} else {
				Err(ApiError::Database(e))
			}
		}
	}
}

/// Handler for deleting an item relation
///
/// This function handles DELETE requests to `/item_relations/{parent_id}/{child_id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `parent_id` - The parent item ID from the URL path
/// * `child_id` - The child item ID from the URL path
///
/// ### Returns
///
/// A 204 No Content response if successful, or 404 if the relation doesn't exist.
#[instrument(skip(pool), fields(parent_id = %parent_id, child_id = %child_id))]
pub async fn delete_item_relation_handler(
	State(pool): State<Arc<DbPool>>,
	Path((parent_id, child_id)): Path<(String, String)>,
) -> Result<(), ApiError> {
	info!("Deleting item relation");

	match repo::delete_item_relation(&pool, &parent_id, &child_id).await {
		Ok(_) => {
			info!("Deleted item relation: {} -> {}", parent_id, child_id);
			Ok(())
		}
		Err(e) => {
			if e.to_string().contains("not found") {
				Err(ApiError::NotFound)
			} else {
				Err(ApiError::Database(e))
			}
		}
	}
}

/// Handler for listing item relations with optional filters
///
/// This function handles GET requests to `/item_relations`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `query` - Optional query parameters for filtering
///
/// ### Returns
///
/// A list of item relations as JSON
#[instrument(skip(pool))]
pub async fn list_item_relations_handler(
	State(pool): State<Arc<DbPool>>,
	Query(query): Query<ListItemRelationsQueryDto>,
) -> Result<Json<Vec<ItemRelation>>, ApiError> {
	debug!("Listing item relations");

	let relations = repo::list_item_relations(
		&pool,
		query.parent_item_id.as_deref(),
		query.child_item_id.as_deref(),
		query.relation_type.as_deref(),
	)
	.map_err(ApiError::Database)?;

	info!("Retrieved {} item relations", relations.len());
	Ok(Json(relations))
}

/// Handler for getting the children graph of an item
///
/// This function handles GET requests to `/items/{item_id}/children_graph`.
/// Returns a nested tree of all descendants.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The root item ID from the URL path
///
/// ### Returns
///
/// A nested ItemChildGraphNode tree as JSON
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn get_children_graph_handler(
	State(pool): State<Arc<DbPool>>,
	Path(item_id): Path<String>,
) -> Result<Json<ItemChildGraphNode>, ApiError> {
	debug!("Getting children graph");

	// Verify root item exists
	let root_item = repo::get_item(&pool, &item_id)
		.map_err(ApiError::Database)?
		.ok_or(ApiError::NotFound)?;

	// Get all descendant edges
	let edges = repo::get_all_descendants(&pool, &item_id).map_err(ApiError::Database)?;

	// Collect all item IDs (root + all children from edges)
	let mut item_ids: Vec<String> = edges.iter().map(|(_, c, _)| c.clone()).collect();
	item_ids.push(item_id.clone());
	item_ids.sort();
	item_ids.dedup();

	// Batch load all items
	let items_map = load_items_map(&pool, &item_ids)?;

	// Build adjacency list: parent_id -> [(child_id, relation_type)]
	let mut children_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
	for (parent_id, child_id, relation_type) in &edges {
		children_map
			.entry(parent_id.clone())
			.or_default()
			.push((child_id.clone(), relation_type.clone()));
	}

	// Recursively build the tree
	let graph = build_children_graph(&root_item, None, &items_map, &children_map);

	info!("Built children graph for item {}", item_id);
	Ok(Json(graph))
}

/// Handler for getting the parent graph of an item
///
/// This function handles GET requests to `/items/{item_id}/parent_graph`.
/// Returns a nested tree of all ancestors.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The leaf item ID from the URL path
///
/// ### Returns
///
/// A nested ItemParentGraphNode tree as JSON
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn get_parent_graph_handler(
	State(pool): State<Arc<DbPool>>,
	Path(item_id): Path<String>,
) -> Result<Json<ItemParentGraphNode>, ApiError> {
	debug!("Getting parent graph");

	// Verify root item exists
	let root_item = repo::get_item(&pool, &item_id)
		.map_err(ApiError::Database)?
		.ok_or(ApiError::NotFound)?;

	// Get all ancestor edges
	let edges = repo::get_all_ancestors(&pool, &item_id).map_err(ApiError::Database)?;

	// Collect all item IDs (root + all parents from edges)
	let mut item_ids: Vec<String> = edges.iter().map(|(p, _, _)| p.clone()).collect();
	item_ids.push(item_id.clone());
	item_ids.sort();
	item_ids.dedup();

	// Batch load all items
	let items_map = load_items_map(&pool, &item_ids)?;

	// Build adjacency list: child_id -> [(parent_id, relation_type)]
	let mut parents_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
	for (parent_id, child_id, relation_type) in &edges {
		parents_map
			.entry(child_id.clone())
			.or_default()
			.push((parent_id.clone(), relation_type.clone()));
	}

	// Recursively build the tree
	let graph = build_parent_graph(&root_item, None, &items_map, &parents_map);

	info!("Built parent graph for item {}", item_id);
	Ok(Json(graph))
}

/// Loads items by IDs into a HashMap for efficient lookup
fn load_items_map(pool: &DbPool, item_ids: &[String]) -> Result<HashMap<String, Item>, ApiError> {
	let mut map = HashMap::new();
	for id in item_ids {
		if let Some(item) = repo::get_item(pool, id).map_err(ApiError::Database)? {
			map.insert(id.clone(), item);
		}
	}
	Ok(map)
}

/// Recursively builds a children graph node
fn build_children_graph(
	item: &Item,
	relation_type: Option<String>,
	items_map: &HashMap<String, Item>,
	children_map: &HashMap<String, Vec<(String, String)>>,
) -> ItemChildGraphNode {
	let children = children_map
		.get(&item.get_id())
		.map(|child_entries| {
			child_entries
				.iter()
				.filter_map(|(child_id, rel_type)| {
					items_map.get(child_id).map(|child_item| {
						build_children_graph(
							child_item,
							Some(rel_type.clone()),
							items_map,
							children_map,
						)
					})
				})
				.collect()
		})
		.unwrap_or_default();

	ItemChildGraphNode {
		item: item.clone(),
		relation_type,
		children,
	}
}

/// Recursively builds a parent graph node
fn build_parent_graph(
	item: &Item,
	relation_type: Option<String>,
	items_map: &HashMap<String, Item>,
	parents_map: &HashMap<String, Vec<(String, String)>>,
) -> ItemParentGraphNode {
	let parents = parents_map
		.get(&item.get_id())
		.map(|parent_entries| {
			parent_entries
				.iter()
				.filter_map(|(parent_id, rel_type)| {
					items_map.get(parent_id).map(|parent_item| {
						build_parent_graph(
							parent_item,
							Some(rel_type.clone()),
							items_map,
							parents_map,
						)
					})
				})
				.collect()
		})
		.unwrap_or_default();

	ItemParentGraphNode {
		item: item.clone(),
		relation_type,
		parents,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::repo;
	use crate::test_utils::*;
	use serde_json::json;

	/// Helper to create an item type and item for testing
	async fn create_test_item(pool: &DbPool, title: &str) -> Item {
		let item_type = repo::create_item_type(pool, "Test Type".to_string(), "fsrs".to_string())
			.await
			.unwrap();

		repo::create_item(
			pool,
			&item_type.get_id(),
			title.to_string(),
			json!({"front": "Hello", "back": "World"}),
		)
		.await
		.unwrap()
	}

	async fn create_test_item_with_type(pool: &DbPool, item_type_id: &str, title: &str) -> Item {
		repo::create_item(
			pool,
			item_type_id,
			title.to_string(),
			json!({"front": "Hello", "back": "World"}),
		)
		.await
		.unwrap()
	}

	#[tokio::test]
	async fn test_create_item_relation_handler_success() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;

		let result = create_item_relation_handler(
			State(pool.clone()),
			Path((item_a.get_id(), item_b.get_id())),
			Json(CreateItemRelationDto {
				relation_type: "extract".to_string(),
			}),
		)
		.await
		.unwrap();

		let relation = result.0;
		assert_eq!(relation.get_parent_item_id(), item_a.get_id());
		assert_eq!(relation.get_child_item_id(), item_b.get_id());
		assert_eq!(relation.get_relation_type(), "extract");
	}

	#[tokio::test]
	async fn test_create_item_relation_handler_not_found() {
		let pool = setup_test_db();

		let result = create_item_relation_handler(
			State(pool.clone()),
			Path(("nonexistent".to_string(), "also-nonexistent".to_string())),
			Json(CreateItemRelationDto {
				relation_type: "extract".to_string(),
			}),
		)
		.await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ApiError::NotFound));
	}

	#[tokio::test]
	async fn test_create_item_relation_handler_cycle_detected() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
		let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

		// A -> B -> C
		repo::create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_b.get_id(), &item_c.get_id(), "extract")
			.await
			.unwrap();

		// C -> A should return 409
		let result = create_item_relation_handler(
			State(pool.clone()),
			Path((item_c.get_id(), item_a.get_id())),
			Json(CreateItemRelationDto {
				relation_type: "extract".to_string(),
			}),
		)
		.await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ApiError::CycleDetected));
	}

	#[tokio::test]
	async fn test_delete_item_relation_handler_success() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;

		repo::create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
			.await
			.unwrap();

		let result = delete_item_relation_handler(
			State(pool.clone()),
			Path((item_a.get_id(), item_b.get_id())),
		)
		.await;

		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_delete_item_relation_handler_not_found() {
		let pool = setup_test_db();

		let result = delete_item_relation_handler(
			State(pool.clone()),
			Path(("nonexistent".to_string(), "also-nonexistent".to_string())),
		)
		.await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ApiError::NotFound));
	}

	#[tokio::test]
	async fn test_list_item_relations_handler() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;

		repo::create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
			.await
			.unwrap();

		let result = list_item_relations_handler(
			State(pool.clone()),
			Query(ListItemRelationsQueryDto::default()),
		)
		.await
		.unwrap();

		assert_eq!(result.0.len(), 1);
	}

	#[tokio::test]
	async fn test_get_children_graph_handler_success() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
		let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

		// A -> B -> C
		repo::create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_b.get_id(), &item_c.get_id(), "cloze")
			.await
			.unwrap();

		let result = get_children_graph_handler(State(pool.clone()), Path(item_a.get_id()))
			.await
			.unwrap();

		let graph = result.0;
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
		assert_eq!(child_c.children.len(), 0);
	}

	#[tokio::test]
	async fn test_get_children_graph_handler_not_found() {
		let pool = setup_test_db();

		let result =
			get_children_graph_handler(State(pool.clone()), Path("nonexistent".to_string())).await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ApiError::NotFound));
	}

	#[tokio::test]
	async fn test_get_parent_graph_handler_success() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
		let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

		// A -> B -> C
		repo::create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_b.get_id(), &item_c.get_id(), "cloze")
			.await
			.unwrap();

		let result = get_parent_graph_handler(State(pool.clone()), Path(item_c.get_id()))
			.await
			.unwrap();

		let graph = result.0;
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
		assert_eq!(parent_a.parents.len(), 0);
	}

	#[tokio::test]
	async fn test_get_parent_graph_handler_not_found() {
		let pool = setup_test_db();

		let result =
			get_parent_graph_handler(State(pool.clone()), Path("nonexistent".to_string())).await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ApiError::NotFound));
	}
}

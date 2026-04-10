use axum::{
	Json,
	extract::{Path, Query, State},
};
use std::sync::Arc;
use tracing::{debug, info, instrument};

use crate::db::DbPool;
use crate::dto::{
	CreateItemRelationDto, ItemChildGraphNode, ItemParentGraphNode, ListItemRelationsQueryDto,
};
use crate::errors::ApiError;
use crate::models::ItemRelation;
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

	let root_item = repo::get_item(&pool, &item_id)
		.map_err(ApiError::Database)?
		.ok_or(ApiError::NotFound)?;

	let graph = repo::get_children_graph(&pool, &root_item).map_err(ApiError::Database)?;

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

	let root_item = repo::get_item(&pool, &item_id)
		.map_err(ApiError::Database)?
		.ok_or(ApiError::NotFound)?;

	let graph = repo::get_parent_graph(&pool, &root_item).map_err(ApiError::Database)?;

	info!("Built parent graph for item {}", item_id);
	Ok(Json(graph))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::dto::ListItemRelationsQueryDto;
	use crate::repo;
	use crate::test_utils::*;
	use serde_json::json;

	/// Helper to create an item type and item for testing
	async fn create_test_item(pool: &DbPool, title: &str) -> crate::models::Item {
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

	async fn create_test_item_with_type(
		pool: &DbPool,
		item_type_id: &str,
		title: &str,
	) -> crate::models::Item {
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
	async fn test_list_item_relations_handler_with_parent_filter() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
		let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

		repo::create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_a.get_id(), &item_c.get_id(), "cloze")
			.await
			.unwrap();

		// Filter by parent_item_id = item_a
		let result = list_item_relations_handler(
			State(pool.clone()),
			Query(ListItemRelationsQueryDto {
				parent_item_id: Some(item_a.get_id()),
				child_item_id: None,
				relation_type: None,
			}),
		)
		.await
		.unwrap();

		assert_eq!(result.0.len(), 2);

		// Filter by parent_item_id = item_b (no children)
		let result = list_item_relations_handler(
			State(pool.clone()),
			Query(ListItemRelationsQueryDto {
				parent_item_id: Some(item_b.get_id()),
				child_item_id: None,
				relation_type: None,
			}),
		)
		.await
		.unwrap();

		assert_eq!(result.0.len(), 0);
	}

	#[tokio::test]
	async fn test_list_item_relations_handler_with_child_filter() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
		let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

		repo::create_item_relation(&pool, &item_a.get_id(), &item_c.get_id(), "extract")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_b.get_id(), &item_c.get_id(), "cloze")
			.await
			.unwrap();

		// Filter by child_item_id = item_c
		let result = list_item_relations_handler(
			State(pool.clone()),
			Query(ListItemRelationsQueryDto {
				parent_item_id: None,
				child_item_id: Some(item_c.get_id()),
				relation_type: None,
			}),
		)
		.await
		.unwrap();

		assert_eq!(result.0.len(), 2);
	}

	#[tokio::test]
	async fn test_list_item_relations_handler_with_relation_type_filter() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
		let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;

		repo::create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_a.get_id(), &item_c.get_id(), "cloze")
			.await
			.unwrap();

		// Filter by relation_type = "cloze"
		let result = list_item_relations_handler(
			State(pool.clone()),
			Query(ListItemRelationsQueryDto {
				parent_item_id: None,
				child_item_id: None,
				relation_type: Some("cloze".to_string()),
			}),
		)
		.await
		.unwrap();

		assert_eq!(result.0.len(), 1);
		assert_eq!(result.0[0].get_child_item_id(), item_c.get_id());
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
	async fn test_get_children_graph_handler_diamond_dag() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
		let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;
		let item_d = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item D").await;

		// Diamond: A -> B, A -> C, B -> D, C -> D
		repo::create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_a.get_id(), &item_c.get_id(), "simplify")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_b.get_id(), &item_d.get_id(), "cloze")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_c.get_id(), &item_d.get_id(), "extract")
			.await
			.unwrap();

		let result = get_children_graph_handler(State(pool.clone()), Path(item_a.get_id()))
			.await
			.unwrap();

		let graph = result.0;
		assert_eq!(graph.item.get_id(), item_a.get_id());
		assert_eq!(graph.children.len(), 2);

		// Both paths should reach D
		let mut d_count = 0;
		for child in &graph.children {
			assert_eq!(child.children.len(), 1);
			if child.children[0].item.get_id() == item_d.get_id() {
				d_count += 1;
			}
		}
		assert_eq!(d_count, 2, "D should appear as a child of both B and C");
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
	async fn test_get_parent_graph_handler_diamond_dag() {
		let pool = setup_test_db();
		let item_a = create_test_item(&pool, "Item A").await;
		let item_b = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item B").await;
		let item_c = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item C").await;
		let item_d = create_test_item_with_type(&pool, &item_a.get_item_type(), "Item D").await;

		// Diamond: A -> B, A -> C, B -> D, C -> D
		repo::create_item_relation(&pool, &item_a.get_id(), &item_b.get_id(), "extract")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_a.get_id(), &item_c.get_id(), "simplify")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_b.get_id(), &item_d.get_id(), "cloze")
			.await
			.unwrap();
		repo::create_item_relation(&pool, &item_c.get_id(), &item_d.get_id(), "extract")
			.await
			.unwrap();

		let result = get_parent_graph_handler(State(pool.clone()), Path(item_d.get_id()))
			.await
			.unwrap();

		let graph = result.0;
		assert_eq!(graph.item.get_id(), item_d.get_id());
		assert_eq!(graph.parents.len(), 2);

		// Both B and C are parents of D, and both have A as parent
		let mut a_count = 0;
		for parent in &graph.parents {
			assert_eq!(parent.parents.len(), 1);
			if parent.parents[0].item.get_id() == item_a.get_id() {
				a_count += 1;
			}
		}
		assert_eq!(a_count, 2, "A should appear as parent of both B and C");
	}

	#[tokio::test]
	async fn test_get_parent_graph_handler_not_found() {
		let pool = setup_test_db();

		let result =
			get_parent_graph_handler(State(pool.clone()), Path("nonexistent".to_string())).await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ApiError::NotFound));
	}

	#[tokio::test]
	async fn test_create_item_relation_cross_type() {
		let pool = setup_test_db();

		// Create items under different item types
		let item_type_a =
			repo::create_item_type(&pool, "Type A Test".to_string(), "fsrs".to_string())
				.await
				.unwrap();
		let item_type_b =
			repo::create_item_type(&pool, "Type B Test".to_string(), "fsrs".to_string())
				.await
				.unwrap();

		let item_a = repo::create_item(
			&pool,
			&item_type_a.get_id(),
			"Item A".to_string(),
			json!({"front": "Hello", "back": "World"}),
		)
		.await
		.unwrap();

		let item_b = repo::create_item(
			&pool,
			&item_type_b.get_id(),
			"Item B".to_string(),
			json!({"front": "Foo", "back": "Bar"}),
		)
		.await
		.unwrap();

		// Cross-type relation should succeed
		let result = create_item_relation_handler(
			State(pool.clone()),
			Path((item_a.get_id(), item_b.get_id())),
			Json(CreateItemRelationDto {
				relation_type: "extract".to_string(),
			}),
		)
		.await;

		assert!(result.is_ok());
		let relation = result.unwrap().0;
		assert_eq!(relation.get_parent_item_id(), item_a.get_id());
		assert_eq!(relation.get_child_item_id(), item_b.get_id());
	}
}

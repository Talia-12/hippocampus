use crate::db::{DbPool, ExecuteWithRetry, transaction_with_retry};
use crate::dto::{ItemChildGraphNode, ItemParentGraphNode};
use crate::models::{Item, ItemId, ItemRelation};
use crate::schema::{item_relations, items};
use anyhow::{Result, anyhow};
use diesel::prelude::*;
use diesel::sql_types::{Integer, Text};
use std::collections::HashMap;
use tracing::{debug, info, instrument};

/// Helper struct for the cycle detection query result
#[derive(QueryableByName, Debug)]
struct CycleCount {
	#[diesel(sql_type = Integer)]
	count: i32,
}

/// Checks whether adding an edge from `parent_id` to `child_id` would create a cycle
///
/// Uses a recursive CTE to walk ancestors of `parent_id` and checks if `child_id`
/// is reachable. Also rejects self-loops (parent_id == child_id).
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `parent_id` - The proposed parent item ID
/// * `child_id` - The proposed child item ID
///
/// ### Returns
///
/// A Result containing true if the edge would create a cycle, false otherwise
#[instrument(skip(pool), fields(parent_id = %parent_id, child_id = %child_id))]
pub fn would_create_cycle(pool: &DbPool, parent_id: &ItemId, child_id: &ItemId) -> Result<bool> {
	debug!("Checking for cycle");

	// Self-loops are always cycles
	if parent_id == child_id {
		debug!("Self-loop detected");
		return Ok(true);
	}

	let conn = &mut pool.get()?;
	let has_cycle = would_create_cycle_conn(conn, parent_id, child_id)?;
	Ok(has_cycle)
}

/// Checks for cycles using an existing connection (for use within transactions)
fn would_create_cycle_conn(
	conn: &mut SqliteConnection,
	parent_id: &ItemId,
	child_id: &ItemId,
) -> QueryResult<bool> {
	if parent_id == child_id {
		return Ok(true);
	}

	// Walk ancestors of parent_id to see if child_id is reachable
	// If child_id is already an ancestor of parent_id, adding parent_id -> child_id
	// would create a cycle
	let count: i32 = diesel::sql_query(
		"WITH RECURSIVE ancestors(id) AS ( \
			SELECT parent_item_id FROM item_relations WHERE child_item_id = ?1 \
			UNION \
			SELECT ir.parent_item_id FROM item_relations ir \
			INNER JOIN ancestors a ON ir.child_item_id = a.id \
		) \
		SELECT COUNT(*) as count FROM ancestors WHERE id = ?2",
	)
	.bind::<Text, _>(parent_id)
	.bind::<Text, _>(child_id)
	.get_result::<CycleCount>(conn)?
	.count;

	let has_cycle = count > 0;
	debug!("Cycle check result: {}", has_cycle);
	Ok(has_cycle)
}

/// Creates a new item relation in the database
///
/// The cycle check and insert are performed atomically within an immediate
/// transaction to prevent TOCTOU races.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `parent_id` - The ID of the parent item
/// * `child_id` - The ID of the child item
/// * `relation_type` - The type of relationship
///
/// ### Returns
///
/// A Result containing the newly created ItemRelation
///
/// ### Errors
///
/// Returns an error if:
/// - The relation would create a cycle (error message contains "would create a cycle")
/// - Either item does not exist
/// - The database operation fails
#[instrument(skip(pool), fields(parent_id = %parent_id, child_id = %child_id, relation_type = %relation_type))]
pub async fn create_item_relation(
	pool: &DbPool,
	parent_id: &ItemId,
	child_id: &ItemId,
	relation_type: &str,
) -> Result<ItemRelation> {
	debug!("Creating item relation");

	let conn = &mut pool.get()?;

	let relation = ItemRelation::new(
		parent_id.clone(),
		child_id.clone(),
		relation_type.to_string(),
	);

	// Perform cycle check and insert atomically within an immediate transaction
	// with retry on transient errors (database locked/busy).
	// Returns true if inserted, false if a cycle was detected.
	// The relation is cloned once for the closure; retries re-clone inside
	// because `values()` consumes it and the FnMut may be called multiple times.
	let inserted = transaction_with_retry(conn, {
		let relation = relation.clone();
		move |conn| {
			let has_cycle = would_create_cycle_conn(conn, parent_id, child_id)?;

			if has_cycle {
				return Ok(false);
			}

			diesel::insert_into(item_relations::table)
				.values(relation.clone())
				.execute(conn)?;

			Ok(true)
		}
	})
	.await?;

	if !inserted {
		return Err(anyhow!("Adding this relation would create a cycle"));
	}

	info!(
		"Created item relation: {} -> {} ({})",
		parent_id, child_id, relation_type
	);
	Ok(relation)
}

/// Deletes an item relation from the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `parent_id` - The ID of the parent item
/// * `child_id` - The ID of the child item
///
/// ### Returns
///
/// A Result indicating success
///
/// ### Errors
///
/// Returns an error if the relation does not exist or the database operation fails
#[instrument(skip(pool), fields(parent_id = %parent_id, child_id = %child_id))]
pub async fn delete_item_relation(
	pool: &DbPool,
	parent_id: &ItemId,
	child_id: &ItemId,
) -> Result<()> {
	debug!("Deleting item relation");

	let conn = &mut pool.get()?;

	let rows_deleted = diesel::delete(
		item_relations::table.filter(
			item_relations::parent_item_id
				.eq(parent_id.clone())
				.and(item_relations::child_item_id.eq(child_id.clone())),
		),
	)
	.execute_with_retry(conn)
	.await?;

	if rows_deleted == 0 {
		return Err(anyhow!("Item relation not found"));
	}

	info!("Deleted item relation: {} -> {}", parent_id, child_id);
	Ok(())
}

/// Retrieves an item relation from the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `parent_id` - The ID of the parent item
/// * `child_id` - The ID of the child item
///
/// ### Returns
///
/// A Result containing an Option with the ItemRelation if found
#[instrument(skip(pool), fields(parent_id = %parent_id, child_id = %child_id))]
pub fn get_item_relation(
	pool: &DbPool,
	parent_id: &ItemId,
	child_id: &ItemId,
) -> Result<Option<ItemRelation>> {
	debug!("Getting item relation");

	let conn = &mut pool.get()?;

	let result = item_relations::table
		.filter(
			item_relations::parent_item_id
				.eq(parent_id)
				.and(item_relations::child_item_id.eq(child_id)),
		)
		.first::<ItemRelation>(conn)
		.optional()?;

	Ok(result)
}

/// Lists item relations with optional filters
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `parent_id_filter` - Optional parent item ID to filter by
/// * `child_id_filter` - Optional child item ID to filter by
/// * `relation_type_filter` - Optional relation type to filter by
///
/// ### Returns
///
/// A Result containing a vector of matching ItemRelations
#[instrument(skip(pool))]
pub fn list_item_relations(
	pool: &DbPool,
	parent_id_filter: Option<&ItemId>,
	child_id_filter: Option<&ItemId>,
	relation_type_filter: Option<&str>,
) -> Result<Vec<ItemRelation>> {
	debug!("Listing item relations");

	let conn = &mut pool.get()?;

	let mut query = item_relations::table.into_boxed();

	if let Some(parent_id) = parent_id_filter {
		query = query.filter(item_relations::parent_item_id.eq(parent_id));
	}

	if let Some(child_id) = child_id_filter {
		query = query.filter(item_relations::child_item_id.eq(child_id));
	}

	if let Some(relation_type) = relation_type_filter {
		query = query.filter(item_relations::relation_type.eq(relation_type));
	}

	let results = query.load::<ItemRelation>(conn)?;

	info!("Retrieved {} item relations", results.len());
	Ok(results)
}

/// A directed edge in the item relation graph
#[derive(QueryableByName, Debug, Clone, PartialEq, Eq)]
pub struct RelationEdge {
	/// The parent item ID
	#[diesel(sql_type = Text)]
	pub parent_id: ItemId,

	/// The child item ID
	#[diesel(sql_type = Text)]
	pub child_id: ItemId,

	/// The type of relationship
	#[diesel(sql_type = Text)]
	pub relation_type: String,
}

/// Gets all descendant edges reachable from the given item
///
/// Uses a recursive CTE to traverse the graph downward. Returns all
/// edges in the subtree.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The root item ID to traverse from
///
/// ### Returns
///
/// A Result containing a vector of Edge structs
#[instrument(skip(pool), fields(item_id = %item_id))]
pub fn get_all_descendants(pool: &DbPool, item_id: &ItemId) -> Result<Vec<RelationEdge>> {
	debug!("Getting all descendants");

	let conn = &mut pool.get()?;

	let edges: Vec<RelationEdge> = diesel::sql_query(
		"WITH RECURSIVE descendants(parent_id, child_id, relation_type) AS ( \
			SELECT parent_item_id, child_item_id, relation_type \
			FROM item_relations WHERE parent_item_id = ?1 \
			UNION \
			SELECT ir.parent_item_id, ir.child_item_id, ir.relation_type \
			FROM item_relations ir \
			INNER JOIN descendants d ON ir.parent_item_id = d.child_id \
		) \
		SELECT parent_id, child_id, relation_type FROM descendants",
	)
	.bind::<Text, _>(item_id)
	.load(conn)?;

	info!(
		"Found {} descendant edges for item {}",
		edges.len(),
		item_id
	);
	Ok(edges)
}

/// Gets all ancestor edges reachable from the given item
///
/// Uses a recursive CTE to traverse the graph upward. Returns all
/// edges in the ancestor chain.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The item ID to find ancestors of
///
/// ### Returns
///
/// A Result containing a vector of Edge structs
#[instrument(skip(pool), fields(item_id = %item_id))]
pub fn get_all_ancestors(pool: &DbPool, item_id: &ItemId) -> Result<Vec<RelationEdge>> {
	debug!("Getting all ancestors");

	let conn = &mut pool.get()?;

	let edges: Vec<RelationEdge> = diesel::sql_query(
		"WITH RECURSIVE ancestors(parent_id, child_id, relation_type) AS ( \
			SELECT parent_item_id, child_item_id, relation_type \
			FROM item_relations WHERE child_item_id = ?1 \
			UNION \
			SELECT ir.parent_item_id, ir.child_item_id, ir.relation_type \
			FROM item_relations ir \
			INNER JOIN ancestors a ON ir.child_item_id = a.parent_id \
		) \
		SELECT parent_id, child_id, relation_type FROM ancestors",
	)
	.bind::<Text, _>(item_id)
	.load(conn)?;

	info!("Found {} ancestor edges for item {}", edges.len(), item_id);
	Ok(edges)
}

/// Gets the direct child item IDs of a parent
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `parent_id` - The parent item ID
///
/// ### Returns
///
/// A Result containing a vector of child item IDs
#[instrument(skip(pool), fields(parent_id = %parent_id))]
pub fn get_children_of(pool: &DbPool, parent_id: &ItemId) -> Result<Vec<ItemId>> {
	debug!("Getting children of item");

	let conn = &mut pool.get()?;

	let results = item_relations::table
		.filter(item_relations::parent_item_id.eq(parent_id))
		.select(item_relations::child_item_id)
		.load::<ItemId>(conn)?;

	info!("Found {} children of item {}", results.len(), parent_id);
	Ok(results)
}

/// Gets the direct parent item IDs of a child
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `child_id` - The child item ID
///
/// ### Returns
///
/// A Result containing a vector of parent item IDs
#[instrument(skip(pool), fields(child_id = %child_id))]
pub fn get_parents_of(pool: &DbPool, child_id: &ItemId) -> Result<Vec<ItemId>> {
	debug!("Getting parents of item");

	let conn = &mut pool.get()?;

	let results = item_relations::table
		.filter(item_relations::child_item_id.eq(child_id))
		.select(item_relations::parent_item_id)
		.load::<ItemId>(conn)?;

	info!("Found {} parents of item {}", results.len(), child_id);
	Ok(results)
}

/// Loads items by IDs into a HashMap for efficient lookup
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_ids` - A slice of item ID strings to load
///
/// ### Returns
///
/// A Result containing a HashMap mapping item IDs to Items
fn load_items_map(pool: &DbPool, item_ids: &[ItemId]) -> Result<HashMap<ItemId, Item>> {
	let conn = &mut pool.get()?;

	let loaded_items: Vec<Item> = items::table
		.filter(items::id.eq_any(item_ids))
		.load::<Item>(conn)?;

	let map = loaded_items
		.into_iter()
		.map(|item| (item.get_id(), item))
		.collect();

	Ok(map)
}

/// Recursively builds a children graph node
fn build_children_graph(
	item: &Item,
	relation_type: Option<String>,
	items_map: &HashMap<ItemId, Item>,
	children_map: &HashMap<ItemId, Vec<(ItemId, String)>>,
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
	items_map: &HashMap<ItemId, Item>,
	parents_map: &HashMap<ItemId, Vec<(ItemId, String)>>,
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

/// Builds the full children graph for an item
///
/// Fetches all descendant edges, batch-loads all referenced items,
/// and assembles a nested tree structure.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `root_item` - The root item to build the graph from
///
/// ### Returns
///
/// A Result containing the root ItemChildGraphNode with nested children
#[instrument(skip(pool, root_item), fields(item_id = %root_item.get_id()))]
pub fn get_children_graph(pool: &DbPool, root_item: &Item) -> Result<ItemChildGraphNode> {
	debug!("Building children graph");

	let item_id = root_item.get_id();
	let edges = get_all_descendants(pool, &item_id)?;

	// Collect all item IDs (root + all children from edges)
	let mut item_ids: Vec<_> = edges.iter().map(|e| e.child_id.clone()).collect();
	item_ids.push(item_id.clone());
	item_ids.sort();
	item_ids.dedup();

	// Batch load all items
	let items_map = load_items_map(pool, &item_ids)?;

	// Build adjacency list: parent_id -> [(child_id, relation_type)]
	let mut children_map: HashMap<ItemId, Vec<(ItemId, String)>> = HashMap::new();
	for edge in &edges {
		children_map
			.entry(edge.parent_id.clone())
			.or_default()
			.push((edge.child_id.clone(), edge.relation_type.clone()));
	}

	let graph = build_children_graph(root_item, None, &items_map, &children_map);

	info!("Built children graph for item {}", item_id);
	Ok(graph)
}

/// Builds the full parent graph for an item
///
/// Fetches all ancestor edges, batch-loads all referenced items,
/// and assembles a nested tree structure.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `root_item` - The root item to build the graph from
///
/// ### Returns
///
/// A Result containing the root ItemParentGraphNode with nested parents
#[instrument(skip(pool, root_item), fields(item_id = %root_item.get_id()))]
pub fn get_parent_graph(pool: &DbPool, root_item: &Item) -> Result<ItemParentGraphNode> {
	debug!("Building parent graph");

	let item_id = root_item.get_id();
	let edges = get_all_ancestors(pool, &item_id)?;

	// Collect all item IDs (root + all parents from edges)
	let mut item_ids: Vec<_> = edges.iter().map(|e| e.parent_id.clone()).collect();
	item_ids.push(item_id.clone());
	item_ids.sort();
	item_ids.dedup();

	// Batch load all items
	let items_map = load_items_map(pool, &item_ids)?;

	// Build adjacency list: child_id -> [(parent_id, relation_type)]
	let mut parents_map: HashMap<ItemId, Vec<(ItemId, String)>> = HashMap::new();
	for edge in &edges {
		parents_map
			.entry(edge.child_id.clone())
			.or_default()
			.push((edge.parent_id.clone(), edge.relation_type.clone()));
	}

	let graph = build_parent_graph(root_item, None, &items_map, &parents_map);

	info!("Built parent graph for item {}", item_id);
	Ok(graph)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod prop_tests;

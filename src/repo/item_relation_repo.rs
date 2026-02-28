use crate::db::{DbPool, ExecuteWithRetry};
use crate::models::ItemRelation;
use crate::schema::item_relations;
use anyhow::{Result, anyhow};
use diesel::prelude::*;
use diesel::sql_types::{Integer, Text};
use tracing::{debug, info, instrument};

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
pub fn would_create_cycle(pool: &DbPool, parent_id: &str, child_id: &str) -> Result<bool> {
	debug!("Checking for cycle");

	// Self-loops are always cycles
	if parent_id == child_id {
		debug!("Self-loop detected");
		return Ok(true);
	}

	let conn = &mut pool.get()?;

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

/// Helper struct for the cycle detection query result
#[derive(QueryableByName, Debug)]
struct CycleCount {
	#[diesel(sql_type = Integer)]
	count: i32,
}

/// Creates a new item relation in the database
///
/// Validates that both items exist and that the relation would not create a cycle
/// before inserting.
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
	parent_id: &str,
	child_id: &str,
	relation_type: &str,
) -> Result<ItemRelation> {
	debug!("Creating item relation");

	// Check for cycles before inserting
	if would_create_cycle(pool, parent_id, child_id)? {
		return Err(anyhow!("Adding this relation would create a cycle"));
	}

	let conn = &mut pool.get()?;

	let relation = ItemRelation::new(
		parent_id.to_string(),
		child_id.to_string(),
		relation_type.to_string(),
	);

	diesel::insert_into(item_relations::table)
		.values(relation.clone())
		.execute_with_retry(conn)
		.await?;

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
pub async fn delete_item_relation(pool: &DbPool, parent_id: &str, child_id: &str) -> Result<()> {
	debug!("Deleting item relation");

	let conn = &mut pool.get()?;

	let rows_deleted = diesel::delete(
		item_relations::table.filter(
			item_relations::parent_item_id
				.eq(parent_id.to_string())
				.and(item_relations::child_item_id.eq(child_id.to_string())),
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
	parent_id: &str,
	child_id: &str,
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
	parent_id_filter: Option<&str>,
	child_id_filter: Option<&str>,
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

/// Helper struct for graph traversal query results
#[derive(QueryableByName, Debug)]
struct EdgeRow {
	#[diesel(sql_type = Text)]
	parent_id: String,
	#[diesel(sql_type = Text)]
	child_id: String,
	#[diesel(sql_type = Text)]
	relation_type: String,
}

/// Gets all descendant edges reachable from the given item
///
/// Uses a recursive CTE to traverse the graph downward. Returns all
/// (parent_id, child_id, relation_type) edges in the subtree.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The root item ID to traverse from
///
/// ### Returns
///
/// A Result containing a vector of (parent_id, child_id, relation_type) tuples
#[instrument(skip(pool), fields(item_id = %item_id))]
pub fn get_all_descendants(pool: &DbPool, item_id: &str) -> Result<Vec<(String, String, String)>> {
	debug!("Getting all descendants");

	let conn = &mut pool.get()?;

	let rows: Vec<EdgeRow> = diesel::sql_query(
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

	let result: Vec<(String, String, String)> = rows
		.into_iter()
		.map(|r| (r.parent_id, r.child_id, r.relation_type))
		.collect();

	info!(
		"Found {} descendant edges for item {}",
		result.len(),
		item_id
	);
	Ok(result)
}

/// Gets all ancestor edges reachable from the given item
///
/// Uses a recursive CTE to traverse the graph upward. Returns all
/// (parent_id, child_id, relation_type) edges in the ancestor chain.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The item ID to find ancestors of
///
/// ### Returns
///
/// A Result containing a vector of (parent_id, child_id, relation_type) tuples
#[instrument(skip(pool), fields(item_id = %item_id))]
pub fn get_all_ancestors(pool: &DbPool, item_id: &str) -> Result<Vec<(String, String, String)>> {
	debug!("Getting all ancestors");

	let conn = &mut pool.get()?;

	let rows: Vec<EdgeRow> = diesel::sql_query(
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

	let result: Vec<(String, String, String)> = rows
		.into_iter()
		.map(|r| (r.parent_id, r.child_id, r.relation_type))
		.collect();

	info!("Found {} ancestor edges for item {}", result.len(), item_id);
	Ok(result)
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
/// A Result containing a vector of child item ID strings
#[instrument(skip(pool), fields(parent_id = %parent_id))]
pub fn get_children_of(pool: &DbPool, parent_id: &str) -> Result<Vec<String>> {
	debug!("Getting children of item");

	let conn = &mut pool.get()?;

	let results = item_relations::table
		.filter(item_relations::parent_item_id.eq(parent_id))
		.select(item_relations::child_item_id)
		.load::<String>(conn)?;

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
/// A Result containing a vector of parent item ID strings
#[instrument(skip(pool), fields(child_id = %child_id))]
pub fn get_parents_of(pool: &DbPool, child_id: &str) -> Result<Vec<String>> {
	debug!("Getting parents of item");

	let conn = &mut pool.get()?;

	let results = item_relations::table
		.filter(item_relations::child_item_id.eq(child_id))
		.select(item_relations::parent_item_id)
		.load::<String>(conn)?;

	info!("Found {} parents of item {}", results.len(), child_id);
	Ok(results)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod prop_tests;

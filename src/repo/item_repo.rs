use std::collections::HashSet;

use crate::db::{DbPool, ExecuteWithRetry};
use crate::dto::GetQueryDto;
use crate::models::{Item, ItemId, ItemTypeId, JsonValue};
use crate::schema::items;
use anyhow::Result;
use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use tracing::{debug, info, instrument};

use super::card_repo::{create_cards_for_item, list_cards_with_filters};

/// Creates a new item in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_type_id` - The ID of the item type for this item
/// * `new_title` - The title for the new item
/// * `item_data` - JSON data specific to this item type
///
/// ### Returns
///
/// A Result containing the newly created Item if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
#[instrument(skip(pool, item_data), fields(item_type_id = %item_type_id, title = %new_title))]
pub async fn create_item(
	pool: &DbPool,
	item_type_id: &ItemTypeId,
	new_title: String,
	item_data: serde_json::Value,
) -> Result<Item> {
	debug!("Creating new item");

	// Get a connection from the pool
	let mut conn = pool.get()?;

	// Create a new item with the provided title
	let new_item = Item::new(item_type_id.clone(), new_title, JsonValue(item_data));

	debug!(
		"Inserting item into database with id: {}",
		new_item.get_id()
	);

	// Insert the new item into the database
	diesel::insert_into(items::table)
		.values(new_item.clone())
		.execute_with_retry(&mut conn)
		.await?;

	// Drop the connection back to the pool
	drop(conn);

	debug!("Creating cards for item");

	// Create all necessary cards for the item
	create_cards_for_item(pool, &new_item).await?;

	// TODO: If there's an error, we should delete the item and all its cards

	info!("Successfully created item with id: {}", new_item.get_id());

	// Return the newly created item
	Ok(new_item)
}

/// Retrieves an item from the database by its ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to retrieve
///
/// ### Returns
///
/// A Result containing an Option with the Item if found, or None if not found
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails for reasons other than the item not existing
#[instrument(skip(pool), fields(item_id = %item_id))]
pub fn get_item(pool: &DbPool, item_id: &ItemId) -> Result<Option<Item>> {
	debug!("Retrieving item by id");

	// Get a connection from the pool
	let conn = &mut pool.get()?;

	// Query the database for the item with the specified ID
	let result = items::table
		.filter(items::id.eq(item_id))
		.first::<Item>(conn)
		.optional()?;

	if let Some(ref item) = result {
		debug!("Item found with id: {}", item.get_id());
	} else {
		debug!("Item not found");
	}

	// Return the result (Some(Item) if found, None if not)
	Ok(result)
}

/// Updates an item in the database by its ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to update
/// * `title` - The new title for the item
/// * `item_data` - The new JSON data for the item
///
/// ### Returns
///
/// A Result containing the updated Item if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database update operation fails
/// - The item is not found
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn update_item(
	pool: &DbPool,
	item_id: &ItemId,
	title: Option<String>,
	item_data: Option<serde_json::Value>,
) -> Result<Item> {
	debug!("Updating item by id");

	// Get the current item to check if it exists
	let _item = get_item(pool, item_id)?
		.ok_or_else(|| anyhow::anyhow!("Item with id {} not found", item_id))?;

	// Always update the updated_at timestamp
	let now = Utc::now().naive_utc();

	// Create a struct for changeset that implements AsChangeset
	// This allows us to only include fields that are Some
	#[derive(AsChangeset)]
	#[diesel(table_name = items)]
	struct ItemChangeset {
		title: Option<String>,
		item_data: Option<JsonValue>,
		updated_at: NaiveDateTime,
	}

	let changeset = ItemChangeset {
		title,
		item_data: item_data.map(JsonValue),
		updated_at: now,
	};

	let mut conn = pool.get()?;

	// Execute the update with the dynamic changeset
	diesel::update(items::table.find(item_id.clone()))
		.set(changeset)
		.execute_with_retry(&mut conn)
		.await?;

	drop(conn);

	// Get the updated item
	let updated_item = get_item(pool, item_id)?
		.ok_or_else(|| panic!("Item with id {} not found after update", item_id))?; // this should panic because updating the item should never result in the item being deleted

	Ok(updated_item)
}

/// Deletes an item from the database by its ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to delete
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database delete operation fails
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn delete_item(pool: &DbPool, item_id: &ItemId) -> Result<()> {
	debug!("Deleting item by id");

	let mut conn = pool.get()?;

	diesel::delete(items::table.find(item_id.clone()))
		.execute_with_retry(&mut conn)
		.await?;

	debug!("Successfully deleted item with id: {}", item_id);
	Ok(())
}

/// Retrieves all items from the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result containing a vector of all Items in the database
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
#[instrument(skip(pool))]
pub fn list_items(pool: &DbPool) -> Result<Vec<Item>> {
	debug!("Listing all items");

	// Get a connection from the pool
	let conn = &mut pool.get()?;

	// Query the database for all items
	let result = items::table.load::<Item>(conn)?;

	info!("Retrieved {} items", result.len());

	// Return the list of items
	Ok(result)
}

/// Lists items with optional filters from GetQueryDto
///
/// If the query only has `item_type_id` set (no card-level filters), filters items directly.
/// Otherwise, queries cards with `list_cards_with_filters`, collects unique item IDs,
/// and loads those items.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `query` - The query filters
///
/// ### Returns
///
/// A Result containing a vector of matching Items
#[instrument(skip(pool), fields(query = ?query))]
pub fn list_items_with_filters(pool: &DbPool, query: &GetQueryDto) -> Result<Vec<Item>> {
	debug!("Listing items with filters");

	let has_card_filters = query.next_review_before.is_some()
		|| query.last_review_after.is_some()
		|| query.suspended_after.is_some()
		|| query.suspended_before.is_some()
		|| !query.tag_ids.is_empty()
		|| query.suspended_filter != crate::dto::SuspendedFilter::default();

	// Resolve relation filters into a set of allowed item IDs
	let relation_item_ids: Option<HashSet<ItemId>> = resolve_relation_filter(pool, query)?;

	if !has_card_filters {
		// Only item_type_id filter — query items directly
		let mut result = if let Some(ref item_type_id) = query.item_type_id {
			get_items_by_type(pool, &item_type_id)?
		} else {
			list_items(pool)?
		};

		// Apply relation filter if present
		if let Some(ref allowed_ids) = relation_item_ids {
			result.retain(|item| allowed_ids.contains(&item.get_id()));
		}

		return Ok(result);
	}

	// Use card-level filtering, then collect unique item IDs
	let cards = list_cards_with_filters(pool, query)?;
	let mut item_ids: HashSet<ItemId> = cards.into_iter().map(|c| c.get_item_id()).collect();

	// Intersect with relation filter if present
	if let Some(ref allowed_ids) = relation_item_ids {
		item_ids = item_ids.intersection(allowed_ids).cloned().collect();
	}

	if item_ids.is_empty() {
		return Ok(Vec::new());
	}

	let conn = &mut pool.get()?;
	let result = items::table
		.filter(items::id.eq_any(&item_ids))
		.load::<Item>(conn)?;

	info!("Retrieved {} items from card-level filters", result.len());
	Ok(result)
}

/// Retrieves items of a specific type from the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_type_id` - The ID of the item type to filter by
///
/// ### Returns
///
/// A Result containing a vector of Items of the specified type
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
#[instrument(skip(pool), fields(item_type_id = %item_type_id))]
pub fn get_items_by_type(pool: &DbPool, item_type_id: &ItemTypeId) -> Result<Vec<Item>> {
	debug!("Getting items by type");

	// Get a connection from the pool
	let conn = &mut pool.get()?;

	// Query the database for all items of the specified type
	let result = items::table
		.filter(items::item_type.eq(item_type_id))
		.load::<Item>(conn)?;

	info!("Retrieved {} items of type {}", result.len(), item_type_id);

	// Return the list of items
	Ok(result)
}

/// Resolves parent_item_id / child_item_id relation filters into an optional
/// set of allowed item IDs.
///
/// Returns `None` if no relation filters are set, meaning no restriction.
/// Returns `Some(set)` with the item IDs that match the relation filter.
fn resolve_relation_filter(pool: &DbPool, query: &GetQueryDto) -> Result<Option<HashSet<ItemId>>> {
	match (&query.parent_item_id, &query.child_item_id) {
		(Some(parent_id), None) => {
			let children = super::item_relation_repo::get_children_of(pool, &parent_id)?;
			Ok(Some(children.into_iter().collect()))
		}
		(None, Some(child_id)) => {
			let parents = super::item_relation_repo::get_parents_of(pool, &child_id)?;
			Ok(Some(parents.into_iter().collect()))
		}
		(Some(parent_id), Some(child_id)) => {
			let children: HashSet<ItemId> =
				super::item_relation_repo::get_children_of(pool, &parent_id)?
					.into_iter()
					.collect();
			let parents: HashSet<ItemId> =
				super::item_relation_repo::get_parents_of(pool, &child_id)?
					.into_iter()
					.collect();
			Ok(Some(children.intersection(&parents).cloned().collect()))
		}
		(None, None) => Ok(None),
	}
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod prop_tests;

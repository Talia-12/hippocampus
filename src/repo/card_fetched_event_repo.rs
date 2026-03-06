use crate::card_event_registry::is_registered;
use crate::db::{DbPool, ExecuteWithRetry};
use crate::models::{CardEventFnName, CardFetchedEvent, ItemTypeId, OrderIndex};
use crate::schema::{card_fetched_events, item_types};
use anyhow::Result;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use diesel::sql_query;
use diesel::sql_types::Text;
use tracing::{debug, info, instrument};

/// Errors specific to creating a card fetched event
#[derive(Debug, thiserror::Error)]
pub enum CreateCardFetchedEventError {
	/// An event already exists for this item type with either the same
	/// `function_name` or the same `order_index`
	#[error(
		"Card fetched event already exists for this item type with the same function_name or order_index"
	)]
	Duplicate,

	/// The referenced `item_type_id` does not exist. Detected via the SQL
	/// FK violation so the check-and-insert is atomic.
	#[error("Item type not found")]
	ItemTypeNotFound,

	/// The `function_name` is not registered in the card event registry.
	#[error("Unknown card event function: {0}")]
	UnknownFunction(CardEventFnName),

	/// Any other database failure
	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

/// Creates a new card fetched event in the database
///
/// The repo owns the full atomicity contract: the registry check happens
/// here (so callers can't bypass it) and the item-type existence check is
/// delegated to SQLite's foreign-key constraint — a concurrent deletion of
/// the item type between a would-be pre-check and the insert surfaces as
/// `ItemTypeNotFound` rather than a 500.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_type_id` - The ID of the item type this event belongs to
/// * `order_index` - The position of this event in the pipeline
/// * `function_name` - The name of the function to execute
///
/// ### Returns
///
/// A Result containing the newly created CardFetchedEvent if successful.
///
/// ### Errors
///
/// * `Duplicate` — collides on `(item_type_id, function_name)` or `(item_type_id, order_index)`.
/// * `ItemTypeNotFound` — no item type with the given id exists.
/// * `UnknownFunction` — `function_name` is not in the registry.
#[instrument(skip(pool), fields(item_type_id = %item_type_id, order_index = %order_index, function_name = %function_name))]
pub async fn create_card_fetched_event(
	pool: &DbPool,
	item_type_id: &ItemTypeId,
	order_index: OrderIndex,
	function_name: CardEventFnName,
) -> Result<CardFetchedEvent, CreateCardFetchedEventError> {
	debug!("Creating new card fetched event");

	if !is_registered(&function_name) {
		return Err(CreateCardFetchedEventError::UnknownFunction(function_name));
	}

	let conn = &mut pool.get().map_err(|e| anyhow::Error::from(e))?;

	let event = CardFetchedEvent::new(item_type_id.clone(), order_index, function_name);

	match diesel::insert_into(card_fetched_events::table)
		.values(event.clone())
		.execute_with_retry(conn)
		.await
	{
		Ok(_) => {
			info!(
				"Successfully created card fetched event for item type {} at index {}",
				item_type_id, order_index
			);
			Ok(event)
		}
		Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
			Err(CreateCardFetchedEventError::Duplicate)
		}
		Err(DieselError::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _)) => {
			Err(CreateCardFetchedEventError::ItemTypeNotFound)
		}
		Err(e) => Err(CreateCardFetchedEventError::Other(anyhow::Error::from(e))),
	}
}

/// Errors specific to listing card fetched events for an item type.
///
/// Distinguishes "item type doesn't exist" (→ 404) from "item type exists
/// but has no events registered" (→ 200 with `[]`). Previously the handler
/// did two separate queries to disambiguate these; consolidating into one
/// typed error closes a TOCTOU window (item type deleted between the
/// existence check and the events read) and keeps the two query paths
/// atomic with respect to concurrent writers.
#[derive(Debug, thiserror::Error)]
pub enum ListEventsForItemTypeError {
	/// No item type exists with the given id.
	#[error("Item type not found")]
	ItemTypeNotFound,
	/// Any other database failure.
	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

/// Lists all card fetched events for an item type, ordered by order_index.
///
/// Runs the item-type existence check and the events load inside one
/// transaction so the two observe the same DB snapshot — if the item type
/// is concurrently deleted, the caller either sees both the type and its
/// events (pre-delete) or gets `ItemTypeNotFound` (post-delete), never a
/// torn view.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_type_id` - The ID of the item type to list events for
///
/// ### Returns
///
/// `Ok(Vec<CardFetchedEvent>)` — a possibly-empty list of registered events.
/// `Err(ListEventsForItemTypeError::ItemTypeNotFound)` — no such item type.
#[instrument(skip(pool), fields(item_type_id = %item_type_id))]
pub fn list_events_for_item_type(
	pool: &DbPool,
	item_type_id: &ItemTypeId,
) -> Result<Vec<CardFetchedEvent>, ListEventsForItemTypeError> {
	debug!("Listing card fetched events for item type");

	let conn = &mut pool
		.get()
		.map_err(|e| ListEventsForItemTypeError::Other(anyhow::Error::from(e)))?;

	let results: Result<Vec<CardFetchedEvent>, DieselError> =
		conn.immediate_transaction(|conn| {
			let exists: bool = item_types::table
				.find(item_type_id)
				.count()
				.get_result::<i64>(conn)?
				> 0;
			if !exists {
				// Signal "item type not found" via a Diesel rollback error we
				// convert at the boundary. Any other error is a plain DB
				// failure. Using `Error::NotFound` keeps this a single-variant
				// return type inside the transaction, which is the shape
				// `immediate_transaction` wants.
				return Err(DieselError::NotFound);
			}
			card_fetched_events::table
				.filter(card_fetched_events::item_type_id.eq(item_type_id))
				.order_by(card_fetched_events::order_index.asc())
				.load::<CardFetchedEvent>(conn)
		});

	match results {
		Ok(events) => {
			info!(
				"Retrieved {} card fetched events for item type {}",
				events.len(),
				item_type_id
			);
			Ok(events)
		}
		Err(DieselError::NotFound) => Err(ListEventsForItemTypeError::ItemTypeNotFound),
		Err(e) => Err(ListEventsForItemTypeError::Other(anyhow::Error::from(e))),
	}
}

/// Errors specific to deleting a card fetched event.
///
/// Mirrors the Create path's distinction between "the referenced item type
/// doesn't exist" (→ 404) and "the item type exists but no event with this
/// function_name is registered against it" (→ also 404, but semantically
/// distinct for logging and future API evolution). Having both variants
/// keeps the repo honest about which pre-condition failed and matches the
/// shape of `CreateCardFetchedEventError`.
#[derive(Debug, thiserror::Error)]
pub enum DeleteCardFetchedEventError {
	/// No item type exists with the given id.
	#[error("Item type not found")]
	ItemTypeNotFound,

	/// The item type exists, but no event with this function_name is
	/// registered against it.
	#[error("Card fetched event not found")]
	NotFound,

	/// Any other database failure.
	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

/// Deletes a card fetched event from the database.
///
/// Runs the item-type existence check and the delete inside one transaction
/// so the two observe the same DB snapshot — a concurrent item-type
/// deletion can't cause us to report `NotFound` when what actually happened
/// is `ItemTypeNotFound`.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_type_id` - The ID of the item type
/// * `function_name` - The function name of the event to delete
///
/// ### Returns
///
/// `Ok(())` on successful delete.
///
/// `Err(DeleteCardFetchedEventError::ItemTypeNotFound)` if no item type
/// exists with that id.
///
/// `Err(DeleteCardFetchedEventError::NotFound)` if the item type exists
/// but no event with that function_name is registered.
#[instrument(skip(pool), fields(item_type_id = %item_type_id, function_name = %function_name))]
pub async fn delete_card_fetched_event(
	pool: &DbPool,
	item_type_id: &ItemTypeId,
	function_name: &CardEventFnName,
) -> Result<(), DeleteCardFetchedEventError> {
	debug!("Deleting card fetched event");

	// We can't use `ExecuteWithRetry::execute_with_retry` inside a blocking
	// `immediate_transaction` closure (it's async). The mutation is a single
	// statement on a single row, which SQLite retries naturally via the
	// busy handler; giving up the repo-layer retry here is an acceptable
	// trade for an atomic existence + delete.
	let conn = &mut pool.get().map_err(|e| anyhow::Error::from(e))?;

	// Three outcomes from the transaction, encoded through `DieselError`
	// variants since that's what `immediate_transaction` expects:
	//   * `Ok(())` — deleted one row.
	//   * `Err(DieselError::NotFound)` — used as "item type not found" flag
	//     (we rewrite it at the boundary).
	//   * `Err(DieselError::RollbackTransaction)` — used as "event row not
	//     found" flag (item type exists, delete affected 0 rows).
	let result: Result<(), DieselError> = conn.immediate_transaction(|conn| {
		let exists: bool = item_types::table
			.find(item_type_id)
			.count()
			.get_result::<i64>(conn)?
			> 0;
		if !exists {
			return Err(DieselError::NotFound);
		}

		let deleted = diesel::delete(
			card_fetched_events::table
				.filter(card_fetched_events::item_type_id.eq(item_type_id.clone()))
				.filter(card_fetched_events::function_name.eq(function_name.clone())),
		)
		.execute(conn)?;

		if deleted == 0 {
			return Err(DieselError::RollbackTransaction);
		}

		// If this was the *last* event for the item type, clear the
		// cached `card_data` / `cache_updated_at` for every card of that
		// type. Otherwise those cards would be stranded: the cache-ensure
		// path filters to item types with at least one registered event
		// (the EXISTS guard in `card_cache::load_stale_cards_conn`), so
		// cards with no remaining events are never recomputed and would
		// keep serving the stale output from the now-deleted chain.
		//
		// Done inside the same transaction so a concurrent re-registration
		// of an event between the count check and the reset can't cause us
		// to clear caches we then immediately re-need — both actions
		// commit together or not at all.
		//
		// Note: this UPDATE touches only `card_data` and
		// `cache_updated_at`, which are excluded from the
		// `update_card_updated_at` trigger's WHEN clause — so the reset
		// itself does not bump `cards.updated_at` and does not
		// artificially invalidate other caches.
		let remaining: i64 = card_fetched_events::table
			.filter(card_fetched_events::item_type_id.eq(item_type_id.clone()))
			.count()
			.get_result(conn)?;
		if remaining == 0 {
			sql_query(
				"UPDATE cards SET card_data = NULL, cache_updated_at = NULL \
				 WHERE item_id IN (SELECT id FROM items WHERE item_type = ?)",
			)
			.bind::<Text, _>(item_type_id.0.clone())
			.execute(conn)?;
		}
		Ok(())
	});

	match result {
		Ok(()) => {
			info!(
				"Successfully deleted card fetched event '{}' for item type {}",
				function_name, item_type_id
			);
			Ok(())
		}
		Err(DieselError::NotFound) => Err(DeleteCardFetchedEventError::ItemTypeNotFound),
		Err(DieselError::RollbackTransaction) => Err(DeleteCardFetchedEventError::NotFound),
		Err(e) => Err(DeleteCardFetchedEventError::Other(anyhow::Error::from(e))),
	}
}

#[cfg(test)]
mod prop_tests;

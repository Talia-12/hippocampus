use crate::card_event_registry::CardEventChainError;
use crate::db::{DbPool, ExecuteWithRetry};
use crate::models::{Card, CardId, Item, ItemId};
use crate::repo::card_cache::{self, CacheScope, EnsureCacheError};
use crate::schema::{cards, item_tags, metadata};
use crate::{GetQueryDto, SuspendedFilter};
use anyhow::{Result, anyhow};
use chrono::Utc;
use diesel::prelude::*;
use rand::Rng;
use tracing::{debug, info, instrument, warn};

/// Creates cards for an item
///
/// This function automatically creates the necessary cards for an item
/// based on its type and data. Currently, it creates exactly one card per item.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item` - The item to create cards for
///
/// ### Returns
///
/// A Result containing a vector of the created Cards if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
#[instrument(skip(pool, item), fields(item_id = %item.get_id(), item_type = %item.get_item_type()))]
pub async fn create_cards_for_item(pool: &DbPool, item: &Item) -> Result<Vec<Card>> {
	debug!("Creating cards for item");

	// Get the item type to determine how many cards to create
	let item_type = super::get_item_type(pool, &item.get_item_type())?
		.ok_or_else(|| anyhow!("Item type not found"))?;

	debug!("Item type: {}", item_type.get_name());

	// Vector to store the created cards
	let mut cards = Vec::new();

	// Determine how many cards to create based on the item type
	match item_type.get_name().as_str() {
		"Basic" => {
			debug!("Creating basic card (front/back)");
			// Basic items have just one card (front/back)
			let card = create_card(pool, &item.get_id(), 0, 0.5).await?;
			cards.push(card);
		}
		"Cloze" => {
			debug!("Creating cloze deletion cards");
			// Cloze items might have multiple cards (one per cloze deletion)
			let data = item.get_data();
			let cloze_deletions = data.0["clozes"].clone();
			let cloze_deletions = cloze_deletions
				.as_array()
				.ok_or_else(|| anyhow!("cloze deletion must be an array"))?;

			debug!("Creating {} cloze cards", cloze_deletions.len());
			for (index, _) in cloze_deletions.iter().enumerate() {
				let card = create_card(pool, &item.get_id(), index as i32, 0.5).await?;
				cards.push(card);
			}
		}
		"Todo" => {
			debug!("Creating todo card");
			// Todo items have 1 card (each todo is a card)
			let card = create_card(pool, &item.get_id(), 0, 0.5).await?;
			cards.push(card);
		}
		// TODO: this is a hack
		name if name.contains("Test") => {
			debug!("Creating test cards");
			// Test item types have 2 cards
			for i in 0..2 {
				let card = create_card(pool, &item.get_id(), i, 0.5).await?;
				cards.push(card);
			}
		}
		_ => {
			warn!("Unknown item type: {}", item_type.get_name());
			// Return an error for unknown item types
			return Err(anyhow!(
				"Unable to construct cards for unknown item type: {}",
				item_type.get_name()
			));
		}
	}

	info!("Created {} cards for item {}", cards.len(), item.get_id());

	// Return all created cards
	Ok(cards)
}

/// Creates a new card in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item this card belongs to
/// * `card_index` - The index of this card within its item
///
/// ### Returns
///
/// A Result containing the newly created Card if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
#[instrument(skip(pool), fields(item_id = %item_id, card_index = %card_index, priority = %priority))]
pub async fn create_card(
	pool: &DbPool,
	item_id: &ItemId,
	card_index: i32,
	priority: f32,
) -> Result<Card> {
	debug!("Creating new card");

	let conn = &mut pool.get()?;

	// Create a new card for the item
	let new_card = Card::new(item_id.clone(), card_index, Utc::now(), priority);
	let new_card_id = new_card.get_id();

	debug!("Inserting card into database with id: {}", new_card_id);

	// Insert the new card into the database
	diesel::insert_into(cards::table)
		.values(new_card.clone())
		.execute_with_retry(conn)
		.await?;

	info!("Successfully created card with id: {}", new_card_id);

	// Return the newly created card
	Ok(new_card)
}

/// Bare-DB fetch of a card by id — no cache ensure pass.
///
/// Used inside the repo (existence checks, post-mutate shortcuts, and the
/// cache module itself, which MUST NOT recurse into its own invalidation
/// path). Exposed publicly so handlers that only need existence / metadata
/// (e.g. the reviews listing) can skip the chain-recompute cost of
/// `get_card`. Handlers that return `card_data` to the client must still use
/// `get_card`.
#[instrument(skip(pool), fields(card_id = %card_id))]
pub fn get_card_raw(pool: &DbPool, card_id: &CardId) -> Result<Option<Card>> {
	debug!("Retrieving card by id (raw)");

	let conn = &mut pool.get()?;

	let result = cards::table.find(card_id).first::<Card>(conn).optional()?;

	if let Some(ref card) = result {
		debug!("Card found with id: {}", card.get_id());
	} else {
		debug!("Card not found");
	}

	Ok(result)
}

/// Error surface for the cache-aware card read path.
///
/// Typed so the HTTP layer can distinguish a registry/data misconfiguration
/// (`EventChain`) from a plain database failure (`Other`) and surface a
/// clear error to the client instead of collapsing everything to 500.
/// Internal callers that just want `anyhow::Result` get automatic
/// conversion via the `thiserror`-derived `std::error::Error` impl.
#[derive(Debug, thiserror::Error)]
pub enum CardFetchError {
	/// The event chain for this card failed. In practice this is either a
	/// registry/DB drift (DB references a function name that isn't in the
	/// in-memory registry) or a registered function that errored out.
	#[error(transparent)]
	EventChain(CardEventChainError),
	/// Anything else — connection pool, Diesel, serialization. Collapsed
	/// into `anyhow::Error` because there's nothing the client can do about
	/// any of these.
	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

impl From<EnsureCacheError> for CardFetchError {
	fn from(e: EnsureCacheError) -> Self {
		match e {
			EnsureCacheError::EventChain(ch) => CardFetchError::EventChain(ch),
			EnsureCacheError::Database(de) => CardFetchError::Other(anyhow::Error::from(de)),
			EnsureCacheError::Other(ae) => CardFetchError::Other(ae),
		}
	}
}

impl From<diesel::result::Error> for CardFetchError {
	fn from(e: diesel::result::Error) -> Self {
		CardFetchError::Other(anyhow::Error::from(e))
	}
}

/// Retrieves a card by id, ensuring its `card_data` cache is fresh.
///
/// This is the canonical read path. The cache-ensure + row load happen
/// inside a single `immediate_transaction` on one pooled connection, so
/// the returned card's `card_data` is exactly what the ensure pass
/// computed (or what was already on disk, if the cache was fresh). No
/// concurrent writer can slip between the ensure pass and the read.
///
/// This replaces the old `get_card_with_cache`, which stitched together
/// three separate pool connections to load (card, item, item_type) and
/// could therefore see an inconsistent snapshot across them.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to retrieve
///
/// ### Returns
///
/// `Ok(Some(card))` — the card with fresh `card_data`.
/// `Ok(None)` — no card exists with that id.
/// `Err(CardFetchError::EventChain(_))` — the DB references a card event
/// function that's no longer in the registry, or a registered function
/// errored out. Server-side inconsistency; surface to the client.
/// `Err(CardFetchError::Other(_))` — anything else (pool, Diesel, etc).
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn get_card(pool: &DbPool, card_id: &CardId) -> Result<Option<Card>, CardFetchError> {
	Ok(card_cache::ensure_and_read_card(pool, card_id).await?)
}

/// Lists all cards in the database with optional filtering.
///
/// Synchronous, non-cache-aware — returns cards with whatever `card_data`
/// happens to be on disk. Handlers that need fresh `card_data` must go
/// through `list_cards` (async, cache-aware). Internal repo callers that
/// only need card ids / priorities / sort positions can stay here.
#[instrument(skip(pool), fields(query = %query))]
pub fn list_cards_with_filters(pool: &DbPool, query: &GetQueryDto) -> Result<Vec<Card>> {
	debug!("Listing cards with filters: {:?}", query);

	let conn = &mut pool.get()?;

	// Start with a base query that joins cards with items
	let mut card_query = cards::table.into_boxed();

	// Apply filter by item type, if specified
	if let Some(item_type_id) = &query.item_type_id {
		debug!("Filtering by item type: {}", item_type_id);
		card_query = card_query.filter(
			cards::item_id.eq_any(
				crate::schema::items::table
					.filter(crate::schema::items::item_type.eq(item_type_id))
					.select(crate::schema::items::id),
			),
		);
	}

	// Apply filter by review date, if specified
	if let Some(review_date) = query.next_review_before {
		debug!("Filtering by review date before: {}", review_date);
		card_query = card_query.filter(
			cards::next_review
				.lt(review_date.naive_utc())
				.and(cards::next_review.is_not_null()),
		);
	}

	// Apply filter by last review date, if specified
	if let Some(review_date) = query.last_review_after {
		debug!("Filtering by last review date after: {}", review_date);
		card_query = card_query.filter(
			cards::last_review
				.gt(review_date.naive_utc())
				.and(cards::last_review.is_not_null()),
		);
	}

	// Apply filter to remove suspended cards, if specified
	if query.suspended_filter == SuspendedFilter::Exclude {
		card_query = card_query.filter(cards::suspended.is_null());
	}

	// Apply filter to only include suspended cards, if specified
	if query.suspended_filter == SuspendedFilter::Only {
		card_query = card_query.filter(cards::suspended.is_not_null());
	}

	// Apply filter by suspended date before, if specified
	if let Some(suspended_date) = query.suspended_before {
		debug!("Filtering by suspended date before: {}", suspended_date);
		card_query = card_query.filter(cards::suspended.lt(suspended_date.naive_utc()));
	}

	// Apply filter by suspended date after, if specified
	if let Some(suspended_date) = query.suspended_after {
		debug!("Filtering by suspended date after: {}", suspended_date);
		card_query = card_query.filter(cards::suspended.gt(suspended_date.naive_utc()));
	}

	// Apply relation filters (parent_item_id / child_item_id)
	if let Some(ref parent_id) = query.parent_item_id {
		debug!("Filtering by parent_item_id: {}", parent_id);
		let child_ids = super::item_relation_repo::get_children_of(pool, &parent_id)?;
		card_query = card_query.filter(cards::item_id.eq_any(child_ids));
	}
	if let Some(ref child_id) = query.child_item_id {
		debug!("Filtering by child_item_id: {}", child_id);
		let parent_ids = super::item_relation_repo::get_parents_of(pool, &child_id)?;
		card_query = card_query.filter(cards::item_id.eq_any(parent_ids));
	}

	// Order by sort_position DESC (positive first, 0 = unsorted, negative last),
	// then by effective priority DESC as tiebreaker
	card_query = card_query.order_by((
		cards::sort_position.desc(),
		diesel::dsl::sql::<diesel::sql_types::Float>("(priority + priority_offset) DESC"),
	));

	// Execute the query
	let mut results = card_query.load::<Card>(conn)?;

	// Apply tag filters if specified
	// Note: This is a bit inefficient as we're filtering in Rust rather than SQL,
	// but it's simpler than constructing a complex query with multiple joins.
	if !query.tag_ids.is_empty() {
		debug!("Filtering by tags: {:?}", query.tag_ids);
		// Get all item_ids that have all the requested tags
		let mut item_ids_with_tags: Vec<ItemId> = Vec::new();

		// Get all items with any of the requested tags
		let items_with_tags: Vec<ItemId> = item_tags::table
			.filter(item_tags::tag_id.eq_any(&query.tag_ids))
			.select(item_tags::item_id)
			.load(conn)?;

		// Count how many tags each item has
		let mut item_tag_counts = std::collections::HashMap::new();
		for item_id in items_with_tags {
			*item_tag_counts.entry(item_id).or_insert(0) += 1;
		}

		// Only keep items that have all the requested tags
		for (item_id, count) in item_tag_counts {
			if count == query.tag_ids.len() {
				item_ids_with_tags.push(item_id);
			}
		}

		// Filter the results to only include cards from items with all the requested tags
		results.retain(|card| item_ids_with_tags.contains(&card.get_item_id()));
	}

	info!("Retrieved {} cards matching filters", results.len());

	Ok(results)
}

/// Gets all cards for a specific item
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to get cards for
///
/// ### Returns
///
/// A Result containing a vector of Cards belonging to the item
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
#[instrument(skip(pool), fields(item_id = %item_id))]
pub fn get_cards_for_item(pool: &DbPool, item_id: &ItemId) -> Result<Vec<Card>> {
	debug!("Getting cards for item: {}", item_id);
	let conn = &mut pool.get()?;

	// Check if the item exists
	debug!("Checking if item exists");
	let item_exists: bool = crate::schema::items::table
		.find(item_id)
		.count()
		.get_result::<i64>(conn)?
		> 0;

	if !item_exists {
		info!("Item not found: {}", item_id);
		return Err(anyhow!("Item not found"));
	}

	debug!("Item found, fetching cards");

	// Get all cards for the item
	let results = cards::table
		.filter(cards::item_id.eq(item_id))
		.order_by(cards::card_index.asc())
		.load::<Card>(conn)?;

	debug!(
		"Successfully fetched {} cards for item {}",
		results.len(),
		item_id
	);
	Ok(results)
}

/// Sets a card's suspension state
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to update
/// * `suspended` - The new suspension state for the card
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database update operation fails
/// - The card does not exist
#[instrument(skip(pool), fields(card_id = %card_id, suspended = %suspended))]
pub async fn set_card_suspended(pool: &DbPool, card_id: &CardId, suspended: bool) -> Result<()> {
	debug!(
		"Setting suspension of card to state: {}, {}",
		card_id, suspended
	);

	let card = get_card_raw(pool, card_id)?.ok_or(anyhow!("Card not found"))?;

	// Check if the suspension state is already correct
	if card.get_suspended().is_some() == suspended {
		debug!("Already at correct suspension state.");
		return Ok(());
	}

	// Set the new suspension state
	let new_suspended = if suspended {
		Some(Utc::now().naive_utc())
	} else {
		None
	};
	debug!(
		"Setting suspension of card to state: {}, {:?}",
		card_id, new_suspended
	);

	let conn = &mut pool.get()?;

	// Execute the update
	diesel::update(cards::table.find(card_id.clone()))
		.set(cards::suspended.eq(new_suspended))
		.execute_with_retry(conn)
		.await?;

	debug!(
		"Successfully set suspension of card to state: {}, {:?}",
		card_id, new_suspended
	);

	Ok(())
}

/// Updates a card in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card` - The card to update
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database update operation fails
#[instrument(skip(pool, card), fields(card_id = %card.get_id()))]
pub async fn update_card(pool: &DbPool, card: &Card) -> Result<()> {
	debug!("Updating card");
	let conn = &mut pool.get()?;

	let card_id = card.get_id();
	debug!("Executing update for card_id: {}", card_id);

	diesel::update(cards::table.find(card.get_id()))
		.set((
			cards::next_review.eq(card.get_next_review_raw()),
			cards::last_review.eq(card.get_last_review_raw()),
			cards::scheduler_data.eq(card.get_scheduler_data()),
			cards::priority.eq(card.get_priority()),
			cards::suspended.eq(card.get_suspended_raw()),
			cards::sort_position.eq(card.get_sort_position()),
			cards::priority_offset.eq(card.get_priority_offset()),
		))
		.execute_with_retry(conn)
		.await?;

	debug!("Successfully updated card_id: {}", card_id);
	Ok(())
}

/// Updates a card's priority
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to update
/// * `priority` - The new priority for the card - must be between 0 and 1
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
pub async fn update_card_priority(pool: &DbPool, card_id: &CardId, priority: f32) -> Result<Card> {
	// Check if the priority is within the valid range
	if priority < 0.0 || priority > 1.0 {
		return Err(anyhow!("Priority must be between 0 and 1"));
	}

	// Check if the card exists
	let card = get_card_raw(pool, card_id)?;
	if card.is_none() {
		return Err(anyhow!("Card not found"));
	}

	let mut conn = pool.get()?;
	diesel::update(cards::table.find(card_id.clone()))
		.set((
			cards::priority.eq(priority),
			cards::priority_offset.eq(0.0f32),
		))
		.execute_with_retry(&mut conn)
		.await?;

	drop(conn);

	// Cache-aware read so the returned card carries fresh card_data.
	let card = get_card(pool, card_id).await?;

	return Ok(card.unwrap_or_else(|| panic!("We already checked if the card exists, so this should never happen (somehow the card was deleted)")));
}

/// Moves a card to the top of the sort order
///
/// Sets the card's sort_position to MAX(sort_position) + 1.0, or 1.0 if no other cards exist.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to move to top
///
/// ### Returns
///
/// A Result containing the updated Card
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn move_card_to_top(pool: &DbPool, card_id: &CardId) -> Result<Card> {
	debug!("Moving card to top of sort order");

	// Verify card exists
	let _card = get_card_raw(pool, card_id)?.ok_or(anyhow!("Card not found"))?;

	let conn = &mut pool.get()?;

	// Find the maximum sort_position (excluding this card)
	let max_position: Option<f32> = cards::table
		.filter(cards::id.ne(card_id))
		.select(diesel::dsl::max(cards::sort_position))
		.first::<Option<f32>>(conn)?;

	let new_position = match max_position {
		Some(max) => max + 1.0,
		None => 1.0,
	};

	diesel::update(cards::table.find(card_id.clone()))
		.set(cards::sort_position.eq(new_position))
		.execute_with_retry(conn)
		.await?;

	info!(
		"Moved card {} to top with sort_position {}",
		card_id, new_position
	);

	get_card(pool, card_id)
		.await?
		.ok_or(anyhow!("Card not found after update"))
}

/// Moves a card to the bottom of the sort order
///
/// Sets the card's sort_position to MIN(sort_position) - 1.0, or -1.0 if no other cards exist.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to move to bottom
///
/// ### Returns
///
/// A Result containing the updated Card
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn move_card_to_bottom(pool: &DbPool, card_id: &CardId) -> Result<Card> {
	debug!("Moving card to bottom of sort order");

	// Verify card exists
	let _card = get_card_raw(pool, card_id)?.ok_or(anyhow!("Card not found"))?;

	let conn = &mut pool.get()?;

	// Find the minimum sort_position (excluding this card)
	let min_position: Option<f32> = cards::table
		.filter(cards::id.ne(card_id))
		.select(diesel::dsl::min(cards::sort_position))
		.first::<Option<f32>>(conn)?;

	let new_position = match min_position {
		Some(min) => min - 1.0,
		None => -1.0,
	};

	diesel::update(cards::table.find(card_id.clone()))
		.set(cards::sort_position.eq(new_position))
		.execute_with_retry(conn)
		.await?;

	info!(
		"Moved card {} to bottom with sort_position {}",
		card_id, new_position
	);

	get_card(pool, card_id)
		.await?
		.ok_or(anyhow!("Card not found after update"))
}

/// Moves a card relative to another card (before or after)
///
/// Sets the card's sort_position to the midpoint between the target and its neighbor.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to move
/// * `target_card_id` - The ID of the card to move relative to
/// * `before` - If true, move before the target; if false, move after
///
/// ### Returns
///
/// A Result containing the updated Card
#[instrument(skip(pool), fields(card_id = %card_id, target_card_id = %target_card_id, before = %before))]
pub async fn move_card_relative(
	pool: &DbPool,
	card_id: &CardId,
	target_card_id: &CardId,
	before: bool,
) -> Result<Card> {
	debug!("Moving card relative to another card");

	// Verify both cards exist
	let _card = get_card_raw(pool, card_id)?.ok_or(anyhow!("Card not found"))?;
	let target = get_card_raw(pool, target_card_id)?.ok_or(anyhow!("Target card not found"))?;

	let target_pos = target.get_sort_position();

	let conn = &mut pool.get()?;

	let new_position = if before {
		// "Before" in the queue means higher sort_position (DESC order)
		// Find the card with the next higher sort_position than target
		let predecessor: Option<f32> = cards::table
			.filter(cards::sort_position.gt(target_pos))
			.filter(cards::id.ne(card_id))
			.select(diesel::dsl::min(cards::sort_position))
			.first::<Option<f32>>(conn)?;

		match predecessor {
			Some(pred_pos) => (pred_pos + target_pos) / 2.0,
			None => target_pos + 1.0,
		}
	} else {
		// "After" in the queue means lower sort_position (DESC order)
		// Find the card with the next lower sort_position than target
		let successor: Option<f32> = cards::table
			.filter(cards::sort_position.lt(target_pos))
			.filter(cards::id.ne(card_id))
			.select(diesel::dsl::max(cards::sort_position))
			.first::<Option<f32>>(conn)?;

		match successor {
			Some(succ_pos) => (target_pos + succ_pos) / 2.0,
			None => target_pos - 1.0,
		}
	};

	diesel::update(cards::table.find(card_id.clone()))
		.set(cards::sort_position.eq(new_position))
		.execute_with_retry(conn)
		.await?;

	info!(
		"Moved card {} {} card {} with sort_position {}",
		card_id,
		if before { "before" } else { "after" },
		target_card_id,
		new_position
	);

	get_card(pool, card_id)
		.await?
		.ok_or(anyhow!("Card not found after update"))
}

/// Resets sort positions to default
///
/// Sets sort_position to 0.0 for cards matching the given query filters.
///
/// If no filters are set (default query), resets sort positions for all cards.
/// Otherwise, only resets sort positions for cards matching the filters.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `query` - A reference to the query filters to apply
///
/// ### Returns
///
/// A Result indicating success
#[instrument(skip(pool))]
pub async fn clear_sort_positions(pool: &DbPool, query: &GetQueryDto) -> Result<()> {
	debug!("Clearing sort positions with filters: {:?}", query);

	let conn = &mut pool.get()?;

	let is_default = query.item_type_id.is_none()
		&& query.tag_ids.is_empty()
		&& query.next_review_before.is_none()
		&& query.last_review_after.is_none()
		&& query.suspended_filter == SuspendedFilter::default()
		&& query.suspended_after.is_none()
		&& query.suspended_before.is_none();

	if is_default {
		info!("Empty query, clearing all cards");

		diesel::update(cards::table)
			.set(cards::sort_position.eq(0.0_f32))
			.execute_with_retry(conn)
			.await?;

		info!("Cleared all sort positions");
	} else {
		info!("Non-empty query, clearing matching cards");

		let matching_cards = list_cards_with_filters(pool, query)?;
		let count = matching_cards.len();
		let ids: Vec<CardId> = matching_cards.into_iter().map(|c| c.get_id()).collect();

		diesel::update(cards::table.filter(cards::id.eq_any(ids)))
			.set(cards::sort_position.eq(0.0_f32))
			.execute_with_retry(conn)
			.await?;

		info!("Cleared sort positions for {} matching cards", count);
	}

	Ok(())
}

/// Resets a single card's sort position
///
/// Sets the card's sort_position to 0.0 (unsorted default).
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to clear
///
/// ### Returns
///
/// A Result indicating success
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn clear_card_sort_position(pool: &DbPool, card_id: &CardId) -> Result<()> {
	debug!("Clearing sort position for card");

	let _card = get_card_raw(pool, card_id)?.ok_or(anyhow!("Card not found"))?;

	let conn = &mut pool.get()?;

	diesel::update(cards::table.find(card_id.clone()))
		.set(cards::sort_position.eq(0.0_f32))
		.execute_with_retry(conn)
		.await?;

	info!("Reset sort position for card {}", card_id);
	Ok(())
}

/// Regenerates priority offsets for all cards
///
/// Sets each card's priority_offset to a random value in [-0.05, +0.05]
/// and updates the last_offset_date in the metadata table.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result indicating success
#[instrument(skip(pool))]
pub async fn regenerate_priority_offsets(pool: &DbPool) -> Result<()> {
	debug!("Regenerating priority offsets for all cards");

	let conn = &mut pool.get()?;

	// Get all card IDs
	let card_ids: Vec<CardId> = cards::table.select(cards::id).load::<CardId>(conn)?;

	let mut rng = rand::rng();

	// Update each card with a random offset
	for id in &card_ids {
		let offset: f32 = rng.random_range(-0.05..=0.05);
		diesel::update(cards::table.find(id.clone()))
			.set(cards::priority_offset.eq(offset))
			.execute(conn)?;
	}

	// Update the last_offset_date in metadata
	let today = Utc::now().date_naive().to_string();
	diesel::replace_into(metadata::table)
		.values((
			metadata::key.eq("last_offset_date"),
			metadata::value.eq(&today),
		))
		.execute(conn)?;

	info!("Regenerated priority offsets for {} cards", card_ids.len());
	Ok(())
}

/// Ensures priority offsets are current (regenerates if stale)
///
/// Checks the last_offset_date in metadata against today's date.
/// If stale or never set, regenerates all priority offsets.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result indicating success
#[instrument(skip(pool))]
pub async fn ensure_offsets_current(pool: &DbPool) -> Result<()> {
	let today = Utc::now().date_naive().to_string();

	let is_current = {
		let conn = &mut pool.get()?;
		let last_date: Option<String> = metadata::table
			.find("last_offset_date")
			.select(metadata::value)
			.first::<String>(conn)
			.optional()?;

		matches!(last_date, Some(date) if date == today)
	};

	if is_current {
		debug!("Priority offsets are current");
		Ok(())
	} else {
		debug!("Priority offsets are stale, regenerating");
		regenerate_priority_offsets(pool).await
	}
}

/// Lists all cards in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result containing a vector of all Cards in the database
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn list_all_cards(pool: &DbPool) -> Result<Vec<Card>> {
	let conn = &mut pool.get()?;

	let results = cards::table.load::<Card>(conn)?;

	Ok(results)
}

/// Cache-aware list: ensures `card_data` is current for every card that matches
/// `query`, then returns them. This is what handlers should call when serving
/// `GET /cards`.
///
/// Scope-first: we only recompute caches for cards in the filter, not the
/// whole table (the previous implementation did the latter, which was O(N)
/// per request).
#[instrument(skip(pool, query))]
pub async fn list_cards(pool: &DbPool, query: &GetQueryDto) -> Result<Vec<Card>, CardFetchError> {
	// Two passes of `list_cards_with_filters`: one to figure out which card
	// ids need caching, one to read them back after the cache writes land.
	// Handled this way so the tag-filter logic (which happens in Rust, not
	// SQL) doesn't have to be re-expressed for the cache-ensure query.
	let candidate_ids: Vec<CardId> = list_cards_with_filters(pool, query)?
		.into_iter()
		.map(|c| c.get_id())
		.collect();
	card_cache::ensure_list_cards_cache(pool, CacheScope::Cards(&candidate_ids)).await?;
	Ok(list_cards_with_filters(pool, query)?)
}

/// Cache-aware list: ensures `card_data` is current for every card owned by
/// `item_id`, then returns them. Used by `GET /items/{item_id}/cards`.
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn list_cards_by_item(
	pool: &DbPool,
	item_id: &ItemId,
) -> Result<Vec<Card>, CardFetchError> {
	card_cache::ensure_list_cards_cache(pool, CacheScope::Item(item_id)).await?;
	Ok(get_cards_for_item(pool, item_id)?)
}

#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod tests;

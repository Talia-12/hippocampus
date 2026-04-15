use crate::card_event_registry::CardEventChainError;
use crate::db::{
	DbPool, ExecuteWithRetry, deferred_transaction_with_retry, transaction_with_retry,
};
use crate::models::{Card, CardId, Item, ItemId};
use crate::repo::card_cache::{self, CacheScope, EnsureCacheError};
use crate::repo::query_repo;
use crate::schema::{cards, metadata};
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
/// Before the cache call we run the daily-state ensure in a separate
/// DEFERRED transaction — see the body comment for why DEFERRED (and
/// not IMMEDIATE) and why the two transactions can't be merged.
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
	// DEFERRED tx: in steady state both daily markers are today and the
	// ensure closure is pure-read (SHARED only), so concurrent readers
	// don't serialize on the write lock. First call of the day upgrades
	// to RESERVED to do the regen/clear; the retry handles the unlikely
	// SQLITE_BUSY-on-upgrade race against another caller doing the same.
	//
	// We commit and drop this conn before calling into card_cache: the
	// cache opens its own IMMEDIATE on a separate pool conn, and holding
	// RESERVED on conn A while waiting for RESERVED on conn B from the
	// same pool would deadlock SQLite's single-writer model.
	{
		let conn = &mut pool
			.get()
			.map_err(|e| CardFetchError::Other(anyhow::Error::from(e)))?;
		deferred_transaction_with_retry(conn, ensure_daily_state_current)
			.await
			.map_err(|e| CardFetchError::Other(anyhow::Error::from(e)))?;
	}
	Ok(card_cache::ensure_and_read_card(pool, card_id).await?)
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
	// Precondition: priority must be in [0, 1]. This is a caller error, not
	// a DB concern, so fail fast before we open any transaction.
	if priority < 0.0 || priority > 1.0 {
		return Err(anyhow!("Priority must be between 0 and 1"));
	}

	let conn = &mut pool.get()?;

	// One IMMEDIATE transaction so the existence check, the daily offset
	// ensure, and the priority + offset=0 write commit atomically. Firing
	// the ensure *inside* the transaction is the fix for the pre-review
	// race: without it, a stale-marker day could see the next read's
	// daily regen overwrite our offset=0 reset with a random value.
	transaction_with_retry(conn, |c| {
		let exists = cards::table.find(card_id).count().get_result::<i64>(c)? > 0;
		if !exists {
			return Err(diesel::result::Error::NotFound);
		}

		ensure_offsets_current(c)?;

		diesel::update(cards::table.find(card_id.clone()))
			.set((
				cards::priority.eq(priority),
				cards::priority_offset.eq(0.0f32),
			))
			.execute(c)?;

		Ok(())
	})
	.await
	.map_err(|e| match e {
		diesel::result::Error::NotFound => anyhow!("Card not found"),
		other => anyhow::Error::from(other),
	})?;

	// Cache-aware read so the returned card carries fresh card_data.
	let card = get_card(pool, card_id).await?;

	card.ok_or_else(|| anyhow!("Card not found after priority update"))
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

	let conn = &mut pool.get()?;

	// One IMMEDIATE transaction wraps existence check + daily clear +
	// max-query + update. The ensure must fire *before* the update so
	// the new (non-zero) sort_position isn't wiped by the next read's
	// daily clear; the existence check short-circuits on not-found
	// before we pay the regen/clear cost.
	let new_position = transaction_with_retry(conn, |c| {
		let exists = cards::table.find(card_id).count().get_result::<i64>(c)? > 0;
		if !exists {
			return Err(diesel::result::Error::NotFound);
		}

		ensure_sort_positions_cleared(c)?;

		let max_position: Option<f32> = cards::table
			.filter(cards::id.ne(card_id))
			.select(diesel::dsl::max(cards::sort_position))
			.first::<Option<f32>>(c)?;

		let new_position = match max_position {
			Some(max) => max + 1.0,
			None => 1.0,
		};

		diesel::update(cards::table.find(card_id.clone()))
			.set(cards::sort_position.eq(new_position))
			.execute(c)?;

		Ok(new_position)
	})
	.await
	.map_err(|e| match e {
		diesel::result::Error::NotFound => anyhow!("Card not found"),
		other => anyhow::Error::from(other),
	})?;

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

	let conn = &mut pool.get()?;

	let new_position = transaction_with_retry(conn, |c| {
		let exists = cards::table.find(card_id).count().get_result::<i64>(c)? > 0;
		if !exists {
			return Err(diesel::result::Error::NotFound);
		}

		ensure_sort_positions_cleared(c)?;

		let min_position: Option<f32> = cards::table
			.filter(cards::id.ne(card_id))
			.select(diesel::dsl::min(cards::sort_position))
			.first::<Option<f32>>(c)?;

		let new_position = match min_position {
			Some(min) => min - 1.0,
			None => -1.0,
		};

		diesel::update(cards::table.find(card_id.clone()))
			.set(cards::sort_position.eq(new_position))
			.execute(c)?;

		Ok(new_position)
	})
	.await
	.map_err(|e| match e {
		diesel::result::Error::NotFound => anyhow!("Card not found"),
		other => anyhow::Error::from(other),
	})?;

	info!(
		"Moved card {} to bottom with sort_position {}",
		card_id, new_position
	);

	get_card(pool, card_id)
		.await?
		.ok_or(anyhow!("Card not found after update"))
}

/// Which id was missing when `move_card_relative` short-circuits. Lets the
/// tx body signal "not found" while preserving which-card-was-missing
/// through to the outer error — `diesel::result::Error::NotFound` has no
/// room to carry that.
enum Missing {
	Source,
	Target,
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

	let conn = &mut pool.get()?;

	// One IMMEDIATE transaction wraps existence checks + daily clear +
	// target read + neighbor query + update. Reading `target_pos`
	// *after* the ensure matters: on a stale-marker day the ensure
	// zeroes every card's sort_position, so the relative-move anchors
	// against the post-clear state rather than a pre-clear snapshot
	// that's no longer observable to anyone else.
	//
	// The closure returns `Ok(Err(Missing::…))` for the "one of the two
	// cards doesn't exist" cases so the outer error mapping can preserve
	// which id was missing — `diesel::result::Error::NotFound` has no
	// room to carry that and collapses both to a single string. Genuine
	// DB failures still come out as `Err(_)` and retry normally.
	let outcome = transaction_with_retry(conn, |c| {
		let card_exists = cards::table.find(card_id).count().get_result::<i64>(c)? > 0;
		if !card_exists {
			return Ok(Err(Missing::Source));
		}
		let target_exists = cards::table
			.find(target_card_id)
			.count()
			.get_result::<i64>(c)?
			> 0;
		if !target_exists {
			return Ok(Err(Missing::Target));
		}

		ensure_sort_positions_cleared(c)?;

		let target_pos: f32 = cards::table
			.find(target_card_id)
			.select(cards::sort_position)
			.first::<f32>(c)?;

		let new_position = if before {
			// "Before" in the queue means higher sort_position (DESC order)
			// Find the card with the next higher sort_position than target
			let predecessor: Option<f32> = cards::table
				.filter(cards::sort_position.gt(target_pos))
				.filter(cards::id.ne(card_id))
				.select(diesel::dsl::min(cards::sort_position))
				.first::<Option<f32>>(c)?;

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
				.first::<Option<f32>>(c)?;

			match successor {
				Some(succ_pos) => (target_pos + succ_pos) / 2.0,
				None => target_pos - 1.0,
			}
		};

		diesel::update(cards::table.find(card_id.clone()))
			.set(cards::sort_position.eq(new_position))
			.execute(c)?;

		Ok(Ok(new_position))
	})
	.await?;

	let new_position = match outcome {
		Ok(pos) => pos,
		Err(Missing::Source) => return Err(anyhow!("Card not found")),
		Err(Missing::Target) => return Err(anyhow!("Target card not found")),
	};

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

/// Transaction-body worker that actually zeroes every card's
/// `sort_position` and bumps the `last_sort_clear_date` marker. The
/// caller must already be inside an IMMEDIATE transaction so the bulk
/// clear and the marker write commit atomically — otherwise a partial
/// failure could leave cards reset but the marker stale, forcing a
/// redundant clear on the next request and defeating the once-per-day
/// invariant.
fn do_clear_all_sort_positions(
	conn: &mut SqliteConnection,
	today: &str,
) -> Result<(), diesel::result::Error> {
	diesel::update(cards::table)
		.set(cards::sort_position.eq(0.0_f32))
		.execute(conn)?;

	diesel::replace_into(metadata::table)
		.values((
			metadata::key.eq("last_sort_clear_date"),
			metadata::value.eq(today),
		))
		.execute(conn)?;

	Ok(())
}

/// Resets sort positions to default
///
/// Sets sort_position to 0.0 for cards matching the given query filters.
///
/// If no filters are set (default query), resets sort positions for **all**
/// cards and also bumps the `last_sort_clear_date` metadata marker — so a
/// manual full-clear is treated as today's daily reset, and
/// `ensure_sort_positions_cleared` will be a no-op for the rest of the day.
/// This mirrors `regenerate_priority_offsets`, which similarly owns both
/// the bulk write and the matching daily marker so the two are committed
/// atomically. A filtered clear leaves the marker untouched: only an
/// unfiltered clear satisfies the "every card is at 0.0" precondition the
/// marker stands for.
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
		let today = Utc::now().date_naive().to_string();

		// One IMMEDIATE transaction wraps the bulk clear and the daily
		// marker so a partial failure can't leave cards reset but the
		// marker stale (which would force a redundant clear on the next
		// request, defeating the once-per-day invariant).
		transaction_with_retry(conn, |c| do_clear_all_sort_positions(c, &today)).await?;

		info!("Cleared all sort positions");
	} else {
		info!("Non-empty query, clearing matching cards");

		// Fold the "which cards match" question into the UPDATE itself via
		// the query_repo subquery — one statement end-to-end, no round-trip
		// through a Rust Vec<CardId>.
		//
		// Uses `.execute` rather than `execute_with_retry`: the boxed
		// subquery holds a borrow of `query` and therefore isn't `'static`,
		// so it can't satisfy `execute_with_retry`'s trait bounds. This is
		// one statement on SQLite so the driver-level busy handler covers
		// transient lock retries; giving up the repo-layer retry is an
		// acceptable trade for the atomic single-statement mutation.
		let affected = diesel::update(
			cards::table.filter(cards::id.eq_any(query_repo::cards_matching(query))),
		)
		.set(cards::sort_position.eq(0.0_f32))
		.execute(conn)?;

		info!("Cleared sort positions for {} matching cards", affected);
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
	let today = Utc::now().date_naive().to_string();

	// One IMMEDIATE transaction wraps card-offset updates and the
	// `last_offset_date` metadata write so a partial failure can't leave
	// the DB with shuffled offsets but a stale staleness marker (which
	// would re-shuffle on the next request, defeating the once-per-day
	// invariant).
	let count = transaction_with_retry(conn, |c| do_regenerate_priority_offsets(c, &today)).await?;

	info!("Regenerated priority offsets for {} cards", count);
	Ok(())
}

/// Transaction-body worker that actually regenerates every card's
/// `priority_offset` to a random value in [-0.05, 0.05] and bumps the
/// `last_offset_date` marker. The caller must already be inside an
/// IMMEDIATE transaction so the row updates and the marker write commit
/// atomically — otherwise a partial failure could leave offsets shuffled
/// but the marker stale, which would redo the shuffle on the next
/// request and defeat the once-per-day invariant.
fn do_regenerate_priority_offsets(
	conn: &mut SqliteConnection,
	today: &str,
) -> Result<usize, diesel::result::Error> {
	let card_ids: Vec<CardId> = cards::table.select(cards::id).load::<CardId>(conn)?;

	let mut rng = rand::rng();
	for id in &card_ids {
		let offset: f32 = rng.random_range(-0.05..=0.05);
		diesel::update(cards::table.find(id.clone()))
			.set(cards::priority_offset.eq(offset))
			.execute(conn)?;
	}

	diesel::replace_into(metadata::table)
		.values((
			metadata::key.eq("last_offset_date"),
			metadata::value.eq(today),
		))
		.execute(conn)?;

	Ok(card_ids.len())
}

/// True iff the metadata row keyed `key` holds today's date string.
///
/// Used by the ensure functions as their staleness check. Pulled out so
/// `ensure_daily_state_current`'s fast path can read both daily markers
/// before deciding whether either ensure has work to do.
fn is_marker_today(
	conn: &mut SqliteConnection,
	key: &str,
	today: &str,
) -> Result<bool, diesel::result::Error> {
	let last_date: Option<String> = metadata::table
		.find(key)
		.select(metadata::value)
		.first::<String>(conn)
		.optional()?;
	Ok(matches!(last_date, Some(date) if date == today))
}

/// Ensures priority offsets are current. Reads the `last_offset_date`
/// marker and, if stale, regenerates every card's `priority_offset` and
/// bumps the marker; if today, returns Ok with no DB writes.
///
/// **The caller owns the transaction.** Pass a conn that's already
/// inside one (typically IMMEDIATE for write paths that mix this with a
/// guaranteed write, DEFERRED for read paths whose common case is the
/// fast-path). The marker read, the regen, and the marker write must
/// commit together with whatever other mutation guards them, otherwise
/// a partial failure could leave shuffled offsets but a stale marker
/// (and the next request would re-shuffle, defeating the once-per-day
/// invariant).
pub(crate) fn ensure_offsets_current(
	conn: &mut SqliteConnection,
) -> Result<(), diesel::result::Error> {
	let today = Utc::now().date_naive().to_string();
	if is_marker_today(conn, "last_offset_date", &today)? {
		debug!("Priority offsets are current");
		Ok(())
	} else {
		debug!("Priority offsets are stale, regenerating");
		do_regenerate_priority_offsets(conn, &today).map(|_| ())
	}
}

/// Ensures sort positions have been cleared today. Reads the
/// `last_sort_clear_date` marker and, if stale, zeroes every card's
/// `sort_position` and bumps the marker; if today, returns Ok with no
/// DB writes.
///
/// Same caller-owns-transaction contract as [`ensure_offsets_current`].
pub(crate) fn ensure_sort_positions_cleared(
	conn: &mut SqliteConnection,
) -> Result<(), diesel::result::Error> {
	let today = Utc::now().date_naive().to_string();
	if is_marker_today(conn, "last_sort_clear_date", &today)? {
		debug!("Sort positions already cleared today");
		Ok(())
	} else {
		debug!("Sort positions are stale, clearing all cards");
		do_clear_all_sort_positions(conn, &today)
	}
}

/// Combined daily-state ensure: priority offset regen + sort position
/// clear, with an up-front fast path.
///
/// Reads both `last_offset_date` and `last_sort_clear_date` first; if
/// both are today, returns Ok without entering either per-component
/// ensure — that fast path is the whole point of running this from a
/// DEFERRED transaction in the read path, since it leaves the
/// connection on SHARED only and lets concurrent readers proceed in
/// parallel. If either marker is stale, the corresponding ensure runs
/// (it re-checks its own marker, but those re-checks are cheap and
/// guarantee correctness if a partially-stale day ever arises).
///
/// **The caller owns the transaction.** Same contract as the
/// per-component ensures.
pub(crate) fn ensure_daily_state_current(
	conn: &mut SqliteConnection,
) -> Result<(), diesel::result::Error> {
	let today = Utc::now().date_naive().to_string();
	let offsets_today = is_marker_today(conn, "last_offset_date", &today)?;
	let sort_today = is_marker_today(conn, "last_sort_clear_date", &today)?;
	if offsets_today && sort_today {
		debug!("Daily state already current");
		return Ok(());
	}
	if !offsets_today {
		ensure_offsets_current(conn)?;
	}
	if !sort_today {
		ensure_sort_positions_cleared(conn)?;
	}
	Ok(())
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

/// Cache-aware list: ensures `card_data` is current for every card matching
/// `query`, then returns them. This is what handlers should call when serving
/// `GET /cards`.
///
/// Ensure and read share the *same* `query_repo::cards_matching` predicate
/// (via `CacheScope::Query`), so a card that matches the filter is always
/// cache-fresh on return — even if a concurrent writer added a new matching
/// card between the two passes: the ensure pass's `now_ms` pre-snapshot +
/// the staleness check guarantee eventual freshness on the next fetch.
///
/// Results are ordered by `sort_position DESC` (positive first, 0 = unsorted,
/// negative last), tiebroken by effective priority `(priority + priority_offset) DESC`.
#[instrument(skip(pool, query))]
pub async fn list_cards(pool: &DbPool, query: &GetQueryDto) -> Result<Vec<Card>, CardFetchError> {
	// See `get_card` for why DEFERRED + separate conn + drop-before-cache.
	{
		let conn = &mut pool
			.get()
			.map_err(|e| CardFetchError::Other(anyhow::Error::from(e)))?;
		deferred_transaction_with_retry(conn, ensure_daily_state_current)
			.await
			.map_err(|e| CardFetchError::Other(anyhow::Error::from(e)))?;
	}
	card_cache::ensure_list_cards_cache(pool, CacheScope::Query(query)).await?;
	let conn = &mut pool
		.get()
		.map_err(|e| CardFetchError::Other(anyhow::Error::from(e)))?;
	Ok(cards::table
		.filter(cards::id.eq_any(query_repo::cards_matching(query)))
		.order_by((
			cards::sort_position.desc(),
			diesel::dsl::sql::<diesel::sql_types::Float>("(priority + priority_offset) DESC"),
		))
		.load::<Card>(conn)?)
}

/// Cache-aware list: ensures `card_data` is current for every card owned by
/// `item_id`, then returns them. Used by `GET /items/{item_id}/cards`.
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn list_cards_by_item(
	pool: &DbPool,
	item_id: &ItemId,
) -> Result<Vec<Card>, CardFetchError> {
	// See `get_card` for why DEFERRED + separate conn + drop-before-cache.
	{
		let conn = &mut pool
			.get()
			.map_err(|e| CardFetchError::Other(anyhow::Error::from(e)))?;
		deferred_transaction_with_retry(conn, ensure_daily_state_current)
			.await
			.map_err(|e| CardFetchError::Other(anyhow::Error::from(e)))?;
	}
	card_cache::ensure_list_cards_cache(pool, CacheScope::Item(item_id)).await?;
	Ok(get_cards_for_item(pool, item_id)?)
}

#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod tests;

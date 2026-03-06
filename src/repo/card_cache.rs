//! Internal helpers for keeping each card's `card_data` cache fresh.
//!
//! Nothing here is part of the repo's public surface — `card_repo`
//! re-reads the `super::card_cache` helpers and calls them from `get_card`,
//! `list_cards`, and `list_cards_by_item`, so callers never see a stale
//! cache without us ever exposing a separate "cache-aware" API.
//!
//! ## Invariants this module upholds
//!
//! * `cache_updated_at = NULL`  ⇒  `card_data = NULL`.
//! * For any card whose item type has **no** registered events,
//!   `card_data` stays `NULL` — we never write an empty JSON. This is
//!   enforced at two layers: `run_event_chain` returns `None` for an empty
//!   listener list, and `load_stale_cards_conn` filters out rows whose
//!   item type has no events via an `EXISTS` subquery.
//! * When we *do* write, `cache_updated_at` is the `now_ms()` snapshot
//!   captured *before* we read the inputs, so any mutation that races us
//!   leaves the cache demonstrably older than the mutation timestamp.
//!   Next caller sees it as stale and recomputes.
//!
//! ## Atomicity
//!
//! The single-card read path (`ensure_and_read_card`) does its entire
//! load → compute → write → read sequence on one connection inside one
//! `immediate_transaction`, so there is no inter-query window where a
//! concurrent writer could interleave. List paths (`ensure_list_cards_cache`)
//! do the load-compute-write inside one transaction as well; the caller's
//! follow-up read is on a separate connection, which is acceptable for
//! list APIs where we only need *eventual* freshness.

use crate::card_event_registry::{CardEventChainError, run_event_chain};
use crate::db::DbPool;
use crate::models::{Card, CardFetchedEvent, CardId, Item, ItemId, ItemType, ItemTypeId};
use crate::schema::{card_fetched_events, cards, item_types, items};
use crate::time_utils::now_ms;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text, Timestamp};
use diesel::sqlite::{Sqlite, SqliteConnection};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, instrument};

/// Errors from the cache ensure / read path.
///
/// We keep structure here (rather than collapsing everything to
/// `anyhow::Error`) so callers — in particular the HTTP layer — can
/// distinguish a registry misconfiguration (`EventChain(FunctionsNotFound)`)
/// from a plain DB failure and surface a useful error to the client.
#[derive(Debug, thiserror::Error)]
pub(super) enum EnsureCacheError {
	/// A Diesel error bubbled up from a query or transaction.
	#[error(transparent)]
	Database(DieselError),

	/// The event chain failed — either because the DB references a function
	/// name that isn't in the in-memory registry, or because a registered
	/// function returned an error.
	#[error(transparent)]
	EventChain(#[from] CardEventChainError),

	/// Couldn't acquire a pooled connection or serialize a JSON value.
	#[error("{0}")]
	Other(anyhow::Error),
}

impl From<DieselError> for EnsureCacheError {
	fn from(e: DieselError) -> Self {
		EnsureCacheError::Database(e)
	}
}

impl From<r2d2::Error> for EnsureCacheError {
	fn from(e: r2d2::Error) -> Self {
		EnsureCacheError::Other(anyhow::Error::from(e))
	}
}

impl From<serde_json::Error> for EnsureCacheError {
	fn from(e: serde_json::Error) -> Self {
		EnsureCacheError::Other(anyhow::Error::from(e))
	}
}

/// How much of the cards table to touch when ensuring caches.
///
/// Callers pick the tightest scope their workload allows so we don't do O(N)
/// work for an O(1) request.
pub(super) enum CacheScope<'a> {
	/// Every card in the DB. Only use this if you genuinely intend to refresh
	/// all caches — e.g. after a bulk event-registry change or from test
	/// harnesses that verify end-state across arbitrary datasets.
	#[allow(dead_code)] // exercised from tests; kept for future admin paths.
	All,
	/// All cards belonging to a single item.
	Item(&'a ItemId),
	/// Exactly the listed cards. Callers can hand in an empty slice to make
	/// this a no-op.
	Cards(&'a [CardId]),
}

// ---------------------------------------------------------------------------
// Public (pub(super)) entry points — pool-based.
// ---------------------------------------------------------------------------

/// Ensures `card_data` is current for every card in `scope`, then returns.
///
/// The ensure pass runs inside a single `immediate_transaction` so
/// "load stale rows → compute chain → write cache" is atomic with respect
/// to other SQLite writers. Callers that want the card itself should use
/// `ensure_and_read_card` (single-card) to also fold the read into the same
/// transaction; list callers tolerate the connection split because they
/// re-query anyway.
#[instrument(skip(pool, scope))]
pub(super) async fn ensure_list_cards_cache(
	pool: &DbPool,
	scope: CacheScope<'_>,
) -> Result<(), EnsureCacheError> {
	// Early-out on empty explicit id list so we don't do a DB round-trip for
	// a caller that filtered to nothing.
	if let CacheScope::Cards(ids) = &scope {
		if ids.is_empty() {
			return Ok(());
		}
	}

	let mut conn = pool.get()?;
	run_with_retry(|| {
		conn.immediate_transaction::<_, EnsureCacheError, _>(|conn| {
			ensure_list_cards_cache_conn(conn, &scope)
		})
	})
	.await
}

/// Atomic single-card ensure + read.
///
/// Opens one connection and one `immediate_transaction`, and inside that:
///
/// 1. Loads the card (if it exists), together with its item and item_type,
///    and checks staleness — all in a single JOIN'd SELECT.
/// 2. If stale and the item type has registered events, computes the event
///    chain and writes the new `card_data`.
/// 3. Re-reads the card row and returns it.
///
/// Because everything happens inside one transaction on one connection,
/// the caller gets a `Card` whose `card_data` is the value that was just
/// written (or, if the cache was already fresh, the value already on disk).
/// No concurrent writer can slip between steps (2) and (3).
#[instrument(skip(pool), fields(card_id = %card_id))]
pub(super) async fn ensure_and_read_card(
	pool: &DbPool,
	card_id: &CardId,
) -> Result<Option<Card>, EnsureCacheError> {
	let mut conn = pool.get()?;
	run_with_retry(|| {
		conn.immediate_transaction::<_, EnsureCacheError, _>(|conn| {
			ensure_list_cards_cache_conn(conn, &CacheScope::Cards(std::slice::from_ref(card_id)))?;
			Ok(cards::table.find(card_id).first::<Card>(conn).optional()?)
		})
	})
	.await
}

/// Batched UPDATE of many cards' cached event-chain data.
///
/// Callable on its own (primarily for tests); the ensure path uses the
/// conn-based variant inside the transaction.
#[allow(dead_code)] // exercised from tests; kept as the test surface for batched writes.
#[instrument(skip(pool, updates), fields(updates = updates.len()))]
pub(super) async fn update_cards_cache(
	pool: &DbPool,
	updates: Vec<(CardId, serde_json::Value)>,
	now: NaiveDateTime,
) -> Result<(), EnsureCacheError> {
	if updates.is_empty() {
		return Ok(());
	}
	let mut conn = pool.get()?;
	run_with_retry(|| {
		conn.immediate_transaction::<_, EnsureCacheError, _>(|conn| {
			update_cards_cache_conn(conn, &updates, now)
		})
	})
	.await
}

// ---------------------------------------------------------------------------
// Conn-based internals. These are what the transaction body calls; they
// never open a new connection themselves, so the caller fully controls the
// transaction boundary.
// ---------------------------------------------------------------------------

fn ensure_list_cards_cache_conn(
	conn: &mut SqliteConnection,
	scope: &CacheScope<'_>,
) -> Result<(), EnsureCacheError> {
	// Capture `now` *before* the read: any writer whose trigger fires after
	// this point leaves `<thing>.updated_at > now`, so the next caller
	// observes the cache as stale. (See also the note in `load_stale_cards_conn`
	// on the `<=` staleness comparison.)
	let now = now_ms();

	let stale = load_stale_cards_conn(conn, scope)?;
	if stale.is_empty() {
		debug!("No card caches need recomputation");
		return Ok(());
	}

	let events_by_item_type = load_events_for_item_types_conn(conn, &stale)?;

	let mut updates: Vec<(CardId, serde_json::Value)> = Vec::with_capacity(stale.len());
	for (card, item, _item_type) in stale {
		let card_id = card.get_id();

		// Invariants guaranteeing these `expect`s: query (1) filters to rows
		// where `EXISTS (SELECT 1 FROM card_fetched_events WHERE item_type_id = ...)`,
		// so every stale row's item type has at least one event. Query (2)
		// loads events for every distinct item type in `stale`. Therefore:
		//   (a) the hashmap lookup is Some, and
		//   (b) the events vec is non-empty, so `run_event_chain` returns Some.
		let events = events_by_item_type
			.get(&item.get_item_type())
			.expect("load_stale EXISTS guard + load_events cover every item_type");
		let result = run_event_chain(events, &item, card)?
			.expect("non-empty events — EXISTS guard guarantees run_event_chain returns Some");
		updates.push((card_id, result));
	}

	update_cards_cache_conn(conn, &updates, now)
}

/// Executes query (1): stale cards, scoped, with an EXISTS-events guard.
fn load_stale_cards_conn(
	conn: &mut SqliteConnection,
	scope: &CacheScope<'_>,
) -> Result<Vec<(Card, Item, ItemType)>, EnsureCacheError> {
	let mut query = cards::table
		.inner_join(items::table.on(items::id.eq(cards::item_id)))
		.inner_join(item_types::table.on(item_types::id.eq(items::item_type)))
		// EXISTS-style filter: only consider cards whose item type has at
		// least one registered event. Using `eq_any` on the events table's
		// item_type_id column compiles to a SQL subquery, which avoids the
		// cartesian join across events that an earlier implementation used.
		.filter(
			items::item_type
				.eq_any(card_fetched_events::table.select(card_fetched_events::item_type_id)),
		)
		// Staleness: `<=` (not `<`) because SQLite's `strftime('%f', 'now')`
		// has ms precision, so a cross-transaction mutation that lands in
		// the same ms as a cache write must still invalidate. Worst case:
		// a card first-fetched in the same ms as its creation records
		// `cache_updated_at == card.updated_at` and gets one extra recompute
		// on the very next fetch. In steady state (once `now_ms()` advances
		// past the input timestamps, typically within 1ms of idle) the fast
		// path kicks in. We accept that one-shot wasted recompute in
		// exchange for correctness under ms-precision writer races.
		.filter(
			cards::cache_updated_at
				.is_null()
				.or(cards::cache_updated_at.le(items::updated_at.nullable()))
				.or(cards::cache_updated_at.le(cards::updated_at.nullable()))
				.or(cards::cache_updated_at.le(item_types::updated_at.nullable())),
		)
		.select((Card::as_select(), Item::as_select(), ItemType::as_select()))
		.into_boxed();

	match scope {
		CacheScope::All => {}
		CacheScope::Item(item_id) => {
			query = query.filter(cards::item_id.eq(*item_id));
		}
		CacheScope::Cards(ids) => {
			query = query.filter(cards::id.eq_any(*ids));
		}
	}

	Ok(query.load::<(Card, Item, ItemType)>(conn)?)
}

/// Executes query (2): events for the distinct item types of the
/// stale-card set, grouped by item type in memory.
fn load_events_for_item_types_conn(
	conn: &mut SqliteConnection,
	stale: &[(Card, Item, ItemType)],
) -> Result<HashMap<ItemTypeId, Vec<CardFetchedEvent>>, EnsureCacheError> {
	// Deduplicate item type ids before the IN-list so we don't pay for
	// duplicate keys when many cards share a type.
	let mut ids: Vec<ItemTypeId> = stale
		.iter()
		.map(|(_, item, _)| item.get_item_type())
		.collect();
	ids.sort();
	ids.dedup();

	let events: Vec<CardFetchedEvent> = card_fetched_events::table
		.filter(card_fetched_events::item_type_id.eq_any(&ids))
		.order_by((
			card_fetched_events::item_type_id.asc(),
			card_fetched_events::order_index.asc(),
		))
		.load::<CardFetchedEvent>(conn)?;

	let mut grouped: HashMap<ItemTypeId, Vec<CardFetchedEvent>> = HashMap::new();
	for event in events {
		grouped
			.entry(event.get_item_type_id())
			.or_default()
			.push(event);
	}
	Ok(grouped)
}

/// Batched UPDATE of many cards' cached event-chain data.
///
/// Emits one SQL statement per chunk:
///
/// ```sql
/// UPDATE cards
///    SET cache_updated_at = ?,
///        card_data = CASE id
///          WHEN ? THEN ?   -- (id_0, json_0)
///          WHEN ? THEN ?   -- (id_1, json_1)
///          ...
///        END
///  WHERE id IN (?, ?, ...);
/// ```
///
/// Chunking keeps the bound-parameter count well under SQLite's 32k limit
/// even in pessimistic older-version builds (999). Each row costs 3 params
/// (two in CASE, one in IN), plus one shared `now`.
///
/// We prefer raw SQL here over per-row Diesel DSL updates because Diesel
/// doesn't natively build multi-row CASE/WHEN UPDATEs and wrapping N DSL
/// updates in a transaction still pays for N round-trips. The SQL itself is
/// narrow and covered by dedicated tests.
const UPDATE_CHUNK_SIZE: usize = 200;

fn update_cards_cache_conn(
	conn: &mut SqliteConnection,
	updates: &[(CardId, serde_json::Value)],
	now: NaiveDateTime,
) -> Result<(), EnsureCacheError> {
	if updates.is_empty() {
		return Ok(());
	}

	debug!("Batch-updating {} card caches", updates.len());

	// Pre-serialize all card_data values once.
	let rendered: Vec<(String, String)> = updates
		.iter()
		.map(|(id, data)| serde_json::to_string(data).map(|s| (id.0.clone(), s)))
		.collect::<Result<Vec<_>, _>>()?;

	for chunk in rendered.chunks(UPDATE_CHUNK_SIZE) {
		let mut sql = String::with_capacity(200 + chunk.len() * 32);
		sql.push_str("UPDATE cards SET cache_updated_at = ?, card_data = CASE id");
		for _ in chunk.iter() {
			sql.push_str(" WHEN ? THEN ?");
		}
		sql.push_str(" END WHERE id IN (");
		for (i, _) in chunk.iter().enumerate() {
			if i > 0 {
				sql.push_str(", ");
			}
			sql.push('?');
		}
		sql.push(')');

		// BoxedSqlQuery lets us chain N `bind`s at runtime — plain
		// `sql_query(..).bind(..).bind(..)` grows the static type on each call,
		// which doesn't work for loop-built parameter lists.
		let mut query = sql_query(sql).into_boxed::<Sqlite>();
		query = query.bind::<Timestamp, _>(now);
		// CASE parameters: for each row, bind (id, json_str).
		for (id, json_str) in chunk.iter() {
			query = query
				.bind::<Text, _>(id.clone())
				.bind::<Nullable<Text>, _>(Some(json_str.clone()));
		}
		// IN parameters: the ids again.
		for (id, _) in chunk.iter() {
			query = query.bind::<Text, _>(id.clone());
		}

		query.execute(conn)?;
	}

	info!("Batch-updated card caches");
	Ok(())
}

// ---------------------------------------------------------------------------
// Transient-error retry. Mirrors the behaviour of `db::transaction_with_retry`
// but accepts a closure that returns `EnsureCacheError` (which is richer than
// `DieselError` — we need to pass chain errors through the transaction).
// ---------------------------------------------------------------------------

const INITIAL_DELAY_MS: u64 = 100;
const MAX_RETRIES: u32 = 5;

async fn run_with_retry<T, F>(mut f: F) -> Result<T, EnsureCacheError>
where
	F: FnMut() -> Result<T, EnsureCacheError>,
{
	let mut attempts = 0;
	let mut delay = Duration::from_millis(INITIAL_DELAY_MS);
	loop {
		match f() {
			Ok(v) => return Ok(v),
			Err(EnsureCacheError::Database(ref e)) if attempts < MAX_RETRIES && is_retryable(e) => {
				attempts += 1;
				sleep(delay).await;
				delay *= 2;
			}
			Err(e) => return Err(e),
		}
	}
}

fn is_retryable(err: &DieselError) -> bool {
	match err {
		DieselError::DatabaseError(DatabaseErrorKind::SerializationFailure, _) => true,
		DieselError::DatabaseError(DatabaseErrorKind::Unknown, info) => {
			let message = info.message().to_lowercase();
			message.contains("database is locked") || message.contains("database busy")
		}
		_ => false,
	}
}

#[cfg(test)]
mod prop_tests;

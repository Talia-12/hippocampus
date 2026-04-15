//! SQL-side filter builders for `GetQueryDto`.
//!
//! Each function returns a Diesel boxed select statement that, when executed,
//! yields the IDs of the rows of the corresponding table that match the query.
//! Callers use the result as a subquery, e.g.:
//!
//! ```ignore
//! cards::table
//!     .filter(cards::id.eq_any(query_repo::cards_matching(&query)))
//!     .order_by(...)
//!     .load::<Card>(conn)?;
//! ```
//!
//! Tag filtering is done in SQL via `GROUP BY ... HAVING COUNT(DISTINCT) = N`
//! — not in Rust. See `DESIRED_BEHAVIOUR.md` for the full design.

use crate::dto::{GetQueryDto, SuspendedFilter};
use crate::schema::{cards, item_relations, item_tags, items, reviews};
use diesel::dsl::{Select, count};
use diesel::expression_methods::AggregateExpressionMethods;
use diesel::helper_types::IntoBoxed;
use diesel::prelude::*;
use diesel::sqlite::Sqlite;

/// Boxed SELECT of `cards::id` matching a query.
pub type BoxedCardIdQuery<'a> = IntoBoxed<'a, Select<cards::table, cards::id>, Sqlite>;

/// Boxed SELECT of `items::id` matching a query.
pub type BoxedItemIdQuery<'a> = IntoBoxed<'a, Select<items::table, items::id>, Sqlite>;

/// Boxed SELECT of `reviews::id` matching a query.
#[allow(dead_code)] // currently only reached via tests — see `reviews_matching`.
pub type BoxedReviewIdQuery<'a> =
	IntoBoxed<'a, Select<reviews::table, reviews::id>, Sqlite>;

// ---------------------------------------------------------------------------
// Private helpers — apply one "family" of predicates to a base table.
// Both return a boxed query on the raw table (no SELECT column chosen yet);
// callers finish with `.select(col)` to get a single-column subquery.
// ---------------------------------------------------------------------------

/// Applies item-level predicates (`item_type_id`, `tag_ids`, `parent_item_id`,
/// `child_item_id`) to `items::table`.
///
/// The tag predicate uses `GROUP BY item_id HAVING COUNT(DISTINCT tag_id) = N`
/// so that:
///   * duplicate tag ids in the input (e.g. `[A, A]`) collapse to a single
///     requirement (matches Q2.6 in the proptests); and
///   * a request for *N* distinct tags is satisfied only by items that have
///     *every* one — never by items that happen to repeat a row (defensive
///     against any future schema change that drops the PK on `(item_id, tag_id)`).
///
/// Tag-id deduplication is done in Rust (not SQL) because we want `N` in the
/// HAVING clause to match `tag_ids.len()` exactly.
fn item_level_filters_on_items<'a>(
	query: &'a GetQueryDto,
) -> IntoBoxed<'a, items::table, Sqlite> {
	let mut q = items::table.into_boxed::<Sqlite>();

	if let Some(ref it) = query.item_type_id {
		q = q.filter(items::item_type.eq(it));
	}

	if !query.tag_ids.is_empty() {
		// Dedupe client-side so the `HAVING COUNT(DISTINCT tag_id) = N` count
		// lines up with the actual number of distinct tags requested. Owned
		// Vec so the boxed query can take it by value and not borrow from
		// `query.tag_ids` — keeps the lifetime story simple.
		let mut distinct = query.tag_ids.clone();
		distinct.sort();
		distinct.dedup();
		let n = distinct.len() as i64;

		q = q.filter(
			items::id.eq_any(
				item_tags::table
					.filter(item_tags::tag_id.eq_any(distinct))
					.group_by(item_tags::item_id)
					.having(count(item_tags::tag_id).aggregate_distinct().eq(n))
					.select(item_tags::item_id),
			),
		);
	}

	if let Some(ref parent_id) = query.parent_item_id {
		q = q.filter(
			items::id.eq_any(
				item_relations::table
					.filter(item_relations::parent_item_id.eq(parent_id))
					.select(item_relations::child_item_id),
			),
		);
	}

	if let Some(ref child_id) = query.child_item_id {
		q = q.filter(
			items::id.eq_any(
				item_relations::table
					.filter(item_relations::child_item_id.eq(child_id))
					.select(item_relations::parent_item_id),
			),
		);
	}

	q
}

/// Applies card-level predicates (`next_review_before`, `last_review_after`,
/// `suspended_filter`, `suspended_after`, `suspended_before`) to `cards::table`.
///
/// NULL-falsy semantics fall out of SQL's three-valued logic: `NULL > x` and
/// `NULL < x` are both `NULL` (neither TRUE nor FALSE), so rows with a NULL
/// on the compared column are excluded from the result of `.gt()` / `.lt()`.
/// The proptests in Q3.2, Q4.4, Q4.5 pin this.
fn card_level_filters_on_cards<'a>(
	query: &'a GetQueryDto,
) -> IntoBoxed<'a, cards::table, Sqlite> {
	let mut q = cards::table.into_boxed::<Sqlite>();

	if let Some(cutoff) = query.next_review_before {
		q = q.filter(cards::next_review.lt(cutoff.naive_utc()));
	}
	if let Some(cutoff) = query.last_review_after {
		q = q.filter(cards::last_review.gt(cutoff.naive_utc()));
	}
	match query.suspended_filter {
		SuspendedFilter::Include => {}
		SuspendedFilter::Exclude => q = q.filter(cards::suspended.is_null()),
		SuspendedFilter::Only => q = q.filter(cards::suspended.is_not_null()),
	}
	if let Some(cutoff) = query.suspended_after {
		q = q.filter(cards::suspended.gt(cutoff.naive_utc()));
	}
	if let Some(cutoff) = query.suspended_before {
		q = q.filter(cards::suspended.lt(cutoff.naive_utc()));
	}

	q
}

/// Matches the oracle's notion of "any filter that narrows by card
/// attributes (not item attributes)." If this is false, `items_matching`
/// doesn't need to require the existence of a matching card — every item
/// that passes item-level filters is returned regardless of card state
/// (including zero-card items; see Q7B.2).
fn has_card_level_filter(query: &GetQueryDto) -> bool {
	query.next_review_before.is_some()
		|| query.last_review_after.is_some()
		|| query.suspended_after.is_some()
		|| query.suspended_before.is_some()
		|| query.suspended_filter != SuspendedFilter::default()
}

// ---------------------------------------------------------------------------
// Public entry points.
// ---------------------------------------------------------------------------

/// Returns a boxed subquery selecting `cards::id` for every card that
/// matches `query`.
///
/// Item-level filters are folded in via a subquery on `items`; card-level
/// filters apply to `cards` directly. `split_priority` is presentational
/// and is ignored here.
pub fn cards_matching<'a>(query: &'a GetQueryDto) -> BoxedCardIdQuery<'a> {
	let item_ids_sub = item_level_filters_on_items(query).select(items::id);
	card_level_filters_on_cards(query)
		.filter(cards::item_id.eq_any(item_ids_sub))
		.select(cards::id)
}

/// Returns a boxed subquery selecting `items::id` for every item that
/// matches `query`.
///
/// Item-level filters apply directly. When any card-level filter is set,
/// additionally requires the item to own ≥1 card satisfying the card-level
/// predicates — matching the `has_card_level_filter` branch of the oracle.
pub fn items_matching<'a>(query: &'a GetQueryDto) -> BoxedItemIdQuery<'a> {
	let mut q = item_level_filters_on_items(query);
	if has_card_level_filter(query) {
		let card_item_ids = card_level_filters_on_cards(query).select(cards::item_id);
		q = q.filter(items::id.eq_any(card_item_ids));
	}
	q.select(items::id)
}

/// Returns a boxed subquery selecting `reviews::id` for every review whose
/// card matches `query`. (No review-level filters exist on `GetQueryDto` yet.)
///
/// Currently only used from tests — the proptests in `query_repo/prop_tests.rs`
/// pin its semantics, but no production caller needs filtered reviews yet.
/// Kept in the public API so it's ready the moment a handler wants it.
#[allow(dead_code)]
pub fn reviews_matching<'a>(query: &'a GetQueryDto) -> BoxedReviewIdQuery<'a> {
	reviews::table
		.filter(reviews::card_id.eq_any(cards_matching(query)))
		.select(reviews::id)
		.into_boxed()
}

#[cfg(test)]
mod prop_tests;

//! Property tests for `query_repo`.
//!
//! These tests are the spec for `cards_matching`, `items_matching`, and
//! `reviews_matching`. The strategy is the same throughout:
//!
//! 1. Set up a small "world" (a handful of item types / items / cards /
//!    tags / relations / reviews) in the test DB.
//! 2. Build a `GetQueryDto` exercising the filter under test.
//! 3. Execute the SQL subquery from `query_repo`.
//! 4. Compute the same answer in pure Rust (`oracle_*`).
//! 5. Assert the two sets of IDs are equal.
//!
//! Per `CLAUDE.md`'s testing philosophy, the goal is for the suite alone to
//! force the SQL implementation to behave correctly — including the tag
//! filter's `GROUP BY ... HAVING COUNT(DISTINCT) = N` semantics, the
//! NULL-as-falsy semantics of date predicates, and the AND composition of
//! all filters.

use super::*;
use crate::db::DbPool;
use crate::dto::{GetQueryDto, GetQueryDtoBuilder, SuspendedFilter};
use crate::models::{Card, CardId, Item, ItemId, ItemTypeId, Review, TagId};
use crate::repo::tests::setup_test_db;
use crate::repo::{
	add_tag_to_item, create_item, create_item_relation, create_item_type, create_tag,
	get_cards_for_item, record_review, update_card,
};
use crate::test_utils::arb_datetime_utc;
use chrono::{DateTime, Utc};
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// World construction
// ---------------------------------------------------------------------------

/// In-memory mirror of the DB state, used by oracles.
#[derive(Debug, Clone)]
struct TestWorld {
	item_types: Vec<ItemTypeId>,
	items: Vec<Item>,
	cards: Vec<Card>,
	tags: Vec<TagId>,
	/// Per-item set of tag ids (sorted).
	item_tags: HashMap<ItemId, Vec<TagId>>,
	/// (parent_item_id, child_item_id) pairs.
	relations: Vec<(ItemId, ItemId)>,
	#[allow(dead_code)] // reviews are only inspected by reviews_matching tests
	reviews: Vec<Review>,
}

impl TestWorld {
	fn item_type_of(&self, item_id: &ItemId) -> Option<ItemTypeId> {
		self.items
			.iter()
			.find(|i| &i.get_id() == item_id)
			.map(|i| i.get_item_type())
	}

	fn cards_of_item(&self, item_id: &ItemId) -> Vec<&Card> {
		self.cards
			.iter()
			.filter(|c| &c.get_item_id() == item_id)
			.collect()
	}

	fn children_of(&self, parent: &ItemId) -> HashSet<ItemId> {
		self.relations
			.iter()
			.filter(|(p, _)| p == parent)
			.map(|(_, c)| c.clone())
			.collect()
	}

	fn parents_of(&self, child: &ItemId) -> HashSet<ItemId> {
		self.relations
			.iter()
			.filter(|(_, c)| c == child)
			.map(|(p, _)| p.clone())
			.collect()
	}
}

/// Bare bones build-a-world helper. Always creates the same kinds of things
/// — tests vary only what they mutate / query against.
async fn build_basic_world(pool: &DbPool, n_item_types: usize, items_per_type: usize) -> TestWorld {
	let mut item_types = Vec::new();
	let mut items = Vec::new();
	let mut cards = Vec::new();

	for ti in 0..n_item_types {
		let name = format!("Test Type {}", ti);
		let it = create_item_type(pool, name, "fsrs".to_owned())
			.await
			.unwrap();
		item_types.push(it.get_id());

		for ii in 0..items_per_type {
			let title = format!("title-t{}-i{}", ti, ii);
			let item = create_item(pool, &it.get_id(), title, serde_json::json!({}))
				.await
				.unwrap();
			let item_cards = get_cards_for_item(pool, &item.get_id()).unwrap();
			cards.extend(item_cards.iter().cloned());
			items.push(item);
		}
	}

	TestWorld {
		item_types,
		items,
		cards,
		tags: Vec::new(),
		item_tags: HashMap::new(),
		relations: Vec::new(),
		reviews: Vec::new(),
	}
}

/// Adds `n_tags` tags to the world and returns the (mutated) world.
async fn add_tags(pool: &DbPool, world: &mut TestWorld, n_tags: usize) {
	for ti in 0..n_tags {
		let name = format!("tag-{}", ti);
		let tag = create_tag(pool, name, true).await.unwrap();
		world.tags.push(tag.get_id());
	}
}

/// For each (item_index, tag_index) pair, attach the tag to the item.
async fn attach_tags(pool: &DbPool, world: &mut TestWorld, pairs: &[(usize, usize)]) {
	for (ii, ti) in pairs {
		let item_id = world.items[*ii].get_id();
		let tag_id = world.tags[*ti].clone();
		// add_tag_to_item is idempotent at the SQL level (PRIMARY KEY on
		// (item_id, tag_id)) — but we only attach unique pairs in tests.
		let already = world
			.item_tags
			.get(&item_id)
			.map(|ts| ts.contains(&tag_id))
			.unwrap_or(false);
		if !already {
			add_tag_to_item(pool, &tag_id, &item_id).await.unwrap();
			world.item_tags.entry(item_id).or_default().push(tag_id);
		}
	}
}

/// For each (parent_index, child_index) pair, create a relation.
///
/// Pairs that would create a cycle (a relation in the opposite direction
/// already exists, transitively) are silently skipped — the
/// `create_item_relation` repo refuses cycles, so this just keeps test setup
/// from panicking on arbitrary input.
async fn add_relations(pool: &DbPool, world: &mut TestWorld, pairs: &[(usize, usize)]) {
	for (pi, ci) in pairs {
		if pi == ci {
			continue;
		}
		let parent_id = world.items[*pi].get_id();
		let child_id = world.items[*ci].get_id();
		if world
			.relations
			.iter()
			.any(|(p, c)| p == &parent_id && c == &child_id)
		{
			continue;
		}
		match create_item_relation(pool, &parent_id, &child_id, "extract").await {
			Ok(_) => {
				world.relations.push((parent_id, child_id));
			}
			Err(e) if e.to_string().contains("cycle") => {
				// expected for arbitrary inputs; silently skip.
			}
			Err(e) => panic!("unexpected create_item_relation error: {}", e),
		}
	}
}

// ---------------------------------------------------------------------------
// Oracles
// ---------------------------------------------------------------------------

/// Pure-Rust oracle for `cards_matching`.
fn oracle_cards_matching(world: &TestWorld, query: &GetQueryDto) -> HashSet<CardId> {
	world
		.cards
		.iter()
		.filter(|c| card_matches_query(c, world, query))
		.map(|c| c.get_id())
		.collect()
}

/// Pure-Rust oracle for `items_matching`.
///
/// An item is included iff the item-level filters all pass AND, if any
/// card-level filter is set, at least one of the item's cards matches the
/// full query.
fn oracle_items_matching(world: &TestWorld, query: &GetQueryDto) -> HashSet<ItemId> {
	world
		.items
		.iter()
		.filter(|item| item_matches_query(item, world, query))
		.map(|i| i.get_id())
		.collect()
}

/// Pure-Rust oracle for `reviews_matching`.
fn oracle_reviews_matching(
	world: &TestWorld,
	query: &GetQueryDto,
) -> HashSet<crate::models::ReviewId> {
	let matching_card_ids = oracle_cards_matching(world, query);
	world
		.reviews
		.iter()
		.filter(|r| matching_card_ids.contains(&r.get_card_id()))
		.map(|r| r.get_id())
		.collect()
}

fn has_card_level_filter(query: &GetQueryDto) -> bool {
	query.next_review_before.is_some()
		|| query.last_review_after.is_some()
		|| query.suspended_after.is_some()
		|| query.suspended_before.is_some()
		|| query.suspended_filter != SuspendedFilter::default()
}

fn card_matches_query(card: &Card, world: &TestWorld, query: &GetQueryDto) -> bool {
	let item_id = card.get_item_id();

	// All item-level predicates first
	if !item_id_matches_item_filters(&item_id, world, query) {
		return false;
	}

	// Card-level
	if let Some(cutoff) = query.next_review_before {
		if !(card.get_next_review() < cutoff) {
			return false;
		}
	}
	if let Some(cutoff) = query.last_review_after {
		match card.get_last_review() {
			Some(lr) if lr > cutoff => {}
			_ => return false,
		}
	}
	match query.suspended_filter {
		SuspendedFilter::Include => {}
		SuspendedFilter::Exclude => {
			if card.get_suspended().is_some() {
				return false;
			}
		}
		SuspendedFilter::Only => {
			if card.get_suspended().is_none() {
				return false;
			}
		}
	}
	if let Some(cutoff) = query.suspended_after {
		match card.get_suspended() {
			Some(s) if s > cutoff => {}
			_ => return false,
		}
	}
	if let Some(cutoff) = query.suspended_before {
		match card.get_suspended() {
			Some(s) if s < cutoff => {}
			_ => return false,
		}
	}
	true
}

fn item_id_matches_item_filters(item_id: &ItemId, world: &TestWorld, query: &GetQueryDto) -> bool {
	if let Some(ref type_id) = query.item_type_id {
		if world.item_type_of(item_id).as_ref() != Some(type_id) {
			return false;
		}
	}
	if !query.tag_ids.is_empty() {
		let item_tags = world.item_tags.get(item_id).cloned().unwrap_or_default();
		if !query.tag_ids.iter().all(|t| item_tags.contains(t)) {
			return false;
		}
	}
	if let Some(ref parent_id) = query.parent_item_id {
		if !world.children_of(parent_id).contains(item_id) {
			return false;
		}
	}
	if let Some(ref child_id) = query.child_item_id {
		if !world.parents_of(child_id).contains(item_id) {
			return false;
		}
	}
	true
}

fn item_matches_query(item: &Item, world: &TestWorld, query: &GetQueryDto) -> bool {
	let item_id = item.get_id();
	if !item_id_matches_item_filters(&item_id, world, query) {
		return false;
	}
	if has_card_level_filter(query) {
		// Item must own ≥1 card that matches the full query.
		world
			.cards_of_item(&item_id)
			.iter()
			.any(|c| card_matches_query(c, world, query))
	} else {
		true
	}
}

// ---------------------------------------------------------------------------
// Helpers for executing the SQL subqueries
// ---------------------------------------------------------------------------

fn sql_cards_matching(pool: &DbPool, query: &GetQueryDto) -> HashSet<CardId> {
	let conn = &mut pool.get().unwrap();
	cards_matching(query)
		.load::<CardId>(conn)
		.unwrap()
		.into_iter()
		.collect()
}

fn sql_items_matching(pool: &DbPool, query: &GetQueryDto) -> HashSet<ItemId> {
	let conn = &mut pool.get().unwrap();
	items_matching(query)
		.load::<ItemId>(conn)
		.unwrap()
		.into_iter()
		.collect()
}

fn sql_reviews_matching(pool: &DbPool, query: &GetQueryDto) -> HashSet<crate::models::ReviewId> {
	let conn = &mut pool.get().unwrap();
	reviews_matching(query)
		.load::<crate::models::ReviewId>(conn)
		.unwrap()
		.into_iter()
		.collect()
}

// ===========================================================================
// Q0: Empty / default query
// ===========================================================================

proptest! {
	/// Q0.1: A default (all-fields-default) `GetQueryDto` selects every card.
	#[test]
	fn prop_q0_1_empty_query_selects_all_cards(
		n_types in 1usize..=3,
		items_per_type in 1usize..=3,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let world = build_basic_world(&pool, n_types, items_per_type).await;
			let query = GetQueryDto::default();

			let sql = sql_cards_matching(&pool, &query);
			let oracle = oracle_cards_matching(&world, &query);

			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q0.2: A default `GetQueryDto` selects every item.
	#[test]
	fn prop_q0_2_empty_query_selects_all_items(
		n_types in 1usize..=3,
		items_per_type in 1usize..=3,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let world = build_basic_world(&pool, n_types, items_per_type).await;
			let query = GetQueryDto::default();

			let sql = sql_items_matching(&pool, &query);
			let oracle = oracle_items_matching(&world, &query);

			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q0.3: A default `GetQueryDto` selects every review. Mirror of Q0.1/Q0.2
	/// for the third matching function.
	#[test]
	fn prop_q0_3_empty_query_selects_all_reviews(
		n_items in 1usize..=3,
		ratings in prop::collection::vec((0usize..6, 1i32..=4), 1..6),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_reviews(&pool, &mut world, &ratings).await;
			let query = GetQueryDto::default();

			let sql = sql_reviews_matching(&pool, &query);
			let oracle = oracle_reviews_matching(&world, &query);

			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q1: item_type_id filter
// ===========================================================================

proptest! {
	/// Q1.1: Filtering cards by `item_type_id` keeps only cards whose item is
	/// of that type. Indexing into the type list keeps the proptest input
	/// small while still exploring "every type" because n_types is varied.
	#[test]
	fn prop_q1_1_cards_by_item_type(
		n_types in 1usize..=4,
		items_per_type in 1usize..=3,
		type_ix in 0usize..4,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let world = build_basic_world(&pool, n_types, items_per_type).await;

			let target_type = world.item_types[type_ix % n_types].clone();
			let query = GetQueryDtoBuilder::new()
				.item_type_id(target_type)
				.build();

			let sql = sql_cards_matching(&pool, &query);
			let oracle = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q1.2: Same property for items.
	#[test]
	fn prop_q1_2_items_by_item_type(
		n_types in 1usize..=4,
		items_per_type in 1usize..=3,
		type_ix in 0usize..4,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let world = build_basic_world(&pool, n_types, items_per_type).await;

			let target_type = world.item_types[type_ix % n_types].clone();
			let query = GetQueryDtoBuilder::new()
				.item_type_id(target_type)
				.build();

			let sql = sql_items_matching(&pool, &query);
			let oracle = oracle_items_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q1.3: An `item_type_id` that doesn't exist returns the empty set.
	#[test]
	fn prop_q1_3_unknown_item_type_returns_empty(garbage in "\\PC*") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let _world = build_basic_world(&pool, 2, 2).await;

			let query = GetQueryDtoBuilder::new()
				.item_type_id(ItemTypeId(format!("not-real-{}", garbage)))
				.build();

			let sql_c = sql_cards_matching(&pool, &query);
			let sql_i = sql_items_matching(&pool, &query);
			prop_assert!(sql_c.is_empty());
			prop_assert!(sql_i.is_empty());
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q2: tag_ids filter (the SQL change this PR is really about)
// ===========================================================================

proptest! {
	/// Q2.1: A single tag filter selects items (and their cards) tagged with
	/// that tag, exactly. Every item-tag association is generated from
	/// `attach_pairs` and mirrored into the oracle.
	#[test]
	fn prop_q2_1_single_tag(
		n_items in 2usize..=5,
		n_tags in 1usize..=3,
		attach_pairs in prop::collection::vec((0usize..5, 0usize..3), 0..10),
		query_tag_ix in 0usize..3,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_tags(&pool, &mut world, n_tags).await;
			let pairs: Vec<(usize, usize)> = attach_pairs
				.into_iter()
				.map(|(i, t)| (i % n_items, t % n_tags))
				.collect();
			attach_tags(&pool, &mut world, &pairs).await;

			let target_tag = world.tags[query_tag_ix % n_tags].clone();
			let query = GetQueryDtoBuilder::new().add_tag_id(target_tag).build();

			let sql_c = sql_cards_matching(&pool, &query);
			let oracle_c = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql_c, oracle_c);

			let sql_i = sql_items_matching(&pool, &query);
			let oracle_i = oracle_items_matching(&world, &query);
			prop_assert_eq!(sql_i, oracle_i);
			Ok(())
		})?;
	}

	/// Q2.2: Multiple tag ids compose with AND — an item is included iff it
	/// has *every* requested tag (not just any). This is the property the
	/// SQL `GROUP BY ... HAVING COUNT(DISTINCT tag_id) = N` encoding has to
	/// preserve.
	#[test]
	fn prop_q2_2_multiple_tags_and_semantics(
		n_items in 2usize..=5,
		n_tags in 2usize..=6,
		attach_pairs in prop::collection::vec((0usize..5, 0usize..6), 0..25),
		query_tag_ixs in prop::collection::vec(0usize..6, 1..=5),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_tags(&pool, &mut world, n_tags).await;
			let pairs: Vec<(usize, usize)> = attach_pairs
				.into_iter()
				.map(|(i, t)| (i % n_items, t % n_tags))
				.collect();
			attach_tags(&pool, &mut world, &pairs).await;

			let mut query_tags: Vec<TagId> = query_tag_ixs
				.into_iter()
				.map(|ix| world.tags[ix % n_tags].clone())
				.collect();
			query_tags.sort();
			query_tags.dedup();

			let query = GetQueryDtoBuilder::new().tag_ids(query_tags).build();

			let sql_c = sql_cards_matching(&pool, &query);
			let oracle_c = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql_c, oracle_c);

			let sql_i = sql_items_matching(&pool, &query);
			let oracle_i = oracle_items_matching(&world, &query);
			prop_assert_eq!(sql_i, oracle_i);
			Ok(())
		})?;
	}

	/// Q2.3: An empty `tag_ids` vec is a no-op — items without any tags must
	/// still be selected. (Easy to get wrong if tag filter mistakenly always
	/// joins through `item_tags`.)
	#[test]
	fn prop_q2_3_empty_tag_ids_is_no_op(
		n_items in 2usize..=5,
		n_tags in 1usize..=3,
		attach_pairs in prop::collection::vec((0usize..5, 0usize..3), 0..6),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_tags(&pool, &mut world, n_tags).await;
			let pairs: Vec<(usize, usize)> = attach_pairs
				.into_iter()
				.map(|(i, t)| (i % n_items, t % n_tags))
				.collect();
			attach_tags(&pool, &mut world, &pairs).await;

			let query = GetQueryDtoBuilder::new().tag_ids(Vec::new()).build();

			let sql_c = sql_cards_matching(&pool, &query);
			let oracle_c = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql_c, oracle_c);

			let sql_i = sql_items_matching(&pool, &query);
			let oracle_i = oracle_items_matching(&world, &query);
			prop_assert_eq!(sql_i, oracle_i);
			Ok(())
		})?;
	}

	/// Q2.4: A tag id that exists but is not attached to any item selects nothing.
	#[test]
	fn prop_q2_4_unattached_tag_selects_nothing(n_items in 1usize..=4) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_tags(&pool, &mut world, 1).await;
			// no attach_tags call

			let query = GetQueryDtoBuilder::new()
				.add_tag_id(world.tags[0].clone())
				.build();

			let sql_c = sql_cards_matching(&pool, &query);
			prop_assert!(sql_c.is_empty());
			let sql_i = sql_items_matching(&pool, &query);
			prop_assert!(sql_i.is_empty());
			let sql_r = sql_reviews_matching(&pool, &query);
			prop_assert!(sql_r.is_empty());
			Ok(())
		})?;
	}

	/// Q2.5: A non-existent tag id selects nothing (and produces no SQL error).
	#[test]
	fn prop_q2_5_unknown_tag_selects_nothing(garbage in "\\PC*") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let _world = build_basic_world(&pool, 1, 2).await;

			let query = GetQueryDtoBuilder::new()
				.add_tag_id(TagId(format!("not-a-tag-{}", garbage)))
				.build();

			let sql_c = sql_cards_matching(&pool, &query);
			prop_assert!(sql_c.is_empty());
			let sql_i = sql_items_matching(&pool, &query);
			prop_assert!(sql_i.is_empty());
			let sql_r = sql_reviews_matching(&pool, &query);
			prop_assert!(sql_r.is_empty());
			Ok(())
		})?;
	}

	/// Q2.6: Duplicated tag ids in the input collapse — a query for
	/// `[tagA, tagA]` selects exactly the same items as `[tagA]`. This pins
	/// the SQL `COUNT(DISTINCT tag_id)` semantics: a naive `COUNT(*) = N`
	/// would over-count when N includes duplicates and silently drop items
	/// that legitimately have the tag once.
	#[test]
	fn prop_q2_6_duplicate_tag_ids_collapse(
		n_items in 2usize..=4,
		n_tags in 1usize..=3,
		attach_pairs in prop::collection::vec((0usize..4, 0usize..3), 0..8),
		query_tag_ix in 0usize..3,
		dup_count in 1usize..=4,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_tags(&pool, &mut world, n_tags).await;
			let pairs: Vec<(usize, usize)> = attach_pairs
				.into_iter()
				.map(|(i, t)| (i % n_items, t % n_tags))
				.collect();
			attach_tags(&pool, &mut world, &pairs).await;

			let target = world.tags[query_tag_ix % n_tags].clone();
			let single = GetQueryDtoBuilder::new()
				.tag_ids(vec![target.clone()])
				.build();
			let dupes = GetQueryDtoBuilder::new()
				.tag_ids(vec![target.clone(); dup_count])
				.build();

			let sql_single_c = sql_cards_matching(&pool, &single);
			let sql_dupes_c = sql_cards_matching(&pool, &dupes);
			prop_assert_eq!(sql_single_c, sql_dupes_c);

			let sql_single_i = sql_items_matching(&pool, &single);
			let sql_dupes_i = sql_items_matching(&pool, &dupes);
			prop_assert_eq!(sql_single_i, sql_dupes_i);
			Ok(())
		})?;
	}

	/// Q2.7: AND-ing a known (possibly attached) tag with a nonexistent tag
	/// still selects nothing — the bogus tag can never be satisfied, and AND
	/// semantics require *every* requested tag to be present. Q2.2 only draws
	/// query tags from the set of existing tags and Q2.5 covers the
	/// unknown-alone case; this pins the mixed "known ∧ bogus ⇒ empty" edge,
	/// which a naive `IN (...)` predicate would get wrong by treating the
	/// list as disjunctive.
	#[test]
	fn prop_q2_7_known_tag_and_unknown_tag_selects_nothing(
		n_items in 2usize..=4,
		n_tags in 1usize..=3,
		attach_pairs in prop::collection::vec((0usize..4, 0usize..3), 0..8),
		known_tag_ix in 0usize..3,
		garbage in "\\PC*",
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_tags(&pool, &mut world, n_tags).await;
			let pairs: Vec<(usize, usize)> = attach_pairs
				.into_iter()
				.map(|(i, t)| (i % n_items, t % n_tags))
				.collect();
			attach_tags(&pool, &mut world, &pairs).await;

			let known = world.tags[known_tag_ix % n_tags].clone();
			let bogus = TagId(format!("not-a-tag-{}", garbage));
			let query = GetQueryDtoBuilder::new()
				.tag_ids(vec![known, bogus])
				.build();

			let sql_c = sql_cards_matching(&pool, &query);
			let oracle_c = oracle_cards_matching(&world, &query);
			prop_assert_eq!(&sql_c, &oracle_c);
			prop_assert!(sql_c.is_empty(), "AND with a bogus tag must select nothing");

			let sql_i = sql_items_matching(&pool, &query);
			let oracle_i = oracle_items_matching(&world, &query);
			prop_assert_eq!(&sql_i, &oracle_i);
			prop_assert!(sql_i.is_empty());

			let sql_r = sql_reviews_matching(&pool, &query);
			prop_assert!(sql_r.is_empty());
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q3: date filters with NULL semantics
// ===========================================================================

/// Helper that mutates each card's review/suspended fields from arbitrary input.
async fn mutate_cards_for_dates(
	pool: &DbPool,
	world: &mut TestWorld,
	mutations: &[(
		Option<DateTime<Utc>>,
		Option<DateTime<Utc>>,
		Option<DateTime<Utc>>,
	)],
) {
	for (i, card) in world.cards.iter_mut().enumerate() {
		let (next_opt, last_opt, susp_opt) = mutations[i % mutations.len()];
		if let Some(nr) = next_opt {
			card.set_next_review(nr);
		}
		card.set_last_review(last_opt);
		card.set_suspended(susp_opt);
		update_card(pool, card).await.unwrap();
	}
}

proptest! {
	/// Q3.1: `next_review_before` keeps cards whose `next_review < cutoff`.
	#[test]
	fn prop_q3_1_next_review_before(
		n_items in 2usize..=4,
		mutations in prop::collection::vec(
			(arb_datetime_utc().prop_map(Some), Just(None), Just(None)),
			1..6
		),
		cutoff in arb_datetime_utc(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			mutate_cards_for_dates(&pool, &mut world, &mutations).await;

			let query = GetQueryDtoBuilder::new().next_review_before(cutoff).build();

			let sql = sql_cards_matching(&pool, &query);
			let oracle = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q3.2: `last_review_after` keeps cards whose `last_review` is non-NULL
	/// AND > cutoff. NULL `last_review` must NEVER pass.
	#[test]
	fn prop_q3_2_last_review_after_null_falsy(
		n_items in 2usize..=4,
		mutations in prop::collection::vec(
			(Just(None), prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)], Just(None)),
			1..6
		),
		cutoff in arb_datetime_utc(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			mutate_cards_for_dates(&pool, &mut world, &mutations).await;

			let query = GetQueryDtoBuilder::new().last_review_after(cutoff).build();

			let sql = sql_cards_matching(&pool, &query);
			let oracle = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q4: suspended filter (enum + suspended_after / suspended_before)
// ===========================================================================

proptest! {
	/// Q4.1: `suspended_filter::Exclude` keeps non-suspended cards only.
	#[test]
	fn prop_q4_1_suspended_exclude(
		n_items in 2usize..=4,
		susp_pattern in prop::collection::vec(
			prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
			1..6
		),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let muts: Vec<_> = susp_pattern.iter().map(|s| (None, None, *s)).collect();
			mutate_cards_for_dates(&pool, &mut world, &muts).await;

			let query = GetQueryDtoBuilder::new()
				.suspended_filter(SuspendedFilter::Exclude)
				.build();

			let sql = sql_cards_matching(&pool, &query);
			let oracle = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q4.2: `suspended_filter::Only` keeps suspended cards only.
	#[test]
	fn prop_q4_2_suspended_only(
		n_items in 2usize..=4,
		susp_pattern in prop::collection::vec(
			prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
			1..6
		),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let muts: Vec<_> = susp_pattern.iter().map(|s| (None, None, *s)).collect();
			mutate_cards_for_dates(&pool, &mut world, &muts).await;

			let query = GetQueryDtoBuilder::new()
				.suspended_filter(SuspendedFilter::Only)
				.build();

			let sql = sql_cards_matching(&pool, &query);
			let oracle = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q4.3: `suspended_filter::Include` does not narrow — every card in the
	/// world is returned regardless of its suspended state. (The default is
	/// `Exclude`, so "no-op relative to default" is the wrong framing;
	/// `Include` is a deliberate opt-in to the broadest result set.)
	#[test]
	fn prop_q4_3_suspended_include_does_not_narrow(
		n_items in 2usize..=4,
		susp_pattern in prop::collection::vec(
			prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
			1..6
		),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let muts: Vec<_> = susp_pattern.iter().map(|s| (None, None, *s)).collect();
			mutate_cards_for_dates(&pool, &mut world, &muts).await;

			let with = GetQueryDtoBuilder::new()
				.suspended_filter(SuspendedFilter::Include)
				.build();

			let sql = sql_cards_matching(&pool, &with);
			let all: HashSet<CardId> = world.cards.iter().map(|c| c.get_id()).collect();
			prop_assert_eq!(sql, all);
			Ok(())
		})?;
	}

	/// Q4.4: `suspended_after` requires non-NULL `suspended > cutoff`. NULL
	/// suspended must NEVER pass — same NULL-falsy convention as
	/// `last_review_after`.
	#[test]
	fn prop_q4_4_suspended_after_null_falsy(
		n_items in 2usize..=4,
		susp_pattern in prop::collection::vec(
			prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
			1..6
		),
		cutoff in arb_datetime_utc(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let muts: Vec<_> = susp_pattern.iter().map(|s| (None, None, *s)).collect();
			mutate_cards_for_dates(&pool, &mut world, &muts).await;

			let query = GetQueryDtoBuilder::new()
				.suspended_filter(SuspendedFilter::Include)
				.suspended_after(cutoff)
				.build();

			let sql = sql_cards_matching(&pool, &query);
			let oracle = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q4.5: `suspended_before` requires non-NULL `suspended < cutoff`.
	/// Same NULL-falsy convention.
	#[test]
	fn prop_q4_5_suspended_before_null_falsy(
		n_items in 2usize..=4,
		susp_pattern in prop::collection::vec(
			prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
			1..6
		),
		cutoff in arb_datetime_utc(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let muts: Vec<_> = susp_pattern.iter().map(|s| (None, None, *s)).collect();
			mutate_cards_for_dates(&pool, &mut world, &muts).await;

			let query = GetQueryDtoBuilder::new()
				.suspended_filter(SuspendedFilter::Include)
				.suspended_before(cutoff)
				.build();

			let sql = sql_cards_matching(&pool, &query);
			let oracle = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q5: relation filters (parent_item_id / child_item_id)
// ===========================================================================

proptest! {
	/// Q5.1: `parent_item_id` selects items that are children of the given parent.
	#[test]
	fn prop_q5_1_parent_item_id(
		n_items in 2usize..=5,
		relations in prop::collection::vec((0usize..5, 0usize..5), 0..10),
		parent_ix in 0usize..5,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let pairs: Vec<(usize, usize)> = relations
				.into_iter()
				.map(|(p, c)| (p % n_items, c % n_items))
				.collect();
			add_relations(&pool, &mut world, &pairs).await;

			let parent_id = world.items[parent_ix % n_items].get_id();
			let query = GetQueryDtoBuilder::new().parent_item_id(parent_id).build();

			let sql_c = sql_cards_matching(&pool, &query);
			let oracle_c = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql_c, oracle_c);

			let sql_i = sql_items_matching(&pool, &query);
			let oracle_i = oracle_items_matching(&world, &query);
			prop_assert_eq!(sql_i, oracle_i);
			Ok(())
		})?;
	}

	/// Q5.2: `child_item_id` selects items that are parents of the given child.
	#[test]
	fn prop_q5_2_child_item_id(
		n_items in 2usize..=5,
		relations in prop::collection::vec((0usize..5, 0usize..5), 0..10),
		child_ix in 0usize..5,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let pairs: Vec<(usize, usize)> = relations
				.into_iter()
				.map(|(p, c)| (p % n_items, c % n_items))
				.collect();
			add_relations(&pool, &mut world, &pairs).await;

			let child_id = world.items[child_ix % n_items].get_id();
			let query = GetQueryDtoBuilder::new().child_item_id(child_id).build();

			let sql_c = sql_cards_matching(&pool, &query);
			let oracle_c = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql_c, oracle_c);

			let sql_i = sql_items_matching(&pool, &query);
			let oracle_i = oracle_items_matching(&world, &query);
			prop_assert_eq!(sql_i, oracle_i);
			Ok(())
		})?;
	}

	/// Q5.3: Both `parent_item_id` and `child_item_id` set — the result is
	/// the intersection of "children of parent" and "parents of child".
	/// This pins that the two relation predicates AND, not OR.
	#[test]
	fn prop_q5_3_parent_and_child_intersection(
		n_items in 3usize..=6,
		relations in prop::collection::vec((0usize..6, 0usize..6), 0..12),
		parent_ix in 0usize..6,
		child_ix in 0usize..6,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let pairs: Vec<(usize, usize)> = relations
				.into_iter()
				.map(|(p, c)| (p % n_items, c % n_items))
				.collect();
			add_relations(&pool, &mut world, &pairs).await;

			let parent_id = world.items[parent_ix % n_items].get_id();
			let child_id = world.items[child_ix % n_items].get_id();
			let query = GetQueryDtoBuilder::new()
				.parent_item_id(parent_id)
				.child_item_id(child_id)
				.build();

			let sql_i = sql_items_matching(&pool, &query);
			let oracle_i = oracle_items_matching(&world, &query);
			prop_assert_eq!(sql_i, oracle_i);
			Ok(())
		})?;
	}

	/// Q5.4: A non-existent parent_item_id selects nothing.
	#[test]
	fn prop_q5_4_unknown_parent_selects_nothing(garbage in "\\PC*") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let _world = build_basic_world(&pool, 1, 2).await;

			let query = GetQueryDtoBuilder::new()
				.parent_item_id(ItemId(format!("not-real-{}", garbage)))
				.build();
			let sql_c = sql_cards_matching(&pool, &query);
			prop_assert!(sql_c.is_empty());
			let sql_i = sql_items_matching(&pool, &query);
			prop_assert!(sql_i.is_empty());
			let sql_r = sql_reviews_matching(&pool, &query);
			prop_assert!(sql_r.is_empty());
			Ok(())
		})?;
	}

	/// Q5.5: Mirror of Q5.4 — a non-existent child_item_id selects nothing.
	#[test]
	fn prop_q5_5_unknown_child_selects_nothing(garbage in "\\PC*") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let _world = build_basic_world(&pool, 1, 2).await;

			let query = GetQueryDtoBuilder::new()
				.child_item_id(ItemId(format!("not-real-{}", garbage)))
				.build();
			let sql_c = sql_cards_matching(&pool, &query);
			prop_assert!(sql_c.is_empty());
			let sql_i = sql_items_matching(&pool, &query);
			prop_assert!(sql_i.is_empty());
			let sql_r = sql_reviews_matching(&pool, &query);
			prop_assert!(sql_r.is_empty());
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q6: composition — multiple filters AND together
// ===========================================================================

proptest! {
	// Q6.1 does a lot of DB work per case; lower cases for the whole block.
	#![proptest_config(ProptestConfig::with_cases(64))]

	/// Q6.1: All filters compose via AND. We construct a worst-case query
	/// (item_type + tags + every date + suspended + relation) and assert
	/// the SQL result equals the oracle (which ANDs everything) for cards,
	/// items, AND reviews — so dropping any predicate from any of the three
	/// matching functions causes a failure here.
	#[test]
	fn prop_q6_1_full_composition(
		n_types in 1usize..=2,
		items_per_type in 2usize..=3,
		n_tags in 1usize..=3,
		attach_pairs in prop::collection::vec((0usize..6, 0usize..3), 0..12),
		relation_pairs in prop::collection::vec((0usize..6, 0usize..6), 0..6),
		date_muts in prop::collection::vec(
			(
				prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
				prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
				prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
			),
			1..6
		),
		ratings in prop::collection::vec((0usize..6, 1i32..=4), 0..6),
		type_ix in 0usize..2,
		query_tag_ixs in prop::collection::vec(0usize..3, 0..=2),
		next_cutoff in prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
		last_after in prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
		susp_after in prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
		susp_before in prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
		parent_ix in prop_oneof![Just(None), (0usize..6).prop_map(Some)],
		child_ix in prop_oneof![Just(None), (0usize..6).prop_map(Some)],
		susp_filter in prop_oneof![
			Just(SuspendedFilter::Include),
			Just(SuspendedFilter::Exclude),
			Just(SuspendedFilter::Only),
		],
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, n_types, items_per_type).await;
			add_tags(&pool, &mut world, n_tags).await;
			let n_items = world.items.len();

			let pairs: Vec<(usize, usize)> = attach_pairs
				.into_iter()
				.map(|(i, t)| (i % n_items, t % n_tags))
				.collect();
			attach_tags(&pool, &mut world, &pairs).await;

			let rels: Vec<(usize, usize)> = relation_pairs
				.into_iter()
				.map(|(p, c)| (p % n_items, c % n_items))
				.collect();
			add_relations(&pool, &mut world, &rels).await;

			mutate_cards_for_dates(&pool, &mut world, &date_muts).await;
			add_reviews(&pool, &mut world, &ratings).await;

			let mut query = GetQueryDtoBuilder::new()
				.item_type_id(world.item_types[type_ix % n_types].clone())
				.suspended_filter(susp_filter);
			if !query_tag_ixs.is_empty() {
				let tags: Vec<TagId> = query_tag_ixs
					.into_iter()
					.map(|ix| world.tags[ix % n_tags].clone())
					.collect();
				query = query.tag_ids(tags);
			}
			if let Some(c) = next_cutoff {
				query = query.next_review_before(c);
			}
			if let Some(c) = last_after {
				query = query.last_review_after(c);
			}
			if let Some(c) = susp_after {
				query = query.suspended_after(c);
			}
			if let Some(c) = susp_before {
				query = query.suspended_before(c);
			}
			if let Some(ix) = parent_ix {
				query = query.parent_item_id(world.items[ix % n_items].get_id());
			}
			if let Some(ix) = child_ix {
				query = query.child_item_id(world.items[ix % n_items].get_id());
			}
			let query = query.build();

			let sql_c = sql_cards_matching(&pool, &query);
			let oracle_c = oracle_cards_matching(&world, &query);
			prop_assert_eq!(sql_c, oracle_c);

			let sql_i = sql_items_matching(&pool, &query);
			let oracle_i = oracle_items_matching(&world, &query);
			prop_assert_eq!(sql_i, oracle_i);

			let sql_r = sql_reviews_matching(&pool, &query);
			let oracle_r = oracle_reviews_matching(&world, &query);
			prop_assert_eq!(sql_r, oracle_r);
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q7: items_matching specifics
// ===========================================================================

proptest! {
	/// Q7.1: With *no* card-level filter, an item matching item-level filters
	/// is included even if its cards would not match a hypothetical
	/// card-level filter — i.e. items_matching does NOT silently fold in
	/// card existence.
	#[test]
	fn prop_q7_1_no_card_filter_does_not_require_matching_card(
		n_items in 1usize..=3,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let world = build_basic_world(&pool, 1, n_items).await;

			let query = GetQueryDtoBuilder::new()
				.item_type_id(world.item_types[0].clone())
				.build();

			let sql = sql_items_matching(&pool, &query);
			let oracle = oracle_items_matching(&world, &query);
			let sql_len = sql.len();
			prop_assert_eq!(sql, oracle);
			// Sanity: every item is returned (one type, no other filters).
			prop_assert_eq!(sql_len, world.items.len());
			Ok(())
		})?;
	}

	/// Q7.2: With a card-level filter set, an item is included iff at least
	/// one of its cards matches.
	#[test]
	fn prop_q7_2_card_filter_requires_matching_card(
		n_items in 1usize..=4,
		date_muts in prop::collection::vec(
			(
				arb_datetime_utc().prop_map(Some),
				Just(None),
				Just(None),
			),
			1..6
		),
		cutoff in arb_datetime_utc(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			mutate_cards_for_dates(&pool, &mut world, &date_muts).await;

			let query = GetQueryDtoBuilder::new().next_review_before(cutoff).build();

			let sql = sql_items_matching(&pool, &query);
			let oracle = oracle_items_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q7.3: Every card-level filter (`last_review_after`, `suspended_filter`,
	/// `suspended_after`, `suspended_before`) — not just `next_review_before` —
	/// must propagate through `items_matching`. We push them through one at a
	/// time and assert items_matching == oracle. Captures the family of bugs
	/// where one card-level predicate is folded into items but another silently
	/// is not.
	#[test]
	fn prop_q7_3_items_matching_with_each_card_level_filter(
		n_items in 1usize..=4,
		date_muts in prop::collection::vec(
			(
				prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
				prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
				prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
			),
			1..6
		),
		cutoff in arb_datetime_utc(),
		susp_filter in prop_oneof![
			Just(SuspendedFilter::Exclude),
			Just(SuspendedFilter::Only),
		],
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			mutate_cards_for_dates(&pool, &mut world, &date_muts).await;

			for query in [
				GetQueryDtoBuilder::new().last_review_after(cutoff).build(),
				GetQueryDtoBuilder::new().suspended_filter(susp_filter).build(),
				GetQueryDtoBuilder::new()
					.suspended_filter(SuspendedFilter::Include)
					.suspended_after(cutoff)
					.build(),
				GetQueryDtoBuilder::new()
					.suspended_filter(SuspendedFilter::Include)
					.suspended_before(cutoff)
					.build(),
			] {
				let sql = sql_items_matching(&pool, &query);
				let oracle = oracle_items_matching(&world, &query);
				prop_assert_eq!(sql, oracle);
			}
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q7B: items_matching when an item has zero cards
// ===========================================================================
//
// `DESIRED_BEHAVIOUR.md` explicitly carves out: when any card-level filter is
// set, an item with **zero cards** must be excluded — there's no card to
// match. The world built by `build_basic_world` always populates ≥1 card per
// item via `create_item`, so we use `delete_cards_for_item` to manufacture
// the zero-card edge case.

/// Test helper: drop every card belonging to an item, both in the DB and in
/// the in-memory `TestWorld`. Used to construct the "item with zero cards"
/// edge case.
async fn delete_cards_for_item(pool: &DbPool, world: &mut TestWorld, item_ix: usize) {
	use crate::schema::cards;
	let item_id = world.items[item_ix].get_id();
	let conn = &mut pool.get().unwrap();
	diesel::delete(cards::table.filter(cards::item_id.eq(&item_id)))
		.execute(conn)
		.unwrap();
	world.cards.retain(|c| c.get_item_id() != item_id);
}

proptest! {
	/// Q7B.1: An item with zero cards is excluded from items_matching when a
	/// card-level filter is set, regardless of which card-level filter.
	#[test]
	fn prop_q7b_1_zero_card_item_excluded_under_any_card_filter(
		n_items in 2usize..=4,
		stripped_ix in 0usize..4,
		cutoff in arb_datetime_utc(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let stripped_ix = stripped_ix % n_items;
			let stripped_item_id = world.items[stripped_ix].get_id();
			delete_cards_for_item(&pool, &mut world, stripped_ix).await;

			// `suspended_filter::Exclude` is the default and therefore NOT a
			// card-level filter (per DESIRED_BEHAVIOUR.md: "any non-default"
			// suspended_filter triggers the zero-card exclusion). Q7B.2 pins
			// that default-query zero-card items are included, so we cannot
			// simultaneously treat Exclude as a card-level filter here.
			for query in [
				GetQueryDtoBuilder::new().next_review_before(cutoff).build(),
				GetQueryDtoBuilder::new().last_review_after(cutoff).build(),
				GetQueryDtoBuilder::new()
					.suspended_filter(SuspendedFilter::Only)
					.build(),
				GetQueryDtoBuilder::new()
					.suspended_filter(SuspendedFilter::Include)
					.suspended_after(cutoff)
					.build(),
				GetQueryDtoBuilder::new()
					.suspended_filter(SuspendedFilter::Include)
					.suspended_before(cutoff)
					.build(),
			] {
				let sql = sql_items_matching(&pool, &query);
				let oracle = oracle_items_matching(&world, &query);
				prop_assert_eq!(&sql, &oracle);
				prop_assert!(
					!sql.contains(&stripped_item_id),
					"zero-card item {:?} must not appear in items_matching under any card-level filter",
					stripped_item_id
				);
			}
			Ok(())
		})?;
	}

	/// Q7B.2: With NO card-level filter, a zero-card item must STILL be
	/// returned by items_matching — the absence of cards is only disqualifying
	/// when there's a card predicate to satisfy.
	#[test]
	fn prop_q7b_2_zero_card_item_included_when_no_card_filter(
		n_items in 2usize..=4,
		stripped_ix in 0usize..4,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let stripped_ix = stripped_ix % n_items;
			let stripped_item_id = world.items[stripped_ix].get_id();
			delete_cards_for_item(&pool, &mut world, stripped_ix).await;

			let query = GetQueryDto::default();
			let sql = sql_items_matching(&pool, &query);
			let oracle = oracle_items_matching(&world, &query);
			prop_assert_eq!(&sql, &oracle);
			prop_assert!(
				sql.contains(&stripped_item_id),
				"zero-card item {:?} must appear in items_matching when no card-level filter is set",
				stripped_item_id
			);
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q8: reviews_matching
// ===========================================================================

/// Setup helper: record some reviews so reviews_matching has anything to do.
///
/// `record_review` also mutates the reviewed card's `last_review`,
/// `next_review`, and `scheduler_data` in the DB. We re-read every card from
/// the DB so `world.cards` stays in lockstep with on-disk state — otherwise
/// date-based oracles would compare against pre-review Rust values while SQL
/// sees the post-review DB values, giving false failures on tests like Q6.1
/// and Q8.6.
async fn add_reviews(pool: &DbPool, world: &mut TestWorld, ratings: &[(usize, i32)]) {
	for (ci, rating) in ratings {
		let card_id = world.cards[*ci % world.cards.len()].get_id();
		let r = record_review(pool, &card_id, *rating).await.unwrap();
		world.reviews.push(r);
	}
	for card in world.cards.iter_mut() {
		*card = crate::repo::get_card_raw(pool, &card.get_id())
			.unwrap()
			.unwrap();
	}
}

proptest! {
	/// Q8.1: `reviews_matching` returns the reviews of cards selected by
	/// `cards_matching` for the same query.
	#[test]
	fn prop_q8_1_reviews_match_cards_set(
		n_items in 1usize..=3,
		ratings in prop::collection::vec((0usize..6, 1i32..=4), 1..8),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_reviews(&pool, &mut world, &ratings).await;

			let query = GetQueryDto::default();
			let sql = sql_reviews_matching(&pool, &query);
			let oracle = oracle_reviews_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q8.2: A query that excludes every card (e.g. unknown item type) yields
	/// no reviews.
	#[test]
	fn prop_q8_2_excluded_cards_yield_no_reviews(
		n_items in 1usize..=3,
		ratings in prop::collection::vec((0usize..6, 1i32..=4), 1..6),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_reviews(&pool, &mut world, &ratings).await;

			let query = GetQueryDtoBuilder::new()
				.item_type_id(ItemTypeId("ghost-type".to_owned()))
				.build();
			let sql = sql_reviews_matching(&pool, &query);
			prop_assert!(sql.is_empty());
			Ok(())
		})?;
	}

	/// Q8.3: Filtering reviews by item_type_id selects exactly the reviews
	/// of cards belonging to that type.
	#[test]
	fn prop_q8_3_reviews_filter_by_item_type(
		ratings in prop::collection::vec((0usize..4, 1i32..=4), 1..6),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 2, 2).await;
			add_reviews(&pool, &mut world, &ratings).await;

			let query = GetQueryDtoBuilder::new()
				.item_type_id(world.item_types[0].clone())
				.build();
			let sql = sql_reviews_matching(&pool, &query);
			let oracle = oracle_reviews_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q8.4: Tag filter propagates to reviews — only reviews of cards on
	/// items tagged with all the requested tags are returned.
	#[test]
	fn prop_q8_4_reviews_filter_by_tags(
		n_items in 2usize..=4,
		n_tags in 1usize..=3,
		attach_pairs in prop::collection::vec((0usize..4, 0usize..3), 0..8),
		ratings in prop::collection::vec((0usize..4, 1i32..=4), 1..6),
		query_tag_ixs in prop::collection::vec(0usize..3, 1..=2),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_tags(&pool, &mut world, n_tags).await;
			let pairs: Vec<(usize, usize)> = attach_pairs
				.into_iter()
				.map(|(i, t)| (i % n_items, t % n_tags))
				.collect();
			attach_tags(&pool, &mut world, &pairs).await;
			add_reviews(&pool, &mut world, &ratings).await;

			let mut query_tags: Vec<TagId> = query_tag_ixs
				.into_iter()
				.map(|ix| world.tags[ix % n_tags].clone())
				.collect();
			query_tags.sort();
			query_tags.dedup();

			let query = GetQueryDtoBuilder::new().tag_ids(query_tags).build();

			let sql = sql_reviews_matching(&pool, &query);
			let oracle = oracle_reviews_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q8.5b: `child_item_id` propagates to reviews. Symmetric mirror of
	/// Q8.5 — without this, a bug that dropped `child_item_id` from
	/// `reviews_matching` would only be caught by the omnibus Q6.1, making
	/// the failure far harder to diagnose.
	#[test]
	fn prop_q8_5b_reviews_filter_by_child_item(
		n_items in 2usize..=5,
		relations in prop::collection::vec((0usize..5, 0usize..5), 0..8),
		ratings in prop::collection::vec((0usize..6, 1i32..=4), 1..6),
		child_ix in 0usize..5,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let pairs: Vec<(usize, usize)> = relations
				.into_iter()
				.map(|(p, c)| (p % n_items, c % n_items))
				.collect();
			add_relations(&pool, &mut world, &pairs).await;
			add_reviews(&pool, &mut world, &ratings).await;

			let child_id = world.items[child_ix % n_items].get_id();
			let query = GetQueryDtoBuilder::new().child_item_id(child_id).build();

			let sql = sql_reviews_matching(&pool, &query);
			let oracle = oracle_reviews_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q8.5: `parent_item_id` propagates to reviews.
	#[test]
	fn prop_q8_5_reviews_filter_by_parent_item(
		n_items in 2usize..=5,
		relations in prop::collection::vec((0usize..5, 0usize..5), 0..8),
		ratings in prop::collection::vec((0usize..6, 1i32..=4), 1..6),
		parent_ix in 0usize..5,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			let pairs: Vec<(usize, usize)> = relations
				.into_iter()
				.map(|(p, c)| (p % n_items, c % n_items))
				.collect();
			add_relations(&pool, &mut world, &pairs).await;
			add_reviews(&pool, &mut world, &ratings).await;

			let parent_id = world.items[parent_ix % n_items].get_id();
			let query = GetQueryDtoBuilder::new().parent_item_id(parent_id).build();

			let sql = sql_reviews_matching(&pool, &query);
			let oracle = oracle_reviews_matching(&world, &query);
			prop_assert_eq!(sql, oracle);
			Ok(())
		})?;
	}

	/// Q8.6: Date filters (`next_review_before`, `last_review_after`)
	/// propagate to reviews — picking only reviews of cards passing the
	/// date predicates.
	#[test]
	fn prop_q8_6_reviews_filter_by_dates(
		n_items in 2usize..=4,
		date_muts in prop::collection::vec(
			(
				prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
				prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
				Just(None),
			),
			1..6
		),
		ratings in prop::collection::vec((0usize..6, 1i32..=4), 1..6),
		next_cutoff in arb_datetime_utc(),
		last_cutoff in arb_datetime_utc(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_reviews(&pool, &mut world, &ratings).await;
			mutate_cards_for_dates(&pool, &mut world, &date_muts).await;

			for query in [
				GetQueryDtoBuilder::new().next_review_before(next_cutoff).build(),
				GetQueryDtoBuilder::new().last_review_after(last_cutoff).build(),
			] {
				let sql = sql_reviews_matching(&pool, &query);
				let oracle = oracle_reviews_matching(&world, &query);
				prop_assert_eq!(sql, oracle);
			}
			Ok(())
		})?;
	}

	/// Q8.7: `suspended_filter` (Exclude / Only) and `suspended_after` /
	/// `suspended_before` propagate to reviews.
	#[test]
	fn prop_q8_7_reviews_filter_by_suspended(
		n_items in 2usize..=4,
		susp_pattern in prop::collection::vec(
			prop_oneof![Just(None), arb_datetime_utc().prop_map(Some)],
			1..6
		),
		ratings in prop::collection::vec((0usize..6, 1i32..=4), 1..6),
		cutoff in arb_datetime_utc(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_reviews(&pool, &mut world, &ratings).await;
			let muts: Vec<_> = susp_pattern.iter().map(|s| (None, None, *s)).collect();
			mutate_cards_for_dates(&pool, &mut world, &muts).await;

			for query in [
				GetQueryDtoBuilder::new()
					.suspended_filter(SuspendedFilter::Exclude)
					.build(),
				GetQueryDtoBuilder::new()
					.suspended_filter(SuspendedFilter::Only)
					.build(),
				GetQueryDtoBuilder::new()
					.suspended_filter(SuspendedFilter::Include)
					.suspended_after(cutoff)
					.build(),
				GetQueryDtoBuilder::new()
					.suspended_filter(SuspendedFilter::Include)
					.suspended_before(cutoff)
					.build(),
			] {
				let sql = sql_reviews_matching(&pool, &query);
				let oracle = oracle_reviews_matching(&world, &query);
				prop_assert_eq!(sql, oracle);
			}
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q9: split_priority is presentation-only and must NOT affect matching
// ===========================================================================

proptest! {
	/// Q9.1: Setting `split_priority` to either `Some(true)` or `Some(false)`
	/// produces the same matched id set as `None`, for all three matching
	/// functions. `split_priority` is presentational and must not affect
	/// row selection.
	#[test]
	fn prop_q9_1_split_priority_is_ignored(
		n_items in 1usize..=3,
		choice in 0u8..3,
		ratings in prop::collection::vec((0usize..6, 1i32..=4), 0..6),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let mut world = build_basic_world(&pool, 1, n_items).await;
			add_reviews(&pool, &mut world, &ratings).await;

			let mut q = GetQueryDtoBuilder::new();
			match choice {
				0 => {} // None
				1 => q = q.split_priority(true),
				_ => q = q.split_priority(false),
			};
			let q = q.build();

			let default_q = GetQueryDto::default();

			let sql_c = sql_cards_matching(&pool, &q);
			let baseline_c = sql_cards_matching(&pool, &default_q);
			prop_assert_eq!(sql_c, baseline_c);

			let sql_i = sql_items_matching(&pool, &q);
			let baseline_i = sql_items_matching(&pool, &default_q);
			prop_assert_eq!(sql_i, baseline_i);

			let sql_r = sql_reviews_matching(&pool, &q);
			let baseline_r = sql_reviews_matching(&pool, &default_q);
			prop_assert_eq!(sql_r, baseline_r);
			Ok(())
		})?;
	}
}

// ===========================================================================
// Q10: composability — the returned subquery is usable inside other diesel
// queries via .eq_any(). This test pins the *type* shape of the API as well
// as its semantics.
// ===========================================================================

#[tokio::test]
async fn q10_1_subquery_usable_with_eq_any() {
	use crate::schema::cards;
	let pool: Arc<DbPool> = setup_test_db();
	let world = build_basic_world(&pool, 1, 3).await;
	let target_type = world.item_types[0].clone();
	let query = GetQueryDtoBuilder::new()
		.item_type_id(target_type.clone())
		.build();

	let conn = &mut pool.get().unwrap();
	// The composability we care about: cards_matching(...) is usable as a
	// subquery argument to `eq_any`. If the type ever changes shape and
	// breaks this, this test fails to compile.
	let count: i64 = cards::table
		.filter(cards::id.eq_any(cards_matching(&query)))
		.count()
		.get_result(conn)
		.unwrap();
	let oracle = oracle_cards_matching(&world, &query);
	assert_eq!(count as usize, oracle.len());
}

/// Q10.2: The subquery is also usable inside DML builders — the spec-level
/// example in `DESIRED_BEHAVIOUR.md` is
/// `diesel::update(items::table.filter(items::id.eq_any(items_matching(&query)))).set(...)`.
/// If a future Diesel type tweak breaks that embedding, this test fails to
/// compile. We use a query that matches zero items (nonexistent item type)
/// so the UPDATE is a runtime no-op regardless of impl correctness.
#[tokio::test]
async fn q10_2_subquery_usable_in_dml_filter() {
	use crate::schema::items;
	let pool: Arc<DbPool> = setup_test_db();
	let _world = build_basic_world(&pool, 1, 3).await;

	let query = GetQueryDtoBuilder::new()
		.item_type_id(ItemTypeId("ghost-type".to_owned()))
		.build();

	let conn = &mut pool.get().unwrap();
	let affected = diesel::update(items::table.filter(items::id.eq_any(items_matching(&query))))
		.set(items::title.eq("compat-check"))
		.execute(conn)
		.unwrap();
	assert_eq!(
		affected, 0,
		"query selects no items (ghost type) — update must affect zero rows",
	);
}

// ===========================================================================
// Regression replays.
//
// Per `CLAUDE.md`'s testing philosophy: every seed proptest has saved to
// `proptest-regressions/repo/query_repo/prop_tests.txt` should also have a
// dedicated, named unit test exercising the specific failing input. The
// regression file guards the seed; these unit tests make each failure mode
// explicit and visible by name in the test output if it ever recurs.
//
// Each test below corresponds to one entry in that file; the comment above
// each test reproduces the shrunk input.
// ===========================================================================

/// Regression: `n_items = 1`. Smallest world the proptests can construct.
/// Pin the trivial case across every matching function under the default
/// (no-op) query — `Q0.1`, `Q0.2`, `Q7.1`, and `Q8.1` all shrink here.
#[tokio::test]
async fn regression_n_items_1_default_query_matches_everything() {
	let pool = setup_test_db();
	let world = build_basic_world(&pool, 1, 1).await;
	let query = GetQueryDto::default();

	assert_eq!(
		sql_cards_matching(&pool, &query),
		oracle_cards_matching(&world, &query)
	);
	assert_eq!(
		sql_items_matching(&pool, &query),
		oracle_items_matching(&world, &query)
	);
	assert_eq!(
		sql_reviews_matching(&pool, &query),
		oracle_reviews_matching(&world, &query)
	);
}

/// Regression: `n_types = 2, items_per_type = 1, type_ix = 0` for cards
/// (Q1.1 shrink). Two types, one item each, query for cards of type 0.
#[tokio::test]
async fn regression_q1_1_two_types_one_item_each_filter_cards_by_type_0() {
	let pool = setup_test_db();
	let world = build_basic_world(&pool, 2, 1).await;
	let target = world.item_types[0].clone();
	let query = GetQueryDtoBuilder::new().item_type_id(target).build();

	let sql = sql_cards_matching(&pool, &query);
	let oracle = oracle_cards_matching(&world, &query);
	assert_eq!(sql, oracle);
}

/// Regression: `n_types = 2, items_per_type = 1, type_ix = 0` for items
/// (Q1.2 shrink). Same world, asserts items_matching.
#[tokio::test]
async fn regression_q1_2_two_types_one_item_each_filter_items_by_type_0() {
	let pool = setup_test_db();
	let world = build_basic_world(&pool, 2, 1).await;
	let target = world.item_types[0].clone();
	let query = GetQueryDtoBuilder::new().item_type_id(target).build();

	let sql = sql_items_matching(&pool, &query);
	let oracle = oracle_items_matching(&world, &query);
	assert_eq!(sql, oracle);
}

/// Regression: `n_items = 3, n_tags = 2, attach_pairs = [], query_tag_ix = 0`
/// (Q2.1 shrink). No tags attached to any item — querying for a real tag
/// must return the empty set, not "all items".
#[tokio::test]
async fn regression_q2_1_no_attachments_single_tag_query_returns_empty() {
	let pool = setup_test_db();
	let mut world = build_basic_world(&pool, 1, 3).await;
	add_tags(&pool, &mut world, 2).await;
	// no attach_tags

	let query = GetQueryDtoBuilder::new()
		.add_tag_id(world.tags[0].clone())
		.build();

	let sql_c = sql_cards_matching(&pool, &query);
	let oracle_c = oracle_cards_matching(&world, &query);
	assert_eq!(sql_c, oracle_c);
	assert!(sql_c.is_empty(), "no item has tag — set must be empty");

	let sql_i = sql_items_matching(&pool, &query);
	let oracle_i = oracle_items_matching(&world, &query);
	assert_eq!(sql_i, oracle_i);
	assert!(sql_i.is_empty());
}

/// Regression: `n_items = 2, n_tags = 2, attach_pairs = [], query_tag_ixs = [0]`
/// (Q2.2 shrink). Same as above but exercising the multi-tag (`tag_ids`)
/// code path with a 1-element vec.
#[tokio::test]
async fn regression_q2_2_no_attachments_tag_id_list_returns_empty() {
	let pool = setup_test_db();
	let mut world = build_basic_world(&pool, 1, 2).await;
	add_tags(&pool, &mut world, 2).await;

	let query = GetQueryDtoBuilder::new()
		.tag_ids(vec![world.tags[0].clone()])
		.build();

	let sql_c = sql_cards_matching(&pool, &query);
	let oracle_c = oracle_cards_matching(&world, &query);
	assert_eq!(sql_c, oracle_c);
	assert!(sql_c.is_empty());

	let sql_i = sql_items_matching(&pool, &query);
	let oracle_i = oracle_items_matching(&world, &query);
	assert_eq!(sql_i, oracle_i);
	assert!(sql_i.is_empty());
}

/// Regression: `garbage = ""` (Q1.3 shrink). Empty-string item_type_id with
/// the literal prefix `"not-real-"` — exercises that an unknown but
/// well-formed type id selects nothing without erroring.
#[tokio::test]
async fn regression_q1_3_unknown_item_type_id_empty_garbage_returns_empty() {
	let pool = setup_test_db();
	let _world = build_basic_world(&pool, 2, 2).await;

	let query = GetQueryDtoBuilder::new()
		.item_type_id(ItemTypeId("not-real-".to_string()))
		.build();

	assert!(sql_cards_matching(&pool, &query).is_empty());
	assert!(sql_items_matching(&pool, &query).is_empty());
}

/// Regression: `garbage = ""` (Q2.5 shrink). Empty-string tag id with the
/// literal prefix `"not-a-tag-"` — exercises unknown but well-formed tag id.
#[tokio::test]
async fn regression_q2_5_unknown_tag_id_empty_garbage_returns_empty() {
	let pool = setup_test_db();
	let _world = build_basic_world(&pool, 1, 2).await;

	let query = GetQueryDtoBuilder::new()
		.add_tag_id(TagId("not-a-tag-".to_string()))
		.build();

	assert!(sql_cards_matching(&pool, &query).is_empty());
	assert!(sql_items_matching(&pool, &query).is_empty());
}

/// Regression: `n_items = 2, mutations = [(None, None, None)],
/// cutoff = 2020-01-01T00:00:00Z` (Q3.2 shrink). All cards are unmodified
/// (NULL `last_review`), so `last_review_after(<any cutoff>)` must select
/// nothing — the NULL-falsy semantics. The cutoff at 2020 is the earliest
/// `arb_datetime_utc` can produce.
#[tokio::test]
async fn regression_q3_2_null_last_review_with_early_cutoff_selects_nothing() {
	let pool = setup_test_db();
	let world = build_basic_world(&pool, 1, 2).await;
	// no date mutations — all cards have last_review = NULL

	let cutoff = DateTime::from_timestamp(1_577_836_800, 0).unwrap();
	let query = GetQueryDtoBuilder::new().last_review_after(cutoff).build();

	let sql = sql_cards_matching(&pool, &query);
	let oracle = oracle_cards_matching(&world, &query);
	assert_eq!(sql, oracle);
	assert!(
		sql.is_empty(),
		"NULL last_review must not pass last_review_after"
	);
}

/// Regression: `n_items = 2, mutations = [(Some(2026-09-02T09:42:07Z), None,
/// None)], cutoff = 2020-01-01T00:00:00Z` (Q3.1 shrink). All cards have a
/// `next_review` in the future and the cutoff is in the past — every card
/// must be excluded.
#[tokio::test]
async fn regression_q3_1_future_next_review_with_early_cutoff_selects_nothing() {
	let pool = setup_test_db();
	let mut world = build_basic_world(&pool, 1, 2).await;
	let future = DateTime::from_timestamp(1_756_806_127, 0).unwrap(); // 2026-09-02T09:42:07Z
	let mutations = vec![(Some(future), None, None)];
	mutate_cards_for_dates(&pool, &mut world, &mutations).await;

	let cutoff = DateTime::from_timestamp(1_577_836_800, 0).unwrap();
	let query = GetQueryDtoBuilder::new().next_review_before(cutoff).build();

	let sql = sql_cards_matching(&pool, &query);
	let oracle = oracle_cards_matching(&world, &query);
	assert_eq!(sql, oracle);
	assert!(
		sql.is_empty(),
		"future next_review must not pass next_review_before(past cutoff)"
	);
}

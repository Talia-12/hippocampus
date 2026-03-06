use super::*;
use crate::models::OrderIndex;
use crate::repo::{create_item_type, tests::setup_test_db};
use crate::schema::item_types;
use crate::test_utils::{arb_messy_string, dedup_names};
use diesel::{QueryDsl, RunQueryDsl};
use proptest::prelude::*;
use std::collections::HashSet;

/// A function name that is guaranteed to *not* be present in the registry.
/// Used by tests that need to check behaviour for orphaned DB rows pointing
/// at functions we never registered — the three `test_*` functions are the
/// only registered names under `feature = "test"`, so anything else will
/// cycle through the `UnknownFunction` path.
const UNREGISTERED_FN: &str = "never_registered_anywhere";

/// Event names generated here must be registered in the test registry —
/// the repo now rejects unknown names up-front with `UnknownFunction`.
fn arb_event_name() -> impl Strategy<Value = CardEventFnName> {
	prop_oneof![
		Just(CardEventFnName("test_set_title".to_owned())),
		Just(CardEventFnName("test_increment".to_owned())),
		Just(CardEventFnName("test_fail".to_owned())),
	]
}

proptest! {
	/// CFE1.1: For an arbitrary set of (order_index, function_name) inserts,
	/// `list_events_for_item_type` returns events sorted ascending by
	/// `order_index`.
	#[test]
	fn prop_cfe1_1_list_events_ordered_by_order_index(
		indices in prop::collection::vec(0u16..=u16::MAX, 1..8),
		raw_names in prop::collection::vec(arb_messy_string(), 1..8),
	) {
		// Both vectors must be the same length and contain unique
		// (order_index, function_name) pairs to satisfy table constraints.
		//
		// Also: the registered-function check now happens in the repo, so we
		// must restrict `function_name` to names that are registered. We map
		// the messy input into one of the three test-registered names, cycling
		// through them by dedup-index to ensure uniqueness per item type.
		let registered = ["test_set_title", "test_increment", "test_fail"];
		let pairs: Vec<(OrderIndex, CardEventFnName)> = {
			let mut seen_idx = HashSet::new();
			let mut idx_iter = indices
				.into_iter()
				.filter(|i| seen_idx.insert(*i))
				.map(OrderIndex);
			let deduped = dedup_names(raw_names);
			let mut pairs = Vec::new();
			// Per-item-type uniqueness of function_name is required; since
			// there are only three registered names, cap to three.
			for (i, _) in deduped.into_iter().enumerate().take(registered.len()) {
				if let Some(idx) = idx_iter.next() {
					pairs.push((idx, CardEventFnName(registered[i].to_owned())));
				} else {
					break;
				}
			}
			pairs
		};
		prop_assume!(!pairs.is_empty());

		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type =
				create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned()).await.unwrap();

			for (idx, name) in &pairs {
				create_card_fetched_event(&pool, &item_type.get_id(), *idx, name.clone())
					.await
					.unwrap();
			}

			let events = list_events_for_item_type(&pool, &item_type.get_id()).unwrap();

			// Same set returned
			prop_assert_eq!(events.len(), pairs.len());

			// Strictly ascending order_index (we filtered to unique indices)
			for w in events.windows(2) {
				prop_assert!(
					w[0].get_order_index() < w[1].get_order_index(),
					"events not sorted by order_index"
				);
			}
			Ok(())
		})?;
	}

	/// CFE2.1: A second insert with the same (item_type_id, function_name)
	/// returns Duplicate.
	#[test]
	fn prop_cfe2_1_unique_function_name(name in arb_event_name()) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type =
				create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned()).await.unwrap();

			let r1 = create_card_fetched_event(&pool, &item_type.get_id(), OrderIndex(0), name.clone()).await;
			prop_assert!(r1.is_ok(), "first insert failed: {:?}", r1.err());

			// Same function_name, different order_index — still a duplicate
			// because of the UNIQUE (item_type_id, function_name) constraint.
			let r2 = create_card_fetched_event(&pool, &item_type.get_id(), OrderIndex(1), name.clone()).await;
			prop_assert!(matches!(r2, Err(CreateCardFetchedEventError::Duplicate)));
			Ok(())
		})?;
	}

	/// CFE2.2: A second insert with the same (item_type_id, order_index)
	/// returns Duplicate (PRIMARY KEY collision).
	#[test]
	fn prop_cfe2_2_unique_order_index(
		name1 in arb_event_name(),
		name2 in arb_event_name(),
	) {
		prop_assume!(name1 != name2);
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type =
				create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned()).await.unwrap();

			let r1 = create_card_fetched_event(&pool, &item_type.get_id(), OrderIndex(7), name1).await;
			prop_assert!(r1.is_ok(), "first insert failed: {:?}", r1.err());

			let r2 = create_card_fetched_event(&pool, &item_type.get_id(), OrderIndex(7), name2).await;
			prop_assert!(matches!(r2, Err(CreateCardFetchedEventError::Duplicate)));
			Ok(())
		})?;
	}

	/// CFE3.1: delete_card_fetched_event with no matching row returns NotFound.
	#[test]
	fn prop_cfe3_1_delete_missing_returns_not_found(
		name in arb_event_name(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type =
				create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned()).await.unwrap();

			let r = delete_card_fetched_event(&pool, &item_type.get_id(), &name).await;
			prop_assert!(matches!(r, Err(DeleteCardFetchedEventError::NotFound)));
			Ok(())
		})?;
	}

	/// CFE4.1: An insert referencing a non-existent `item_type_id` returns
	/// `ItemTypeNotFound`. This is the atomic replacement for the previous
	/// handler-level pre-check.
	#[test]
	fn prop_cfe4_1_unknown_item_type_returns_not_found(
		bogus in arb_messy_string(),
		name in arb_event_name(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let r = create_card_fetched_event(
				&pool,
				&crate::models::ItemTypeId(bogus),
				OrderIndex(0),
				name,
			)
			.await;
			prop_assert!(matches!(r, Err(CreateCardFetchedEventError::ItemTypeNotFound)));
			Ok(())
		})?;
	}

	/// CFE4.2: The SQL-level `CHECK (order_index >= 0)` constraint is a
	/// defense-in-depth for any path that bypasses the `OrderIndex(u16)`
	/// Rust newtype — e.g. manual SQL, migrations, or another language
	/// binding. A raw INSERT with `order_index = -1` must fail with a
	/// constraint violation.
	#[test]
	fn prop_cfe4_2_sql_check_rejects_negative_order_index(
		bogus_idx in i32::MIN..0,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type =
				create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned()).await.unwrap();
			let conn = &mut pool.get().unwrap();
			let inserted = diesel::sql_query(
				"INSERT INTO card_fetched_events (item_type_id, order_index, function_name) \
				 VALUES (?, ?, 'anything')",
			)
			.bind::<diesel::sql_types::Text, _>(item_type.get_id().0.clone())
			.bind::<diesel::sql_types::Integer, _>(bogus_idx)
			.execute(conn);
			prop_assert!(inserted.is_err(), "negative order_index unexpectedly inserted");
			Ok(())
		})?;
	}

	/// CFE5.1: An insert with a function_name that isn't registered returns
	/// `UnknownFunction` — catching the 400-worthy case before it reaches the
	/// DB at all.
	#[test]
	fn prop_cfe5_1_unknown_function_rejected(
		bogus in arb_messy_string()
			.prop_filter(
				"must not be a registered function name",
				|s| !matches!(
					s.as_str(),
					"test_set_title" | "test_increment" | "test_fail"
				),
			),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type =
				create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned()).await.unwrap();

			let r = create_card_fetched_event(
				&pool,
				&item_type.get_id(),
				OrderIndex(0),
				CardEventFnName(bogus),
			)
			.await;
			prop_assert!(matches!(r, Err(CreateCardFetchedEventError::UnknownFunction(_))));
			Ok(())
		})?;
	}

	/// CFE3.2: `delete_card_fetched_event` targeting an item type that
	/// doesn't exist at all returns `ItemTypeNotFound`, distinguishing the
	/// "wrong item type id" case from the "item type exists but no such
	/// event" case (CFE3.1). Pins the error-variant symmetry with the
	/// Create path: both Create and Delete now emit `ItemTypeNotFound` for
	/// the same pre-condition failure.
	#[test]
	fn prop_cfe3_2_delete_unknown_item_type_returns_item_type_not_found(
		bogus in arb_messy_string(),
		name in arb_event_name(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let r = delete_card_fetched_event(
				&pool,
				&crate::models::ItemTypeId(bogus),
				&name,
			).await;
			prop_assert!(
				matches!(r, Err(DeleteCardFetchedEventError::ItemTypeNotFound)),
				"got {:?}, expected ItemTypeNotFound",
				r.err(),
			);
			Ok(())
		})?;
	}

	/// CFE6.1: `list_events_for_item_type` targeting a non-existent item
	/// type returns `ItemTypeNotFound`, never an empty vec. The distinction
	/// lets the HTTP layer send 404 (vs 200 with `[]`) for a genuinely
	/// missing type — previously this lived in a separate handler pre-check
	/// with a TOCTOU window.
	#[test]
	fn prop_cfe6_1_list_unknown_item_type_returns_not_found(
		bogus in arb_messy_string(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let r = list_events_for_item_type(&pool, &crate::models::ItemTypeId(bogus));
			prop_assert!(
				matches!(r, Err(ListEventsForItemTypeError::ItemTypeNotFound)),
				"got {:?}, expected ItemTypeNotFound",
				r.err(),
			);
			Ok(())
		})?;
	}
}

/// CFE6.2: When the item type exists but has no events registered,
/// `list_events_for_item_type` returns `Ok(empty vec)`, not
/// `ItemTypeNotFound`. Pinning this case explicitly (rather than only via
/// CFE6.1's negation) keeps the empty-vs-missing distinction visible in
/// the test output.
#[tokio::test]
async fn cfe6_2_list_existing_item_type_with_no_events_returns_empty_vec() {
	let pool = setup_test_db();
	let item_type = create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned())
		.await
		.unwrap();

	let events = list_events_for_item_type(&pool, &item_type.get_id())
		.expect("existing item type with no events should return Ok(empty), not NotFound");
	assert!(
		events.is_empty(),
		"expected empty vec; got {} events",
		events.len()
	);
}

/// Orphaned-function-name regression: if the DB holds an event row whose
/// `function_name` is not in the in-memory registry (e.g. a deploy removed
/// the function while the row stayed behind), a subsequent `get_card` must
/// surface a structured `CardFetchError::EventChain` rather than
/// collapsing into an opaque `anyhow::Error`. The `#[cfg(any(test, feature = "test"))]`
/// test functions in `card_event_registry` give us a fixed, known-registered
/// set of names to compare against; we reach the DB directly to insert a
/// row under a name that we know is *not* in that set.
#[tokio::test]
async fn cfe7_1_orphaned_function_name_surfaces_typed_event_chain_error() {
	use crate::card_event_registry::CardEventChainError;
	use crate::repo::{CardFetchError, create_item, get_cards_for_item};

	let pool = setup_test_db();
	let item_type = create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned())
		.await
		.unwrap();

	// Bypass the repo's `is_registered` gate by writing the orphaned row
	// via raw SQL. This is precisely the state the test is meant to simulate:
	// a row that was once valid is now pointing at a function no deploy knows
	// about.
	{
		let conn = &mut pool.get().unwrap();
		diesel::sql_query(
			"INSERT INTO card_fetched_events (item_type_id, order_index, function_name) \
			 VALUES (?, 0, ?)",
		)
		.bind::<diesel::sql_types::Text, _>(item_type.get_id().0.clone())
		.bind::<diesel::sql_types::Text, _>(UNREGISTERED_FN.to_owned())
		.execute(conn)
		.unwrap();
	}

	let item = create_item(
		&pool,
		&item_type.get_id(),
		"t".to_owned(),
		serde_json::json!({}),
	)
	.await
	.unwrap();
	let card = get_cards_for_item(&pool, &item.get_id()).unwrap()[0].clone();

	let err = crate::repo::get_card(&pool, &card.get_id())
		.await
		.expect_err("fetching a card whose item type references an unregistered function should error");

	match err {
		CardFetchError::EventChain(CardEventChainError::FunctionsNotFound(names)) => {
			assert!(
				names.iter().any(|n| n.0 == UNREGISTERED_FN),
				"expected `{}` in missing names, got {:?}",
				UNREGISTERED_FN,
				names
			);
		}
		other => panic!(
			"expected CardFetchError::EventChain(FunctionsNotFound(..)), got {:?}",
			other
		),
	}
}

/// CFE8.1: Deleting the **last** event for an item type must clear the
/// cached `card_data` (and `cache_updated_at`) of every card whose item
/// belongs to that type.
///
/// Without this, cards that were previously fetched retain their old
/// cached output forever: the `ensure_list_cards_cache_conn` EXISTS guard
/// skips item types with no events, so the stale row is never recomputed,
/// and from the client's perspective `card_data` stays pinned to whatever
/// the (now-removed) chain last produced.
///
/// Covers the `delete_card_fetched_event` path specifically. Partial
/// deletes (leaving at least one event behind) are left alone — the
/// staleness check via `item_types.updated_at` handles those.
#[tokio::test]
async fn cfe8_1_deleting_last_event_clears_cached_card_data() {
	use crate::card_event_registry::test_fns::REGISTERED_NAMES;
	use crate::repo::{create_item, get_card, get_cards_for_item};

	// Sanity: the two test-registry names we rely on here must actually be
	// registered — otherwise the setup registers nothing and the test
	// tautologically passes.
	assert!(REGISTERED_NAMES.contains(&"test_set_title"));
	assert!(REGISTERED_NAMES.contains(&"test_increment"));

	let pool = setup_test_db();
	let item_type = create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned())
		.await
		.unwrap();

	// Register two events so the first delete is a *partial* removal and
	// the second is the final one. Lets us assert: partial-removal keeps
	// the cache populated (it's still valid, just with a different chain
	// composition), final-removal clears it.
	create_card_fetched_event(
		&pool,
		&item_type.get_id(),
		OrderIndex(0),
		CardEventFnName("test_set_title".to_owned()),
	)
	.await
	.unwrap();
	create_card_fetched_event(
		&pool,
		&item_type.get_id(),
		OrderIndex(1),
		CardEventFnName("test_increment".to_owned()),
	)
	.await
	.unwrap();

	let item = create_item(
		&pool,
		&item_type.get_id(),
		"t".to_owned(),
		serde_json::json!({}),
	)
	.await
	.unwrap();
	let card = get_cards_for_item(&pool, &item.get_id()).unwrap()[0].clone();

	// Fill the cache.
	let filled = get_card(&pool, &card.get_id()).await.unwrap().unwrap();
	assert!(
		filled.get_card_data().is_some(),
		"precondition: cache should be filled after fetching with events registered"
	);

	// First delete leaves `test_increment` behind — cache should *not* be
	// cleared, because the chain is still non-empty and the
	// `item_types.updated_at` bump will drive a recompute on next fetch.
	delete_card_fetched_event(
		&pool,
		&item_type.get_id(),
		&CardEventFnName("test_set_title".to_owned()),
	)
	.await
	.unwrap();
	let after_partial = crate::repo::get_card_raw(&pool, &card.get_id()).unwrap().unwrap();
	assert!(
		after_partial.get_card_data().is_some(),
		"partial event removal should not clear card_data; that's what the \
		 staleness check is for"
	);

	// Now delete the last remaining event — cache must be reset so that
	// subsequent fetches return `card_data = None`.
	delete_card_fetched_event(
		&pool,
		&item_type.get_id(),
		&CardEventFnName("test_increment".to_owned()),
	)
	.await
	.unwrap();

	// Direct row check: both cached fields must be NULL. Checked via the
	// raw read so we're sure we're observing the on-disk state, not
	// something `get_card` might transparently recompute.
	let after_final = crate::repo::get_card_raw(&pool, &card.get_id())
		.unwrap()
		.unwrap();
	assert!(
		after_final.get_card_data().is_none(),
		"card_data must be None after the last event is removed; got {:?}",
		after_final.get_card_data()
	);
	assert!(
		after_final.get_cache_updated_at_raw().is_none(),
		"cache_updated_at must be None after the last event is removed"
	);

	// And the cache-aware read path must agree — no "ghost" recompute
	// producing stale output.
	let after_final_fetched = get_card(&pool, &card.get_id()).await.unwrap().unwrap();
	assert!(
		after_final_fetched.get_card_data().is_none(),
		"get_card must return no card_data after the last event is removed"
	);
}

// ============================================================================
// CFE-TRIG: `update_item_type_on_event_update` trigger.
//
// The repo API exposes only INSERT (`create_card_fetched_event`) and DELETE
// (`delete_card_fetched_event`) for this table — UPDATE never happens through
// normal callers. The `update_item_type_on_event_update` trigger defends
// the `item_types.updated_at` invariant against *any* UPDATE path that
// could bypass the repo: a migration that renames a function, a manual SQL
// fix-up, another language binding, etc. Without the trigger an external
// UPDATE would leave `item_types.updated_at` stale, which in turn would
// keep `card_data` caches live when they should have been marked for
// recompute (the cache-staleness check reads `item_types.updated_at`).
//
// Since no public API exercises UPDATE on this table, these tests drive it
// via raw SQL — precisely the class of callers the trigger was meant to
// guard.
// ============================================================================

proptest! {
	/// CFE-TRIG.1: An UPDATE to any column of a `card_fetched_events` row
	/// bumps the owning item_type's `updated_at` strictly forward.
	///
	/// Exercises `order_index` mutation specifically — the most likely
	/// "manual fix-up" shape (reordering a chain without replacing its
	/// functions).
	#[test]
	fn prop_cfe_trig_1_event_update_bumps_item_type_updated_at(
		name in arb_event_name(),
		new_idx in 0u16..=u16::MAX,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type = create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned())
				.await
				.unwrap();

			create_card_fetched_event(&pool, &item_type.get_id(), OrderIndex(0), name.clone())
				.await
				.unwrap();

			let before: chrono::NaiveDateTime = item_types::table
				.find(item_type.get_id())
				.select(item_types::updated_at)
				.first(&mut pool.get().unwrap())
				.unwrap();

			// SQLite's `strftime('%Y-%m-%d %H:%M:%f', 'now')` has
			// millisecond precision. Without a small sleep here the
			// pre-UPDATE and post-UPDATE trigger-set timestamps can both
			// land in the same millisecond, and the strict-greater check
			// would spuriously fail even though the trigger fired
			// correctly.
			tokio::time::sleep(std::time::Duration::from_millis(10)).await;

			let conn = &mut pool.get().unwrap();
			diesel::sql_query(
				"UPDATE card_fetched_events SET order_index = ? \
				 WHERE item_type_id = ? AND function_name = ?",
			)
			.bind::<diesel::sql_types::Integer, _>(new_idx as i32)
			.bind::<diesel::sql_types::Text, _>(item_type.get_id().0.clone())
			.bind::<diesel::sql_types::Text, _>(name.0.clone())
			.execute(conn)
			.unwrap();

			let after: chrono::NaiveDateTime = item_types::table
				.find(item_type.get_id())
				.select(item_types::updated_at)
				.first(&mut pool.get().unwrap())
				.unwrap();

			prop_assert!(
				after > before,
				"item_type.updated_at must advance after UPDATE on its event row; before={:?} after={:?}",
				before,
				after,
			);
			Ok(())
		})?;
	}

	/// CFE-TRIG.2: Updating `function_name` (a different column than
	/// CFE-TRIG.1) also bumps the owning item type's `updated_at`. Pins
	/// that the trigger has no WHEN clause restricting which columns
	/// count — the whole point is to catch every UPDATE shape, not just
	/// the common ones.
	#[test]
	fn prop_cfe_trig_2_event_function_name_update_bumps_item_type_updated_at(
		original in arb_event_name(),
		replacement in arb_event_name(),
	) {
		prop_assume!(original != replacement);
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type = create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned())
				.await
				.unwrap();

			create_card_fetched_event(&pool, &item_type.get_id(), OrderIndex(0), original.clone())
				.await
				.unwrap();

			let before: chrono::NaiveDateTime = item_types::table
				.find(item_type.get_id())
				.select(item_types::updated_at)
				.first(&mut pool.get().unwrap())
				.unwrap();

			tokio::time::sleep(std::time::Duration::from_millis(10)).await;

			let conn = &mut pool.get().unwrap();
			diesel::sql_query(
				"UPDATE card_fetched_events SET function_name = ? \
				 WHERE item_type_id = ? AND function_name = ?",
			)
			.bind::<diesel::sql_types::Text, _>(replacement.0.clone())
			.bind::<diesel::sql_types::Text, _>(item_type.get_id().0.clone())
			.bind::<diesel::sql_types::Text, _>(original.0.clone())
			.execute(conn)
			.unwrap();

			let after: chrono::NaiveDateTime = item_types::table
				.find(item_type.get_id())
				.select(item_types::updated_at)
				.first(&mut pool.get().unwrap())
				.unwrap();

			prop_assert!(
				after > before,
				"item_type.updated_at must advance when function_name is updated; before={:?} after={:?}",
				before,
				after,
			);
			Ok(())
		})?;
	}

	/// CFE-TRIG.3: The bump touches *only* the owning item type — other
	/// item types in the DB keep their previous `updated_at`. Catches a
	/// class of trigger regressions where the `WHERE id = NEW.item_type_id`
	/// guard is dropped, causing every item type's cache to invalidate on
	/// any single event edit.
	#[test]
	fn prop_cfe_trig_3_event_update_bumps_only_owning_item_type(
		name_a in arb_event_name(),
		name_b in arb_event_name(),
		new_idx in 0u16..=u16::MAX,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let type_a =
				create_item_type(&pool, "Type A".to_owned(), "fsrs".to_owned()).await.unwrap();
			let type_b =
				create_item_type(&pool, "Type B".to_owned(), "fsrs".to_owned()).await.unwrap();

			create_card_fetched_event(&pool, &type_a.get_id(), OrderIndex(0), name_a.clone())
				.await
				.unwrap();
			create_card_fetched_event(&pool, &type_b.get_id(), OrderIndex(0), name_b.clone())
				.await
				.unwrap();

			let a_before: chrono::NaiveDateTime = item_types::table
				.find(type_a.get_id())
				.select(item_types::updated_at)
				.first(&mut pool.get().unwrap())
				.unwrap();
			let b_before: chrono::NaiveDateTime = item_types::table
				.find(type_b.get_id())
				.select(item_types::updated_at)
				.first(&mut pool.get().unwrap())
				.unwrap();

			tokio::time::sleep(std::time::Duration::from_millis(10)).await;

			let conn = &mut pool.get().unwrap();
			diesel::sql_query(
				"UPDATE card_fetched_events SET order_index = ? \
				 WHERE item_type_id = ? AND function_name = ?",
			)
			.bind::<diesel::sql_types::Integer, _>(new_idx as i32)
			.bind::<diesel::sql_types::Text, _>(type_a.get_id().0.clone())
			.bind::<diesel::sql_types::Text, _>(name_a.0.clone())
			.execute(conn)
			.unwrap();

			let a_after: chrono::NaiveDateTime = item_types::table
				.find(type_a.get_id())
				.select(item_types::updated_at)
				.first(&mut pool.get().unwrap())
				.unwrap();
			let b_after: chrono::NaiveDateTime = item_types::table
				.find(type_b.get_id())
				.select(item_types::updated_at)
				.first(&mut pool.get().unwrap())
				.unwrap();

			prop_assert!(
				a_after > a_before,
				"A.updated_at must advance; before={:?} after={:?}",
				a_before,
				a_after,
			);
			prop_assert_eq!(
				b_before, b_after,
				"B.updated_at must not move when only A's event is updated",
			);
			Ok(())
		})?;
	}
}

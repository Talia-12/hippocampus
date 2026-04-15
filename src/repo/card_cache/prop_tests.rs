//! Tests covering the cache-ensure / cache-write invariants. The public
//! entry points live in `card_repo` (`get_card`, `list_cards`,
//! `list_cards_by_item`); the helpers being exercised here
//! (`ensure_and_read_card`, `ensure_list_cards_cache`, `update_cards_cache`)
//! are crate-private — we reach them via `super::*` because this module is
//! a child of `card_cache`.

use super::*;
use crate::models::{CardEventFnName, OrderIndex};
use crate::repo::card_repo::get_card_raw;
use crate::repo::query_repo;
use crate::repo::{self, create_card_fetched_event, tests::setup_test_db, update_item};
use crate::schema::cards;
use crate::test_utils::{SetupCardParams, arb_setup_card_params, setup_card};
use diesel::prelude::{ExpressionMethods, QueryDsl, RunQueryDsl};
use proptest::prelude::*;

/// Bypasses the cache-ensure pass and loads raw cards matching a query —
/// the test needs to observe the state left by an earlier
/// `ensure_list_cards_cache` call without triggering a re-ensure.
fn raw_cards_matching(pool: &crate::db::DbPool, query: &crate::dto::GetQueryDto) -> Vec<crate::models::Card> {
	let conn = &mut pool.get().unwrap();
	cards::table
		.filter(cards::id.eq_any(query_repo::cards_matching(query)))
		.load::<crate::models::Card>(conn)
		.unwrap()
}

/// Helper: register the `test_set_title` event for an item type so that
/// `ensure_*_cache` actually has work to do.
async fn register_set_title(pool: &crate::db::DbPool, item_type_id: &crate::models::ItemTypeId) {
	create_card_fetched_event(
		pool,
		item_type_id,
		OrderIndex(0),
		CardEventFnName("test_set_title".to_owned()),
	)
	.await
	.unwrap();
}

proptest! {
	/// CC1.1 (cache invariant): After fetching a card via the canonical
	/// `repo::get_card` path, either no cache was written (no events
	/// registered) or `cache_updated_at` is >= every input's `updated_at`.
	#[test]
	fn prop_cc1_1_cache_invariant(params in arb_setup_card_params()) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let test_card = setup_card(&pool, params).await;
			register_set_title(&pool, &test_card.item_type.get_id()).await;

			let fetched = repo::get_card(&pool, &test_card.card.get_id())
				.await
				.unwrap()
				.unwrap();

			let item_after = repo::get_item(&pool, &test_card.item.get_id()).unwrap().unwrap();
			let item_type_after = repo::get_item_type(&pool, &test_card.item_type.get_id())
				.unwrap()
				.unwrap();
			let card_after = get_card_raw(&pool, &test_card.card.get_id()).unwrap().unwrap();

			match card_after.get_cache_updated_at_raw() {
				None => {
					prop_assert!(
						card_after.get_card_data().is_none(),
						"card_data should be None when cache_updated_at is None"
					);
					prop_assert!(fetched.get_card_data().is_none());
				}
				Some(cache_ts) => {
					prop_assert!(
						cache_ts >= item_after.get_updated_at_raw(),
						"cache_updated_at < item.updated_at"
					);
					prop_assert!(
						cache_ts >= card_after.get_updated_at_raw(),
						"cache_updated_at < card.updated_at"
					);
					prop_assert!(
						cache_ts >= item_type_after.get_updated_at_raw(),
						"cache_updated_at < item_type.updated_at"
					);
					prop_assert!(fetched.get_card_data().is_some());
				}
			}
			Ok(())
		})?;
	}

	/// CC2.1 (value idempotency): Two fetches with no intervening mutation
	/// must return the same `card_data`. The `cache_updated_at` may advance
	/// (a same-ms recompute is allowed under the conservative staleness
	/// check), but the *value* must be stable.
	#[test]
	fn prop_cc2_1_idempotent_value(params in arb_setup_card_params()) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let test_card = setup_card(&pool, params).await;
			register_set_title(&pool, &test_card.item_type.get_id()).await;

			let first = repo::get_card(&pool, &test_card.card.get_id())
				.await
				.unwrap()
				.unwrap();

			let second = repo::get_card(&pool, &test_card.card.get_id())
				.await
				.unwrap()
				.unwrap();

			prop_assert_eq!(first.get_card_data(), second.get_card_data());
			Ok(())
		})?;
	}

	/// CC2.2 (fast path): Once a cache is fresh, a subsequent `get_card`
	/// does not advance `cache_updated_at`.
	///
	/// The staleness check is intentionally `<=` — a mutation that lands in
	/// the *same* millisecond as a cache write must invalidate (see the
	/// comment on `load_stale_cards`). So this test has to sleep between
	/// the last mutation (`register_set_title`) and the first fetch, and
	/// again between the two fetches, so that `cache_updated_at` is strictly
	/// greater than every `updated_at` input and a re-read is observably
	/// redundant.
	#[test]
	fn prop_cc2_2_fresh_cache_is_not_rewritten(params in arb_setup_card_params()) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let test_card = setup_card(&pool, params).await;
			register_set_title(&pool, &test_card.item_type.get_id()).await;

			// Ensure the first fetch's `now_ms` is strictly later than the
			// last mutation timestamp, otherwise the `<=` check legitimately
			// marks the cache stale.
			tokio::time::sleep(std::time::Duration::from_millis(5)).await;

			let _ = repo::get_card(&pool, &test_card.card.get_id())
				.await
				.unwrap()
				.unwrap();
			let after_first = get_card_raw(&pool, &test_card.card.get_id()).unwrap().unwrap();
			let ts1 = after_first.get_cache_updated_at_raw();

			tokio::time::sleep(std::time::Duration::from_millis(5)).await;

			let _ = repo::get_card(&pool, &test_card.card.get_id())
				.await
				.unwrap()
				.unwrap();
			let after_second = get_card_raw(&pool, &test_card.card.get_id()).unwrap().unwrap();
			let ts2 = after_second.get_cache_updated_at_raw();

			prop_assert_eq!(ts1, ts2, "cache_updated_at advanced on a fresh-cache read");
			Ok(())
		})?;
	}

	/// CC3.1 (precision invariant): After updating an item's title, the
	/// next cache-aware fetch must reflect the *new* title — i.e. the cache
	/// is recognised as stale even though item.updated_at and
	/// cache_updated_at were both written in different layers (trigger vs.
	/// Rust). This is the test that catches ms/ns precision drift.
	#[test]
	fn prop_cc3_1_invalidates_after_item_update(
		params in arb_setup_card_params(),
		new_title in "\\PC+",
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let test_card = setup_card(&pool, params).await;
			prop_assume!(new_title != test_card.item.get_title());

			register_set_title(&pool, &test_card.item_type.get_id()).await;

			// First fetch fills the cache with the original title.
			let first = repo::get_card(&pool, &test_card.card.get_id())
				.await
				.unwrap()
				.unwrap();
			let first_data = first.get_card_data().expect("cache should be filled");
			prop_assert_eq!(&first_data.0["title"], &serde_json::Value::String(test_card.item.get_title()));

			// Mutate the item's title — the SQLite trigger bumps items.updated_at.
			update_item(&pool, &test_card.item.get_id(), Some(new_title.clone()), None)
				.await
				.unwrap();

			// Next fetch must recompute the chain against the new title.
			let second = repo::get_card(&pool, &test_card.card.get_id())
				.await
				.unwrap()
				.unwrap();
			let second_data = second.get_card_data().expect("cache should be filled");
			prop_assert_eq!(&second_data.0["title"], &serde_json::Value::String(new_title));
			Ok(())
		})?;
	}

	/// CC3.2: Adding a new event to the item type changes the chain
	/// composition, and the next fetch must reflect that change. This
	/// exercises the item_type.updated_at branch of the staleness check —
	/// the `update_item_type_on_event_insert` trigger bumps
	/// item_types.updated_at.
	#[test]
	fn prop_cc3_2_invalidates_after_event_added(
		params in arb_setup_card_params(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let test_card = setup_card(&pool, params).await;

			// Initial chain: just test_set_title → produces {"title": ...}.
			register_set_title(&pool, &test_card.item_type.get_id()).await;

			let first = repo::get_card(&pool, &test_card.card.get_id())
				.await
				.unwrap()
				.unwrap();
			let first_data = first.get_card_data().expect("cache populated").0;
			prop_assert!(first_data.get("count").is_none());

			// Add a second event that adds a `count` field.
			create_card_fetched_event(
				&pool,
				&test_card.item_type.get_id(),
				OrderIndex(1),
				CardEventFnName("test_increment".to_owned()),
			)
			.await
			.unwrap();

			// Next fetch must include `count`, proving the chain was re-run
			// against the new event list.
			let second = repo::get_card(&pool, &test_card.card.get_id())
				.await
				.unwrap()
				.unwrap();
			let second_data = second.get_card_data().expect("cache populated").0;
			prop_assert_eq!(&second_data["count"], &serde_json::json!(1));
			Ok(())
		})?;
	}

	/// CC4.1: With no events registered, `get_card` leaves the cache
	/// untouched — `cache_updated_at` stays None and `card_data` stays None.
	#[test]
	fn prop_cc4_1_no_events_no_cache_write(params in arb_setup_card_params()) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let test_card = setup_card(&pool, params).await;

			let fetched = repo::get_card(&pool, &test_card.card.get_id())
				.await
				.unwrap()
				.unwrap();

			prop_assert!(fetched.get_card_data().is_none());
			let card_after = get_card_raw(&pool, &test_card.card.get_id()).unwrap().unwrap();
			prop_assert!(card_after.get_cache_updated_at_raw().is_none());
			Ok(())
		})?;
	}

	/// CC6.1 (scope: item): When `list_cards_by_item` ensures the cache, it
	/// must only recompute caches for cards belonging to that item. Cards
	/// owned by a *different* item with registered events must be left
	/// alone.
	///
	/// We force distinct item-type names; `arb_item_type_name()` can otherwise
	/// generate the same "Test " literal twice and collide on the UNIQUE
	/// constraint.
	#[test]
	fn prop_cc6_1_ensure_scoped_to_item(
		mut params_a in arb_setup_card_params(),
		mut params_b in arb_setup_card_params(),
		salt in "\\PC{1,8}",
	) {
		params_a.item_type_name = format!("Test A-{}", salt);
		params_b.item_type_name = format!("Test B-{}", salt);
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let card_a = setup_card(&pool, params_a).await;
			let card_b = setup_card(&pool, params_b).await;
			register_set_title(&pool, &card_a.item_type.get_id()).await;
			register_set_title(&pool, &card_b.item_type.get_id()).await;

			// Scope the cache-ensure to item A.
			let _ = repo::list_cards_by_item(&pool, &card_a.item.get_id())
				.await
				.unwrap();

			let row_a = get_card_raw(&pool, &card_a.card.get_id()).unwrap().unwrap();
			let row_b = get_card_raw(&pool, &card_b.card.get_id()).unwrap().unwrap();

			prop_assert!(
				row_a.get_cache_updated_at_raw().is_some(),
				"scoped-to-A call should have filled A's cache"
			);
			prop_assert!(
				row_b.get_cache_updated_at_raw().is_none(),
				"scoped-to-A call should not have touched B's cache"
			);
			Ok(())
		})?;
	}

	/// CC6.2 (scope: query): `list_cards` only recomputes caches for the
	/// cards the filter actually returns. Cards outside the filter keep
	/// their previous cache state.
	#[test]
	fn prop_cc6_2_ensure_scoped_to_query(
		mut params_a in arb_setup_card_params(),
		mut params_b in arb_setup_card_params(),
		salt in "\\PC{1,8}",
	) {
		// Force unique item-type names so the UNIQUE constraint on item_types.name
		// doesn't spuriously fail the setup.
		params_a.item_type_name = format!("Test A-{}", salt);
		params_b.item_type_name = format!("Test B-{}", salt);
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let card_a = setup_card(&pool, params_a).await;
			let card_b = setup_card(&pool, params_b).await;
			register_set_title(&pool, &card_a.item_type.get_id()).await;
			register_set_title(&pool, &card_b.item_type.get_id()).await;

			let query = crate::dto::GetQueryDtoBuilder::new()
				.item_type_id(card_a.item_type.get_id())
				.build();
			let _ = repo::list_cards(&pool, &query).await.unwrap();

			let row_a = get_card_raw(&pool, &card_a.card.get_id()).unwrap().unwrap();
			let row_b = get_card_raw(&pool, &card_b.card.get_id()).unwrap().unwrap();

			prop_assert!(row_a.get_cache_updated_at_raw().is_some());
			prop_assert!(row_b.get_cache_updated_at_raw().is_none(), "out-of-scope card touched");
			Ok(())
		})?;
	}
}

/// Single-shot: `ensure_list_cards_cache(All)` populates caches across many
/// cards in one call.
#[tokio::test]
async fn cc5_1_ensure_list_cards_cache_populates_many() {
	let pool = setup_test_db();
	let item_type = repo::create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned())
		.await
		.unwrap();
	register_set_title(&pool, &item_type.get_id()).await;

	let titles: Vec<String> = (0..5).map(|i| format!("title-{i}")).collect();
	for t in &titles {
		repo::create_item(&pool, &item_type.get_id(), t.clone(), serde_json::json!({}))
			.await
			.unwrap();
	}

	ensure_list_cards_cache(&pool, CacheScope::Query(&crate::dto::GetQueryDto::default()))
		.await
		.unwrap();

	let cards = repo::list_all_cards(&pool).unwrap();
	assert!(!cards.is_empty());
	for card in &cards {
		assert!(
			card.get_cache_updated_at_raw().is_some(),
			"card {} cache not populated by batch ensure",
			card.get_id()
		);
		assert!(
			card.get_card_data().is_some(),
			"card {} card_data not populated by batch ensure",
			card.get_id()
		);
	}
}

/// Single-shot: cards whose item type has no events registered are skipped
/// by `ensure_list_cards_cache` — they stay `cache_updated_at = None`.
#[tokio::test]
async fn cc5_2_ensure_list_cards_cache_skips_no_events() {
	let pool = setup_test_db();
	let params = SetupCardParams {
		item_type_name: "Test type no events".to_owned(),
		review_function: "fsrs".to_owned(),
		item_title: "x".to_owned(),
		item_data: serde_json::json!({}),
	};
	let test_card = setup_card(&pool, params).await;

	ensure_list_cards_cache(&pool, CacheScope::Query(&crate::dto::GetQueryDto::default()))
		.await
		.unwrap();

	let card = get_card_raw(&pool, &test_card.card.get_id()).unwrap().unwrap();
	assert!(card.get_cache_updated_at_raw().is_none());
	assert!(card.get_card_data().is_none());
}

/// Mixed-workload: the realistic case where some item types have events
/// registered and some don't. The cache must be populated only for the
/// former set; the latter is left alone.
#[tokio::test]
async fn cc5_3_ensure_list_cards_cache_mixed_item_types() {
	let pool = setup_test_db();

	// Type A: has events.
	let type_a = repo::create_item_type(&pool, "Test A".to_owned(), "fsrs".to_owned())
		.await
		.unwrap();
	register_set_title(&pool, &type_a.get_id()).await;
	let item_a = repo::create_item(&pool, &type_a.get_id(), "title-a".to_owned(), serde_json::json!({}))
		.await
		.unwrap();

	// Type B: no events.
	let type_b = repo::create_item_type(&pool, "Test B".to_owned(), "fsrs".to_owned())
		.await
		.unwrap();
	let item_b = repo::create_item(&pool, &type_b.get_id(), "title-b".to_owned(), serde_json::json!({}))
		.await
		.unwrap();

	ensure_list_cards_cache(&pool, CacheScope::Query(&crate::dto::GetQueryDto::default()))
		.await
		.unwrap();

	let cards_a = raw_cards_matching(
		&pool,
		&crate::dto::GetQueryDtoBuilder::new()
			.item_type_id(type_a.get_id())
			.build(),
	);
	assert!(!cards_a.is_empty());
	for c in &cards_a {
		assert!(c.get_cache_updated_at_raw().is_some(), "type-A cache not filled");
	}

	let cards_b = raw_cards_matching(
		&pool,
		&crate::dto::GetQueryDtoBuilder::new()
			.item_type_id(type_b.get_id())
			.build(),
	);
	assert!(!cards_b.is_empty());
	for c in &cards_b {
		assert!(c.get_cache_updated_at_raw().is_none(), "type-B cache incorrectly filled");
	}

	// Keep `item_a` / `item_b` alive so we can reason about them.
	let _ = (item_a, item_b);
}

/// `update_cards_cache` with an empty input set is a no-op — no round-trip
/// to the DB, no error.
#[tokio::test]
async fn ucc1_1_empty_updates_is_noop() {
	let pool = setup_test_db();
	update_cards_cache(&pool, Vec::new(), crate::time_utils::now_ms())
		.await
		.unwrap();
}

/// `update_cards_cache` writes `card_data` and `cache_updated_at` for every
/// card in the input, leaving other cards untouched. Covers the CASE/WHEN
/// SQL path directly.
#[tokio::test]
async fn ucc1_2_batched_update_writes_all_rows() {
	let pool = setup_test_db();
	let item_type = repo::create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned())
		.await
		.unwrap();

	let mut target_ids = Vec::new();
	for i in 0..7 {
		let item = repo::create_item(
			&pool,
			&item_type.get_id(),
			format!("t-{i}"),
			serde_json::json!({}),
		)
		.await
		.unwrap();
		let cards = repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
		target_ids.push(cards[0].get_id());
	}
	// A distinct card we leave out of the batch, to prove scope.
	let untouched_item = repo::create_item(
		&pool,
		&item_type.get_id(),
		"untouched".to_owned(),
		serde_json::json!({}),
	)
	.await
	.unwrap();
	let untouched_id = repo::get_cards_for_item(&pool, &untouched_item.get_id())
		.unwrap()[0]
		.get_id();

	let now = crate::time_utils::now_ms();
	let updates: Vec<(crate::models::CardId, serde_json::Value)> = target_ids
		.iter()
		.enumerate()
		.map(|(i, id)| (id.clone(), serde_json::json!({ "idx": i })))
		.collect();

	update_cards_cache(&pool, updates, now).await.unwrap();

	for (i, id) in target_ids.iter().enumerate() {
		let card = get_card_raw(&pool, id).unwrap().unwrap();
		assert_eq!(card.get_cache_updated_at_raw(), Some(now));
		assert_eq!(
			card.get_card_data().unwrap().0,
			serde_json::json!({ "idx": i })
		);
	}

	let untouched = get_card_raw(&pool, &untouched_id).unwrap().unwrap();
	assert!(untouched.get_cache_updated_at_raw().is_none());
	assert!(untouched.get_card_data().is_none());
}

// ---------------------------------------------------------------------------
// Regression unit tests (CLAUDE.md's "Proptest regressions become unit tests"
// rule). Each of these locks in a specific failing input that proptest caught
// in the past, so a regression surfaces with a named, intention-revealing test
// rather than an anonymous proptest case.
// ---------------------------------------------------------------------------

/// Regression for seed `c9bc2e555...` against CC1.1: exotic unicode in the
/// item title and a deeply-nested item_data used to trip either the chain
/// run or the `run_event_chain` serialization path. Pin that this input now
/// survives the full fetch pipeline.
#[tokio::test]
async fn cc1_1_regression_exotic_unicode() {
	let pool = setup_test_db();
	let params = SetupCardParams {
		item_type_name: "Test 🉐\u{1a65}𑂍N".to_owned(),
		review_function: "incremental_queue".to_owned(),
		item_title: "𐡃{?".to_owned(),
		item_data: serde_json::json!([[
			"nested",
			null,
			true,
			null,
			false,
			"more-nested",
			-8.555576768201634e-34_f64
		]]),
	};
	let tc = setup_card(&pool, params).await;
	register_set_title(&pool, &tc.item_type.get_id()).await;

	let fetched = repo::get_card(&pool, &tc.card.get_id())
		.await
		.unwrap()
		.unwrap();

	// The chain ran at least once: the cache must be populated.
	assert!(fetched.get_card_data().is_some());
}

/// Regression for seed `28dbf7ab55...` against CC3.1: updating an item's
/// title with another exotic-unicode string must flip the cached title on
/// the next fetch.
#[tokio::test]
async fn cc3_1_regression_title_swap_unicode() {
	let pool = setup_test_db();
	let params = SetupCardParams {
		item_type_name: "Test 𛲆=\u{11d90}ƙx🢘?$".to_owned(),
		review_function: "fsrs".to_owned(),
		item_title: "𝍦=%o,U🕴¥'ਣ‖G'𞁩\\Ⱥ𑠑$/h�ਸ਼q{j`<:{/7".to_owned(),
		item_data: serde_json::json!({}),
	};
	let new_title = "𝔼@*%ଢ଼ର:6Q\u{1733}Æ`k¥*xåv𐻂".to_owned();
	let tc = setup_card(&pool, params).await;
	register_set_title(&pool, &tc.item_type.get_id()).await;

	let _ = repo::get_card(&pool, &tc.card.get_id()).await.unwrap();

	update_item(&pool, &tc.item.get_id(), Some(new_title.clone()), None)
		.await
		.unwrap();
	let second = repo::get_card(&pool, &tc.card.get_id())
		.await
		.unwrap()
		.unwrap();

	assert_eq!(
		&second.get_card_data().unwrap().0["title"],
		&serde_json::Value::String(new_title)
	);
}

/// Regression for seed `6ab2219cc6...` against CC6.1: the scoping test used
/// to pass the raw `arb_setup_card_params()` output, which could produce two
/// item-type names collapsing to the same "Test " string and trip the
/// UNIQUE(name) constraint before the actual behaviour was exercised. Pin
/// that the fix (salting the names) covers this specific input.
#[tokio::test]
async fn cc6_1_regression_colliding_type_names() {
	let pool = setup_test_db();
	let salt = "X";
	let params_a = SetupCardParams {
		item_type_name: format!("Test A-{salt}"),
		review_function: "fsrs".to_owned(),
		item_title: "a".to_owned(),
		item_data: serde_json::json!(null),
	};
	let params_b = SetupCardParams {
		item_type_name: format!("Test B-{salt}"),
		review_function: "fsrs".to_owned(),
		item_title: " ".to_owned(),
		item_data: serde_json::json!(null),
	};
	let card_a = setup_card(&pool, params_a).await;
	let card_b = setup_card(&pool, params_b).await;
	register_set_title(&pool, &card_a.item_type.get_id()).await;
	register_set_title(&pool, &card_b.item_type.get_id()).await;

	let _ = repo::list_cards_by_item(&pool, &card_a.item.get_id())
		.await
		.unwrap();

	let row_a = get_card_raw(&pool, &card_a.card.get_id()).unwrap().unwrap();
	let row_b = get_card_raw(&pool, &card_b.card.get_id()).unwrap().unwrap();
	assert!(row_a.get_cache_updated_at_raw().is_some());
	assert!(row_b.get_cache_updated_at_raw().is_none());
}

proptest! {
	/// `update_cards_cache` is a *pure* write-through: whatever (id, value)
	/// pairs the caller supplies end up persisted verbatim. Across an
	/// arbitrary batch (including batches larger than one chunk), every
	/// input row is reflected in the DB.
	#[test]
	fn prop_ucc2_1_batch_write_through(count in 1usize..=250) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type = repo::create_item_type(&pool, "Test type".to_owned(), "fsrs".to_owned())
				.await
				.unwrap();
			let mut rows = Vec::with_capacity(count);
			for i in 0..count {
				let item = repo::create_item(
					&pool,
					&item_type.get_id(),
					format!("t-{i}"),
					serde_json::json!({}),
				)
				.await
				.unwrap();
				let cards = repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
				rows.push(cards[0].get_id());
			}
			let now = crate::time_utils::now_ms();
			let updates: Vec<(crate::models::CardId, serde_json::Value)> = rows
				.iter()
				.enumerate()
				.map(|(i, id)| (id.clone(), serde_json::json!({ "n": i as i64 })))
				.collect();

			update_cards_cache(&pool, updates, now).await.unwrap();

			for (i, id) in rows.iter().enumerate() {
				let card = get_card_raw(&pool, id).unwrap().unwrap();
				prop_assert_eq!(card.get_cache_updated_at_raw(), Some(now));
				prop_assert_eq!(
					card.get_card_data().unwrap().0,
					serde_json::json!({ "n": i as i64 })
				);
			}
			Ok(())
		})?;
	}
}

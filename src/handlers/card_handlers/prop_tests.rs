use super::*;
use crate::repo;
use crate::schema::{cards, metadata};
use crate::test_utils::*;
use diesel::prelude::*;
use proptest::prelude::*;
use serde_json::json;

// ============================================================================
// Daily-ensure integration proptests
//
// These pin that each read handler transitively drives both
// `ensure_offsets_current` and `ensure_sort_positions_cleared` (via
// `ensure_daily_state_current`) on the first request of the day, via
// the repo layer. The handlers themselves no longer call the ensures
// directly; the invariant now lives inside
// `repo::{get_card, list_cards, list_cards_by_item}`. Remove the
// ensure preamble from any of those repo functions and the matching
// assertion here fires. The setup seeds a card with an
// out-of-band `priority_offset` (no value the regen produces in
// [-0.05, 0.05] can fall outside that band, so the sentinel is a
// reliable "did not regen" witness) and a non-zero `sort_position`,
// then rewinds both staleness markers to a date in the distant past. A
// working handler call brings all of that back to the post-ensure
// invariant.
// ============================================================================

/// Offsets strictly outside the regen band [-0.05, 0.05]. Integer-step
/// generation keeps values exactly representable and proptest-shrink stable.
fn arb_out_of_band_offset() -> impl Strategy<Value = f32> {
	prop_oneof![
		(-2000i32..=-51i32).prop_map(|v| v as f32 / 1000.0),
		(51i32..=2000i32).prop_map(|v| v as f32 / 1000.0),
	]
}

/// Any non-zero sort_position — the daily clear must flatten it to 0.0.
fn arb_nonzero_sort_position() -> impl Strategy<Value = f32> {
	prop_oneof![
		(-1000i32..=-1i32).prop_map(|v| v as f32),
		(1i32..=1000i32).prop_map(|v| v as f32),
	]
}

fn seed_stale_daily_state(pool: &Arc<DbPool>, card_id: &CardId, offset: f32, sort_pos: f32) {
	let conn = &mut pool.get().unwrap();
	diesel::update(cards::table.find(card_id))
		.set((
			cards::priority_offset.eq(offset),
			cards::sort_position.eq(sort_pos),
		))
		.execute(conn)
		.unwrap();
	for key in ["last_offset_date", "last_sort_clear_date"] {
		diesel::replace_into(metadata::table)
			.values((metadata::key.eq(key), metadata::value.eq("2020-01-01")))
			.execute(conn)
			.unwrap();
	}
}

fn assert_daily_ensures_fired(pool: &Arc<DbPool>, card_id: &CardId) -> Result<(), TestCaseError> {
	let card = repo::get_card_raw(pool, card_id).unwrap().unwrap();
	let off = card.get_priority_offset();
	prop_assert!(
		off >= -0.05 && off <= 0.05,
		"priority_offset {} still out of band — daily offset regen did not fire",
		off,
	);
	prop_assert_eq!(
		card.get_sort_position(),
		0.0,
		"sort_position still {} — daily sort-position clear did not fire",
		card.get_sort_position(),
	);

	let conn = &mut pool.get().unwrap();
	let today = chrono::Utc::now().date_naive().to_string();
	for key in ["last_offset_date", "last_sort_clear_date"] {
		let last_date: String = metadata::table
			.find(key)
			.select(metadata::value)
			.first::<String>(conn)
			.unwrap();
		prop_assert_eq!(
			last_date,
			today.clone(),
			"metadata {} not bumped to today",
			key
		);
	}
	Ok(())
}

async fn make_item_and_card(pool: &Arc<DbPool>) -> (ItemId, CardId) {
	let item_type = repo::create_item_type(pool, "Test Type 1".to_string(), "fsrs".to_string())
		.await
		.unwrap();
	let item = repo::create_item(
		pool,
		&item_type.get_id(),
		"Test Item".to_string(),
		json!({"front": "Hello", "back": "World"}),
	)
	.await
	.unwrap();
	let card = repo::create_card(pool, &item.get_id(), 3, 0.5)
		.await
		.unwrap();
	(item.get_id(), card.get_id())
}

proptest! {
	#[test]
	fn prop_get_card_handler_fires_daily_ensures(
		offset in arb_out_of_band_offset(),
		sort_pos in arb_nonzero_sort_position(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let (_item_id, card_id) = make_item_and_card(&pool).await;

			seed_stale_daily_state(&pool, &card_id, offset, sort_pos);

			let _ = get_card_handler(
				State(pool.clone()),
				Path(card_id.clone()),
				Query(GetQueryDto::default()),
			)
			.await
			.unwrap();

			assert_daily_ensures_fired(&pool, &card_id)?;
			Ok::<_, TestCaseError>(())
		})?;
	}

	#[test]
	fn prop_list_cards_handler_fires_daily_ensures(
		offset in arb_out_of_band_offset(),
		sort_pos in arb_nonzero_sort_position(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let (_item_id, card_id) = make_item_and_card(&pool).await;

			seed_stale_daily_state(&pool, &card_id, offset, sort_pos);

			let _ = list_cards_handler(State(pool.clone()), Query(GetQueryDto::default()))
				.await
				.unwrap();

			assert_daily_ensures_fired(&pool, &card_id)?;
			Ok::<_, TestCaseError>(())
		})?;
	}

	#[test]
	fn prop_list_cards_by_item_handler_fires_daily_ensures(
		offset in arb_out_of_band_offset(),
		sort_pos in arb_nonzero_sort_position(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let (item_id, card_id) = make_item_and_card(&pool).await;

			seed_stale_daily_state(&pool, &card_id, offset, sort_pos);

			let _ = list_cards_by_item_handler(
				State(pool.clone()),
				Path(item_id),
				Query(GetQueryDto::default()),
			)
			.await
			.unwrap();

			assert_daily_ensures_fired(&pool, &card_id)?;
			Ok::<_, TestCaseError>(())
		})?;
	}

	/// Regression: a `set_sort_position_handler(Top)` call on a stale-marker
	/// day must survive the next read-handler's daily clear. Before the
	/// write-path ensures were pushed into the repo layer, the write set a
	/// non-zero position but the subsequent read fired the daily clear and
	/// silently wiped it. With the ensure living inside the repo write, the
	/// clear runs *before* the move sets its new position, so the move
	/// survives — and the read's ensure is a no-op (marker already today).
	#[test]
	fn prop_sort_position_write_before_first_read_survives(
		offset in arb_out_of_band_offset(),
		sort_pos in arb_nonzero_sort_position(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let (_item_id, card_id) = make_item_and_card(&pool).await;

			seed_stale_daily_state(&pool, &card_id, offset, sort_pos);

			// Write handler fires first on this "new" day. With the bug
			// present, this would set a non-zero position that the read
			// below wipes. With the fix, the write's internal ensure
			// clears everything and bumps the marker, then the write
			// sets this card to the top.
			let move_result = set_sort_position_handler(
				State(pool.clone()),
				Path(card_id.clone()),
				Json(SortPositionAction::Top),
			)
			.await
			.unwrap();
			let moved_position = move_result.0["sort_position"].as_f64().unwrap() as f32;
			prop_assert!(
				moved_position > 0.0,
				"move_to_top should set a positive sort_position; got {}",
				moved_position,
			);

			// First read of the day. With the fix, the daily clear is
			// already today (the write did it), so this is a no-op and
			// the card's position survives.
			let _ = list_cards_handler(State(pool.clone()), Query(GetQueryDto::default()))
				.await
				.unwrap();

			let after = repo::get_card_raw(&pool, &card_id).unwrap().unwrap();
			prop_assert_eq!(
				after.get_sort_position(),
				moved_position,
				"sort_position set by the write before the first read of the day \
				 was wiped — write-path daily ensure missing or firing in wrong order",
			);
			Ok::<_, TestCaseError>(())
		})?;
	}
}

use super::test_fns::REGISTERED_NAMES;
use super::*;
use crate::models::{Card, Item, ItemTypeId, JsonValue, OrderIndex};
use chrono::Utc;
use proptest::prelude::*;
use serde_json::json;

/// Builds a (Item, Card) pair with deterministic non-DB content.
fn make_item_and_card(
	item_type_id: ItemTypeId,
	title: String,
	data: serde_json::Value,
) -> (Item, Card) {
	let item = Item::new(item_type_id, title, JsonValue(data));
	let card = Card::new(item.get_id(), 0, Utc::now(), 0.5);
	(item, card)
}

proptest! {
	/// CR1.1: For any chain composed entirely of registered names with no
	/// failing function, `run_event_chain` is deterministic — running it
	/// twice on the same inputs yields equal outputs.
	#[test]
	fn prop_cr1_1_event_chain_deterministic(
		title in "\\PC*",
		data in crate::test_utils::arb_json(),
		names in prop::collection::vec(prop_oneof![Just("test_set_title"), Just("test_increment")], 0..6)
	) {
		let item_type_id = ItemTypeId("test-it".to_owned());
		let (item, card) = make_item_and_card(item_type_id.clone(), title, data);
		let events: Vec<CardFetchedEvent> = names
			.iter()
			.enumerate()
			.map(|(i, n)| CardFetchedEvent::new(item_type_id.clone(), OrderIndex(i as u16), CardEventFnName((*n).to_owned())))
			.collect();

		let r1 = run_event_chain(&events, &item, card.clone());
		let r2 = run_event_chain(&events, &item, card.clone());

		match (r1, r2) {
			(Ok(a), Ok(b)) => prop_assert_eq!(a, b),
			(Err(_), Err(_)) => {}
			_ => prop_assert!(false, "non-deterministic chain result"),
		}
	}

	/// CR2.1: For an arbitrary list of mixed registered/unregistered names,
	/// `FunctionsNotFound` lists exactly the unregistered ones, in the same
	/// order they appeared in the input.
	#[test]
	fn prop_cr2_1_functions_not_found_aggregation(
		names in prop::collection::vec(crate::test_utils::arb_messy_string(), 0..12)
	) {
		let item_type_id = ItemTypeId("test-it".to_owned());
		let item = Item::new(item_type_id.clone(), "t".to_owned(), JsonValue(json!({})));
		let card = Card::new(item.get_id(), 0, Utc::now(), 0.5);

		let names: Vec<CardEventFnName> = names.into_iter().map(CardEventFnName).collect();
		let events: Vec<CardFetchedEvent> = names
			.iter()
			.enumerate()
			.map(|(i, n)| CardFetchedEvent::new(item_type_id.clone(), OrderIndex(i as u16), n.clone()))
			.collect();

		let expected_missing: Vec<CardEventFnName> = names
			.iter()
			.filter(|n| !REGISTERED_NAMES.contains(&n.0.as_str()))
			.cloned()
			.collect();

		let result = run_event_chain(&events, &item, card);

		if expected_missing.is_empty() {
			// Every name is registered. The chain may still succeed or fail
			// (e.g. test_fail), but it must never return FunctionsNotFound.
		  if let Err(CardEventChainError::FunctionsNotFound(_)) = result {
			  prop_assert!(false, "FunctionsNotFound when no missing names");
		  }
		} else {
			match result {
				Err(CardEventChainError::FunctionsNotFound(actual)) => {
					prop_assert_eq!(actual, expected_missing);
				}
				other => prop_assert!(false, "expected FunctionsNotFound, got {:?}", other),
			}
		}
	}

	/// CR3.1: A chain containing `test_fail` propagates `FunctionFailed`
	/// rather than swallowing the error.
	#[test]
	fn prop_cr3_1_function_failed_propagates(
		prefix_len in 0usize..3,
		suffix_len in 0usize..3,
	) {
		let item_type_id = ItemTypeId("test-it".to_owned());
		let item = Item::new(item_type_id.clone(), "t".to_owned(), JsonValue(json!({})));
		let card = Card::new(item.get_id(), 0, Utc::now(), 0.5);

		let mut events = Vec::new();
		let mut idx: u16 = 0;
		for _ in 0..prefix_len {
			events.push(CardFetchedEvent::new(item_type_id.clone(), OrderIndex(idx), CardEventFnName("test_increment".to_owned())));
			idx += 1;
		}
		events.push(CardFetchedEvent::new(item_type_id.clone(), OrderIndex(idx), CardEventFnName("test_fail".to_owned())));
		idx += 1;
		for _ in 0..suffix_len {
			events.push(CardFetchedEvent::new(item_type_id.clone(), OrderIndex(idx), CardEventFnName("test_increment".to_owned())));
			idx += 1;
		}

		match run_event_chain(&events, &item, card) {
			Err(CardEventChainError::FunctionFailed { function_name, .. }) => {
				prop_assert_eq!(function_name.0, "test_fail");
			}
			other => prop_assert!(false, "expected FunctionFailed, got {:?}", other),
		}
	}
}

#[test]
fn cr0_1_empty_chain_yields_none() {
	// An empty listener list is the "no events registered for this item type"
	// signal — distinct from a chain that runs and produces an empty object.
	// Callers rely on `None` to preserve the "no events → no cache write"
	// invariant in `card_cache`.
	let item_type_id = ItemTypeId("test-it".to_owned());
	let item = Item::new(item_type_id.clone(), "t".to_owned(), JsonValue(json!({})));
	let card = Card::new(item.get_id(), 0, Utc::now(), 0.5);

	let result = run_event_chain(&[], &item, card).unwrap();
	assert_eq!(result, None);
}

#[test]
fn cr0_2_test_set_title_uses_item_title() {
	let item_type_id = ItemTypeId("test-it".to_owned());
	let (item, card) = make_item_and_card(item_type_id.clone(), "the-title".to_owned(), json!({}));
	let events = vec![CardFetchedEvent::new(
		item_type_id,
		OrderIndex(0),
		CardEventFnName("test_set_title".to_owned()),
	)];

	let result = run_event_chain(&events, &item, card).unwrap();
	assert_eq!(
		result.expect("non-empty chain returns Some")["title"],
		json!("the-title")
	);
}

/// CR0.4: `run_event_chain` zeroes `priority_offset` before any registered
/// function sees the card. The offset is a daily-shuffle artifact, not an
/// intrinsic card property; leaking it into chain output would make
/// `card_data` unstable across offset regenerations for no semantic reason.
#[test]
fn cr0_4_priority_offset_is_zeroed_before_event_fns_run() {
	let item_type_id = ItemTypeId("test-it".to_owned());
	let (item, mut card) = make_item_and_card(item_type_id.clone(), "t".to_owned(), json!({}));
	// Seed a deliberately non-zero offset so a no-op zeroing would still
	// produce the original value, not 0.0.
	card.set_priority_offset(0.037);

	let events = vec![CardFetchedEvent::new(
		item_type_id,
		OrderIndex(0),
		CardEventFnName("test_expose_priority_offset".to_owned()),
	)];

	let result = run_event_chain(&events, &item, card)
		.unwrap()
		.expect("non-empty chain returns Some");
	assert_eq!(result["observed_offset"], json!(0.0));
}

#[test]
fn cr0_3_non_empty_chain_always_returns_some_on_success() {
	// A non-empty chain that runs to completion must return `Some(_)`.
	// This pins down the boundary between the `None` case (empty listeners)
	// and the `Some(value)` case (chain ran, produced output).
	let item_type_id = ItemTypeId("test-it".to_owned());
	let (item, card) = make_item_and_card(item_type_id.clone(), "t".to_owned(), json!({}));
	let events = vec![CardFetchedEvent::new(
		item_type_id,
		OrderIndex(0),
		CardEventFnName("test_increment".to_owned()),
	)];

	let result = run_event_chain(&events, &item, card).unwrap();
	assert!(
		result.is_some(),
		"non-empty chain must return Some on success"
	);
}

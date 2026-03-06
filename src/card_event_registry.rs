use std::collections::HashMap;
use std::sync::LazyLock;

use crate::models::{Card, CardEventFnName, CardFetchedEvent, Item};

/// Errors returned by individual card event functions.
///
/// Distinct from `CardEventChainError` (which wraps the chain-level concerns
/// like "function not found"). `CardEventError` is the surface a specific
/// function uses to communicate why it could not transform its input, so the
/// chain can attribute failures to concrete causes rather than free-form strings.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CardEventError {
	/// The input data didn't match what the function expected — e.g. a field
	/// missing or the wrong shape. The caller passed a value that, per the
	/// function's contract, it can't process.
	#[error("Invalid input: {0}")]
	InvalidInput(String),

	/// The function ran but couldn't produce a valid output — e.g. an
	/// internal computation failed for reasons other than bad input.
	#[error("Execution failed: {0}")]
	ExecutionFailed(String),
}

/// Type alias for card fetched event functions
///
/// Each function takes the accumulated data, the item, and the card,
/// and returns the transformed data or a structured `CardEventError`.
pub type CardEventFn = fn(serde_json::Value, &Item, &Card) -> Result<serde_json::Value, CardEventError>;

/// Errors that can occur when running the card event chain.
///
/// `Clone` because these errors cross the boundary from the cache layer
/// (owned result) into the HTTP layer (which may need to both log and
/// format the error) — keeping it cloneable means handlers don't have
/// to pick one or the other.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CardEventChainError {
	/// One or more function names in the event chain were not found in the registry.
	///
	/// In practice this means the DB is referencing a function that used to
	/// be registered at boot but isn't any more (e.g. a deployment removed
	/// it while the events row still pointed to it). From the client's
	/// perspective this is a server misconfiguration, not a client error.
	#[error("Card event functions not found in registry: {}", format_names(.0))]
	FunctionsNotFound(Vec<CardEventFnName>),

	/// A registered function returned an error during execution
	#[error("Card event function '{function_name}' failed: {source}")]
	FunctionFailed {
		/// The name of the function that failed
		function_name: CardEventFnName,

		/// The structured error returned by the function
		source: CardEventError,
	},
}

fn format_names(names: &[CardEventFnName]) -> String {
	names
		.iter()
		.map(|n| n.to_string())
		.collect::<Vec<_>>()
		.join(", ")
}

/// Static registry mapping function names to their implementations
static REGISTRY: LazyLock<HashMap<CardEventFnName, CardEventFn>> = LazyLock::new(|| {
	#[allow(unused_mut)]
	let mut map: HashMap<CardEventFnName, CardEventFn> = HashMap::new();
	// Register event functions here as they are implemented, e.g.:
	// map.insert(CardEventFnName("render_cloze".to_owned()), render_cloze);

	#[cfg(any(test, feature = "test"))]
	{
		map.insert(
			CardEventFnName("test_set_title".to_owned()),
			test_fns::test_set_title as CardEventFn,
		);
		map.insert(
			CardEventFnName("test_increment".to_owned()),
			test_fns::test_increment as CardEventFn,
		);
		map.insert(
			CardEventFnName("test_fail".to_owned()),
			test_fns::test_fail as CardEventFn,
		);
		map.insert(
			CardEventFnName("test_expose_priority_offset".to_owned()),
			test_fns::test_expose_priority_offset as CardEventFn,
		);
	}

	map
});

/// Test-only event functions registered into `REGISTRY` under
/// `cfg(test)` (for in-crate unit/proptests) *or* `feature = "test"` (for
/// integration-test binaries that import the crate as a dependency and
/// therefore don't get `#[cfg(test)]`). Never active in release builds.
#[cfg(any(test, feature = "test"))]
pub mod test_fns {
	use super::*;

	/// Names registered by this module — useful for tests that need to
	/// distinguish "registered" vs "missing" function names.
	pub const REGISTERED_NAMES: &[&str] = &[
		"test_set_title",
		"test_increment",
		"test_fail",
		"test_expose_priority_offset",
	];

	/// Sets a `title` field equal to the item's title. Used by cache tests
	/// that need to observe a chain output that changes when the item is updated.
	pub fn test_set_title(
		data: serde_json::Value,
		item: &Item,
		_card: &Card,
	) -> Result<serde_json::Value, CardEventError> {
		let mut obj = data.as_object().cloned().unwrap_or_default();
		obj.insert(
			"title".to_owned(),
			serde_json::Value::String(item.get_title()),
		);
		Ok(serde_json::Value::Object(obj))
	}

	/// Increments a `count` field by 1. Used by determinism tests with
	/// non-trivial chain composition.
	pub fn test_increment(
		data: serde_json::Value,
		_item: &Item,
		_card: &Card,
	) -> Result<serde_json::Value, CardEventError> {
		let count = data
			.as_object()
			.and_then(|o| o.get("count"))
			.and_then(|v| v.as_i64())
			.unwrap_or(0)
			+ 1;
		let mut obj = data.as_object().cloned().unwrap_or_default();
		obj.insert("count".to_owned(), serde_json::Value::from(count));
		Ok(serde_json::Value::Object(obj))
	}

	/// Always returns an error. Used to test `FunctionFailed` propagation.
	pub fn test_fail(
		_data: serde_json::Value,
		_item: &Item,
		_card: &Card,
	) -> Result<serde_json::Value, CardEventError> {
		Err(CardEventError::ExecutionFailed(
			"intentional test failure".to_owned(),
		))
	}

	/// Writes the observed `priority_offset` of the card into the output under
	/// `"observed_offset"`. Pins the invariant that `run_event_chain` zeroes
	/// the offset before any registered function sees it — if the zeroing
	/// ever regresses, this surfaces as a non-zero value in the JSON.
	pub fn test_expose_priority_offset(
		data: serde_json::Value,
		_item: &Item,
		card: &Card,
	) -> Result<serde_json::Value, CardEventError> {
		let mut obj = data.as_object().cloned().unwrap_or_default();
		obj.insert(
			"observed_offset".to_owned(),
			serde_json::Value::from(card.get_priority_offset()),
		);
		Ok(serde_json::Value::Object(obj))
	}
}

/// Looks up a card event function by name
///
/// ### Arguments
///
/// * `name` - The name of the function to look up
///
/// ### Returns
///
/// The function if found, or None if no function is registered with that name
pub fn get_event_fn(name: &CardEventFnName) -> Option<CardEventFn> {
	REGISTRY.get(name).copied()
}

/// Returns true if a function with the given name is registered
pub fn is_registered(name: &CardEventFnName) -> bool {
	REGISTRY.contains_key(name)
}

/// Runs the event chain for a card, applying each registered function in order.
///
/// Starts with an empty JSON object and chains through each function in order.
/// First validates that all function names exist in the registry, then executes them.
///
/// Takes `card` by value (not `&Card`) because we need to zero the
/// `priority_offset` before it's observable to any event function: the
/// offset is a daily-shuffle artifact, not an intrinsic property of the
/// card, and exposing it to chain functions would make their output
/// unstable across days for no semantic reason (and would invalidate the
/// cache on every offset regen). The caller's `Card` is consumed so the
/// zeroed copy can't leak back out.
///
/// ### Arguments
///
/// * `listeners` - The ordered list of card fetched events to apply
/// * `item` - The item the card belongs to
/// * `card` - The card being fetched. Consumed; its `priority_offset` is
///   zeroed before being handed to any registered function.
///
/// ### Returns
///
/// `Ok(Some(value))` — the accumulated JSON value after all functions have been applied.
///
/// `Ok(None)` — `listeners` was empty. Returned as a distinct signal because a card
/// whose item type has no registered events is not the same as a chain that
/// produced an empty object: callers use this to preserve the invariant that
/// "no events → no cache write", so `card_data` stays `NULL` in the DB rather
/// than being overwritten with `{}`.
///
/// `Err(_)` — one of the named functions was missing from the registry, or a
/// registered function returned an error during execution.
pub fn run_event_chain(
	listeners: &[CardFetchedEvent],
	item: &Item,
	mut card: Card,
) -> Result<Option<serde_json::Value>, CardEventChainError> {
	if listeners.is_empty() {
		return Ok(None);
	}

	let mut missing = Vec::new();
	let mut resolved = Vec::with_capacity(listeners.len());

	for event in listeners {
		let name = event.get_function_name();
		match get_event_fn(&name) {
			Some(func) => resolved.push((name, func)),
			None => missing.push(name),
		}
	}

	if !missing.is_empty() {
		return Err(CardEventChainError::FunctionsNotFound(missing));
	}

	// Zero the priority offset so chain functions never observe it. See the
	// fn-level docstring; individual `CardEventFn`s still take `&Card`.
	card.set_priority_offset(0.0);

	// If functions were found for all the item type's registered event names, execute them in order.
	resolved
		.into_iter()
		.try_fold(
			serde_json::Value::Object(Default::default()),
			|data, (listener_name, listener_func)| {
				listener_func(data, item, &card).map_err(|source| {
					CardEventChainError::FunctionFailed {
						function_name: listener_name,
						source,
					}
				})
			},
		)
		.map(Some)
}

#[cfg(test)]
mod prop_tests;

use axum::{
	Json,
	extract::{Path, State},
};
use std::sync::Arc;
use tracing::{debug, info, instrument};

use crate::db::DbPool;
use crate::dto::CreateCardFetchedEventDto;
use crate::errors::ApiError;
use crate::models::{CardEventFnName, CardFetchedEvent, ItemTypeId};
use crate::repo;
use crate::repo::{
	CreateCardFetchedEventError, DeleteCardFetchedEventError, ListEventsForItemTypeError,
};

/// Handler for registering a new card fetched event for an item type
///
/// This function handles POST requests to `/item_types/{item_type_id}/card_fetched_events`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_type_id` - The ID of the item type
/// * `payload` - The request payload containing the event details
///
/// ### Returns
///
/// The newly created card fetched event as JSON
#[instrument(skip(pool), fields(item_type_id = %item_type_id))]
pub async fn create_card_fetched_event_handler(
	State(pool): State<Arc<DbPool>>,
	Path(item_type_id): Path<ItemTypeId>,
	Json(payload): Json<CreateCardFetchedEventDto>,
) -> Result<Json<CardFetchedEvent>, ApiError> {
	info!("Creating new card fetched event");

	// All validation (item type exists, function registered, no duplicate)
	// happens atomically in the repo — via the FK + UNIQUE constraints plus
	// an up-front registry lookup. The handler just translates error kinds
	// into HTTP status codes.
	let event = repo::create_card_fetched_event(
		&pool,
		&item_type_id,
		payload.order_index,
		payload.function_name,
	)
	.await
	.map_err(|e| match e {
		CreateCardFetchedEventError::Duplicate => ApiError::Conflict(e.to_string()),
		CreateCardFetchedEventError::ItemTypeNotFound => ApiError::NotFound,
		CreateCardFetchedEventError::UnknownFunction(name) => ApiError::UnknownCardEventFn(name),
		CreateCardFetchedEventError::Other(err) => ApiError::Database(err),
	})?;

	info!("Successfully created card fetched event");

	Ok(Json(event))
}

/// Handler for listing all card fetched events for an item type
///
/// This function handles GET requests to `/item_types/{item_type_id}/card_fetched_events`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_type_id` - The ID of the item type
///
/// ### Returns
///
/// A list of card fetched events as JSON
#[instrument(skip(pool), fields(item_type_id = %item_type_id))]
pub async fn list_card_fetched_events_handler(
	State(pool): State<Arc<DbPool>>,
	Path(item_type_id): Path<ItemTypeId>,
) -> Result<Json<Vec<CardFetchedEvent>>, ApiError> {
	debug!("Listing card fetched events");

	// `list_events_for_item_type` does the item-type existence check and
	// the events load inside one transaction, so we can distinguish
	// "item type not found" (404) from "item type exists, no events
	// registered" (200 with `[]`) without a TOCTOU window between two
	// separate queries.
	let events = repo::list_events_for_item_type(&pool, &item_type_id).map_err(|e| match e {
		ListEventsForItemTypeError::ItemTypeNotFound => ApiError::NotFound,
		ListEventsForItemTypeError::Other(err) => ApiError::Database(err),
	})?;

	info!("Retrieved {} card fetched events", events.len());

	Ok(Json(events))
}

/// Handler for deleting a card fetched event
///
/// This function handles DELETE requests to `/item_types/{item_type_id}/card_fetched_events/{function_name}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `path` - The item type ID and function name from the URL path
///
/// ### Returns
///
/// An empty successful response
#[instrument(skip(pool), fields(item_type_id = %item_type_id, function_name = %function_name))]
pub async fn delete_card_fetched_event_handler(
	State(pool): State<Arc<DbPool>>,
	Path((item_type_id, function_name)): Path<(ItemTypeId, CardEventFnName)>,
) -> Result<(), ApiError> {
	info!("Deleting card fetched event");

	// Both `ItemTypeNotFound` and `NotFound` surface as HTTP 404 to the
	// client (the resource doesn't exist either way), but we keep them
	// distinct at the repo layer for logging/metrics and to mirror the
	// Create path's error shape.
	repo::delete_card_fetched_event(&pool, &item_type_id, &function_name)
		.await
		.map_err(|e| match e {
			DeleteCardFetchedEventError::ItemTypeNotFound => ApiError::NotFound,
			DeleteCardFetchedEventError::NotFound => ApiError::NotFound,
			DeleteCardFetchedEventError::Other(err) => ApiError::Database(err),
		})?;

	info!("Successfully deleted card fetched event");

	Ok(())
}

#[cfg(test)]
mod tests {
	//! Handler-level tests for the card-fetched-event API. These pin the
	//! HTTP contract: specifically, that the atomicity work done in the
	//! repo surfaces as the right status codes here (404 for unknown item
	//! type, 409 for duplicates, 400 for unknown function name). These
	//! tests are the reason the handler stayed thin — the invariants are
	//! enforced by the repo and we verify the translation.

	use super::*;
	use crate::models::OrderIndex;
	use crate::repo;
	use crate::test_utils::setup_test_db;

	async fn make_item_type(pool: &crate::db::DbPool, name: &str) -> ItemTypeId {
		repo::create_item_type(pool, name.to_owned(), "fsrs".to_owned())
			.await
			.unwrap()
			.get_id()
	}

	#[tokio::test]
	async fn create_returns_200_with_the_event() {
		let pool = setup_test_db();
		let it = make_item_type(&pool, "Test handler 1").await;

		let res = create_card_fetched_event_handler(
			axum::extract::State(pool.clone()),
			axum::extract::Path(it.clone()),
			axum::Json(CreateCardFetchedEventDto {
				order_index: OrderIndex(0),
				function_name: CardEventFnName("test_set_title".to_owned()),
			}),
		)
		.await
		.expect("should succeed");
		assert_eq!(res.0.get_item_type_id(), it);
		assert_eq!(res.0.get_order_index(), OrderIndex(0));
		assert_eq!(
			res.0.get_function_name(),
			CardEventFnName("test_set_title".to_owned())
		);
	}

	#[tokio::test]
	async fn create_unknown_item_type_yields_not_found() {
		let pool = setup_test_db();
		let err = create_card_fetched_event_handler(
			axum::extract::State(pool.clone()),
			axum::extract::Path(ItemTypeId("nope".to_owned())),
			axum::Json(CreateCardFetchedEventDto {
				order_index: OrderIndex(0),
				function_name: CardEventFnName("test_set_title".to_owned()),
			}),
		)
		.await
		.expect_err("should fail");
		assert!(matches!(err, ApiError::NotFound), "got {:?}", err);
	}

	#[tokio::test]
	async fn create_unknown_function_yields_400() {
		let pool = setup_test_db();
		let it = make_item_type(&pool, "Test handler 2").await;
		let err = create_card_fetched_event_handler(
			axum::extract::State(pool.clone()),
			axum::extract::Path(it),
			axum::Json(CreateCardFetchedEventDto {
				order_index: OrderIndex(0),
				function_name: CardEventFnName("not_registered".to_owned()),
			}),
		)
		.await
		.expect_err("should fail");
		assert!(matches!(err, ApiError::UnknownCardEventFn(_)), "got {:?}", err);
	}

	#[tokio::test]
	async fn create_duplicate_yields_conflict() {
		let pool = setup_test_db();
		let it = make_item_type(&pool, "Test handler 3").await;
		let payload = CreateCardFetchedEventDto {
			order_index: OrderIndex(0),
			function_name: CardEventFnName("test_set_title".to_owned()),
		};
		let _ = create_card_fetched_event_handler(
			axum::extract::State(pool.clone()),
			axum::extract::Path(it.clone()),
			axum::Json(CreateCardFetchedEventDto {
				order_index: payload.order_index,
				function_name: payload.function_name.clone(),
			}),
		)
		.await
		.unwrap();

		let err = create_card_fetched_event_handler(
			axum::extract::State(pool.clone()),
			axum::extract::Path(it),
			axum::Json(payload),
		)
		.await
		.expect_err("should fail");
		assert!(matches!(err, ApiError::Conflict(_)), "got {:?}", err);
	}

	#[tokio::test]
	async fn list_for_unknown_item_type_yields_not_found() {
		let pool = setup_test_db();
		let err = list_card_fetched_events_handler(
			axum::extract::State(pool.clone()),
			axum::extract::Path(ItemTypeId("nope".to_owned())),
		)
		.await
		.expect_err("should 404 when the item type does not exist");
		assert!(matches!(err, ApiError::NotFound), "got {:?}", err);
	}

	#[tokio::test]
	async fn delete_nonexistent_yields_not_found() {
		let pool = setup_test_db();
		let it = make_item_type(&pool, "Test handler 4").await;
		let err = delete_card_fetched_event_handler(
			axum::extract::State(pool.clone()),
			axum::extract::Path((it, CardEventFnName("never-registered".to_owned()))),
		)
		.await
		.expect_err("should fail");
		assert!(matches!(err, ApiError::NotFound), "got {:?}", err);
	}

	/// Wire-format test: `order_index` must deserialize as a non-negative
	/// integer. Negative or out-of-u16-range values must fail at the JSON
	/// boundary, not silently saturate.
	#[test]
	fn dto_rejects_negative_order_index() {
		let err = serde_json::from_str::<CreateCardFetchedEventDto>(
			r#"{"order_index": -1, "function_name": "test_set_title"}"#,
		)
		.expect_err("negative order_index must fail");
		let msg = err.to_string();
		assert!(msg.contains("invalid") || msg.contains("out of range"), "got: {msg}");
	}

	#[test]
	fn dto_rejects_over_u16_order_index() {
		let err = serde_json::from_str::<CreateCardFetchedEventDto>(
			r#"{"order_index": 70000, "function_name": "test_set_title"}"#,
		)
		.expect_err("too-large order_index must fail");
		let msg = err.to_string();
		assert!(msg.contains("invalid") || msg.contains("out of range"), "got: {msg}");
	}
}

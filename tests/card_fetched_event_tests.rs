//! HTTP-level integration tests for card-fetched-event endpoints.
//!
//! Covers routing + wire-format contracts that the handler unit tests can't
//! exercise. Gated behind `required-features = ["test"]` in Cargo.toml so
//! that the test-only event functions (`test_set_title`, etc.) are
//! registered in this integration-test binary — without the feature the
//! registry is empty and every `POST` would trivially 400 on "unknown
//! function", defeating the point of the test.
//!
//! Run with: `cargo test --features test --test card_fetched_event_tests`

use axum::{
	body::{Body, to_bytes},
	http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::Service;

mod common;
use common::*;

async fn post_event(
	app: &mut axum::Router,
	item_type_id: &str,
	body: Value,
) -> (StatusCode, Value) {
	let request = Request::builder()
		.uri(format!("/item_types/{}/card_fetched_events", item_type_id))
		.method("POST")
		.header("Content-Type", "application/json")
		.body(Body::from(serde_json::to_string(&body).unwrap()))
		.unwrap();
	let response = app.call(request).await.unwrap();
	let status = response.status();
	let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
	(status, value)
}

async fn list_events(app: &mut axum::Router, item_type_id: &str) -> (StatusCode, Value) {
	let request = Request::builder()
		.uri(format!("/item_types/{}/card_fetched_events", item_type_id))
		.method("GET")
		.body(Body::empty())
		.unwrap();
	let response = app.call(request).await.unwrap();
	let status = response.status();
	let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
	(status, value)
}

async fn delete_event(
	app: &mut axum::Router,
	item_type_id: &str,
	function_name: &str,
) -> StatusCode {
	let request = Request::builder()
		.uri(format!(
			"/item_types/{}/card_fetched_events/{}",
			item_type_id, function_name
		))
		.method("DELETE")
		.body(Body::empty())
		.unwrap();
	app.call(request).await.unwrap().status()
}

#[tokio::test]
async fn register_list_delete_roundtrip() {
	let mut app = create_test_app();
	let it = create_item_type(&mut app, "Test roundtrip".to_string()).await;
	let it_id = it.get_id().0.clone();

	let (s1, _) = post_event(
		&mut app,
		&it_id,
		json!({"order_index": 0, "function_name": "test_set_title"}),
	)
	.await;
	assert_eq!(s1, StatusCode::OK);
	let (s2, _) = post_event(
		&mut app,
		&it_id,
		json!({"order_index": 1, "function_name": "test_increment"}),
	)
	.await;
	assert_eq!(s2, StatusCode::OK);

	let (sl, body) = list_events(&mut app, &it_id).await;
	assert_eq!(sl, StatusCode::OK);
	let arr = body.as_array().unwrap();
	assert_eq!(arr.len(), 2);
	assert_eq!(arr[0]["order_index"], json!(0));
	assert_eq!(arr[1]["order_index"], json!(1));

	let sd = delete_event(&mut app, &it_id, "test_set_title").await;
	assert_eq!(sd, StatusCode::OK);
	let (_, body) = list_events(&mut app, &it_id).await;
	assert_eq!(body.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn register_rejects_negative_order_index() {
	// A negative `order_index` can't fit in the `OrderIndex(u16)` newtype,
	// so axum's JSON extractor rejects it before the handler runs. Axum
	// surfaces `serde` parse failures as 422 Unprocessable Entity; we don't
	// particularly care *which* 4xx status it is as long as the request
	// doesn't make it into the DB.
	let mut app = create_test_app();
	let it = create_item_type(&mut app, "Test neg".to_string()).await;
	let (status, _) = post_event(
		&mut app,
		&it.get_id().0,
		json!({"order_index": -1, "function_name": "test_set_title"}),
	)
	.await;
	assert!(
		status.is_client_error(),
		"negative order_index should yield a 4xx, got {}",
		status
	);
}

#[tokio::test]
async fn register_rejects_unknown_function_name() {
	let mut app = create_test_app();
	let it = create_item_type(&mut app, "Test unk".to_string()).await;
	let (status, _) = post_event(
		&mut app,
		&it.get_id().0,
		json!({"order_index": 0, "function_name": "not_a_registered_fn"}),
	)
	.await;
	assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_unknown_item_type_yields_404() {
	let mut app = create_test_app();
	let (status, _) = post_event(
		&mut app,
		"item-type-does-not-exist",
		json!({"order_index": 0, "function_name": "test_set_title"}),
	)
	.await;
	assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn register_duplicate_yields_409() {
	let mut app = create_test_app();
	let it = create_item_type(&mut app, "Test dup".to_string()).await;
	let body = json!({"order_index": 0, "function_name": "test_set_title"});
	let (s1, _) = post_event(&mut app, &it.get_id().0, body.clone()).await;
	assert_eq!(s1, StatusCode::OK);
	let (s2, _) = post_event(&mut app, &it.get_id().0, body).await;
	assert_eq!(s2, StatusCode::CONFLICT);
}

#[tokio::test]
async fn delete_nonexistent_yields_404() {
	let mut app = create_test_app();
	let it = create_item_type(&mut app, "Test del-404".to_string()).await;
	let status = delete_event(&mut app, &it.get_id().0, "never_registered").await;
	assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Deleting against an item type id that doesn't exist at all also yields
/// 404. At the HTTP level this is indistinguishable from "item type exists
/// but has no such event" — both are the same observation "the resource
/// you're trying to delete isn't there" — but the repo layer internally
/// reports `ItemTypeNotFound` vs `NotFound` (see
/// `DeleteCardFetchedEventError`). This test pins the HTTP-level behaviour
/// so a future change to the status code can't quietly slip through.
#[tokio::test]
async fn delete_unknown_item_type_yields_404() {
	let mut app = create_test_app();
	let status = delete_event(&mut app, "item-type-does-not-exist", "test_set_title").await;
	assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Listing against a non-existent item type yields 404, not `200 []`. This
/// is the TOCTOU fix from the review landing at the HTTP boundary: the
/// atomic existence + load inside `list_events_for_item_type` now means a
/// genuinely missing item type can never be misreported as "exists with
/// zero events".
#[tokio::test]
async fn list_unknown_item_type_yields_404() {
	let mut app = create_test_app();
	let (status, _) = list_events(&mut app, "item-type-does-not-exist").await;
	assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Listing against a real item type with no registered events yields
/// `200 []`. Counterpart to `list_unknown_item_type_yields_404` — the
/// distinction that used to live in handler code now lives in the typed
/// repo error, and this test pins that the HTTP contract still honours it.
#[tokio::test]
async fn list_existing_item_type_with_no_events_yields_empty_list() {
	let mut app = create_test_app();
	let it = create_item_type(&mut app, "Test empty-list".to_string()).await;
	let (status, body) = list_events(&mut app, &it.get_id().0).await;
	assert_eq!(status, StatusCode::OK);
	let arr = body.as_array().expect("body should be a JSON array");
	assert!(arr.is_empty(), "expected [], got {:?}", arr);
}

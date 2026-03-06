use super::*;
use axum::body::to_bytes;
use axum::response::IntoResponse;

/// Helper to extract status code and body JSON from an ApiError response
async fn error_response(error: ApiError) -> (StatusCode, serde_json::Value) {
	let response = error.into_response();
	let status = response.status();
	let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
	let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
	(status, json)
}

#[tokio::test]
async fn test_database_error_response() {
	let error = ApiError::Database(anyhow::anyhow!("connection refused"));
	let (status, body) = error_response(error).await;
	assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
	assert_eq!(body["error"], "Internal server error");
}

#[tokio::test]
async fn test_not_found_response() {
	let error = ApiError::NotFound;
	let (status, body) = error_response(error).await;
	assert_eq!(status, StatusCode::NOT_FOUND);
	assert_eq!(body["error"], "Item/Card/etc. not found");
}

#[tokio::test]
async fn test_invalid_rating_response() {
	let msg = "Rating must be between 1 and 4".to_string();
	let error = ApiError::InvalidRating(msg.clone());
	let (status, body) = error_response(error).await;
	assert_eq!(status, StatusCode::BAD_REQUEST);
	assert_eq!(body["error"], msg);
}

#[tokio::test]
async fn test_invalid_priority_response() {
	let msg = "Priority must be between 0 and 1".to_string();
	let error = ApiError::InvalidPriority(msg.clone());
	let (status, body) = error_response(error).await;
	assert_eq!(status, StatusCode::BAD_REQUEST);
	assert_eq!(body["error"], msg);
}

#[tokio::test]
async fn test_invalid_review_function_response() {
	let msg = "Unknown review function: foobar".to_string();
	let error = ApiError::InvalidReviewFunction(msg.clone());
	let (status, body) = error_response(error).await;
	assert_eq!(status, StatusCode::BAD_REQUEST);
	assert_eq!(body["error"], msg);
}

#[tokio::test]
async fn test_method_not_allowed_response() {
	let error = ApiError::MethodNotAllowed;
	let (status, body) = error_response(error).await;
	assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
	assert_eq!(body["error"], "Method not allowed");
}

#[tokio::test]
async fn test_card_event_chain_failed_response() {
	use crate::card_event_registry::CardEventChainError;
	use crate::models::CardEventFnName;

	// Registry/data drift: the DB pointed at a function name that isn't
	// in the in-memory registry. The client sees 500 (this is server-side
	// misconfiguration, not bad input) with the function name embedded so
	// operators can identify which row to fix or remove.
	let chain_err = CardEventChainError::FunctionsNotFound(vec![CardEventFnName(
		"ghost_function".to_owned(),
	)]);
	let error = ApiError::CardEventChainFailed(chain_err);
	let (status, body) = error_response(error).await;
	assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
	let msg = body["error"].as_str().unwrap();
	assert!(
		msg.contains("ghost_function"),
		"response should surface the missing function name; got: {}",
		msg
	);
	assert!(
		msg.starts_with("Card event chain failed:"),
		"response should lead with the chain-failure tag; got: {}",
		msg
	);
}

#[tokio::test]
async fn test_card_event_chain_failed_function_failed_response() {
	use crate::card_event_registry::{CardEventChainError, CardEventError};
	use crate::models::CardEventFnName;

	// The other CardEventChainError variant: a registered function that
	// errored out at runtime. Same status code, different message shape.
	let chain_err = CardEventChainError::FunctionFailed {
		function_name: CardEventFnName("test_fail".to_owned()),
		source: CardEventError::ExecutionFailed("boom".to_owned()),
	};
	let error = ApiError::CardEventChainFailed(chain_err);
	let (status, body) = error_response(error).await;
	assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
	let msg = body["error"].as_str().unwrap();
	assert!(msg.contains("test_fail"));
	assert!(msg.contains("boom"));
}

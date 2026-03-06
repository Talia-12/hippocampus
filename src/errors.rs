use axum::{
	Json,
	http::StatusCode,
	response::{IntoResponse, Response},
};
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::card_event_registry::CardEventChainError;
use crate::models::CardEventFnName;
use crate::repo::CardFetchError;

#[derive(Error, Debug)]
pub enum ApiError {
	#[error("Database error: {0}")]
	Database(#[from] anyhow::Error),
	#[error("Item/Card/etc. not found")]
	NotFound,
	#[error("Invalid rating: {0}")]
	InvalidRating(String),
	#[error("Invalid priority: {0}")]
	InvalidPriority(String),
	#[error("Invalid review function: {0}")]
	InvalidReviewFunction(String),
	#[error("Method not allowed")]
	MethodNotAllowed,
	#[error("Cycle detected: adding this relation would create a cycle")]
	CycleDetected,
	#[error("Conflict: {0}")]
	Conflict(String),
	#[error("Unknown card event function: {0}")]
	UnknownCardEventFn(CardEventFnName),
	/// A card fetch tried to run the event chain and the chain failed —
	/// usually because the DB references a function name that isn't in the
	/// in-memory registry, or a registered function errored out. This is
	/// server-side inconsistency (the DB and the registry disagree) rather
	/// than bad client input.
	#[error("Card event chain failed: {0}")]
	CardEventChainFailed(CardEventChainError),
}

impl IntoResponse for ApiError {
	fn into_response(self) -> Response {
		let (status, message) = match &self {
			ApiError::Database(err) => {
				// Log internal server errors at the error level
				error!(error.message = %err, error.kind = "database_error", "Database error: {}", err);
				(
					StatusCode::INTERNAL_SERVER_ERROR,
					"Internal server error".to_string(),
				)
			}
			ApiError::NotFound => {
				// Not Found errors are just informational
				debug!(error.kind = "not_found", "Resource not found");
				(
					StatusCode::NOT_FOUND,
					"Item/Card/etc. not found".to_string(),
				)
			}
			ApiError::InvalidRating(msg) => {
				// Client errors are logged at warn level
				warn!(error.kind = "invalid_rating", message = %msg, "Invalid rating: {}", msg);
				(StatusCode::BAD_REQUEST, msg.clone())
			}
			ApiError::InvalidPriority(msg) => {
				// Client errors are logged at warn level
				warn!(error.kind = "invalid_priority", message = %msg, "Invalid priority: {}", msg);
				(StatusCode::BAD_REQUEST, msg.clone())
			}
			ApiError::InvalidReviewFunction(msg) => {
				// Client errors are logged at warn level
				warn!(error.kind = "invalid_review_function", message = %msg, "Invalid review function: {}", msg);
				(StatusCode::BAD_REQUEST, msg.clone())
			}
			ApiError::MethodNotAllowed => {
				// Client errors are logged at warn level
				warn!(error.kind = "method_not_allowed", "Method not allowed");
				(
					StatusCode::METHOD_NOT_ALLOWED,
					"Method not allowed".to_string(),
				)
			}
			ApiError::CycleDetected => {
				warn!(
					error.kind = "cycle_detected",
					"Adding this relation would create a cycle"
				);
				(
					StatusCode::CONFLICT,
					"Adding this relation would create a cycle".to_string(),
				)
			}
			ApiError::Conflict(msg) => {
				warn!(error.kind = "conflict", message = %msg, "Conflict: {}", msg);
				(StatusCode::CONFLICT, msg.clone())
			}
			ApiError::UnknownCardEventFn(name) => {
				warn!(error.kind = "unknown_card_event_fn", function_name = %name, "Unknown card event function: {}", name);
				(
					StatusCode::BAD_REQUEST,
					format!("Unknown card event function: {}", name),
				)
			}
			ApiError::CardEventChainFailed(chain_err) => {
				// Log at `error` level because this is server-side
				// misconfiguration, not bad client input — operators need
				// to know when the DB and the registry have drifted apart.
				error!(error.kind = "card_event_chain_failed", error.message = %chain_err, "Card event chain failed: {}", chain_err);
				(
					StatusCode::INTERNAL_SERVER_ERROR,
					format!("Card event chain failed: {}", chain_err),
				)
			}
		};

		// Log all error responses in a consistent format
		info!(
			response.status = %status.as_u16(),
			error.message = %message,
			"Returning error response"
		);

		let body = Json(serde_json::json!({
			"error": message
		}));

		(status, body).into_response()
	}
}

/// `?`-friendly conversion for handlers that call `repo::get_card` /
/// `repo::list_cards` / `repo::list_cards_by_item`. Keeps the typed
/// distinction between registry/data drift (which needs operator
/// attention) and incidental DB failures (which collapse to 500).
impl From<CardFetchError> for ApiError {
	fn from(e: CardFetchError) -> Self {
		match e {
			CardFetchError::EventChain(ch) => ApiError::CardEventChainFailed(ch),
			CardFetchError::Other(ae) => ApiError::Database(ae),
		}
	}
}

#[cfg(test)]
mod tests;

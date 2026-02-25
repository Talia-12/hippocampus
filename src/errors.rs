use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
    Json
};
use thiserror::Error;
use tracing::{error, warn, info, debug};

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
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::Database(err) => {
                // Log internal server errors at the error level
                error!(error.message = %err, error.kind = "database_error", "Database error: {}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            },
            ApiError::NotFound => {
                // Not Found errors are just informational
                debug!(error.kind = "not_found", "Resource not found");
                (StatusCode::NOT_FOUND, "Item/Card/etc. not found".to_string())
            },
            ApiError::InvalidRating(msg) => {
                // Client errors are logged at warn level
                warn!(error.kind = "invalid_rating", message = %msg, "Invalid rating: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            },
            ApiError::InvalidPriority(msg) => {
                // Client errors are logged at warn level
                warn!(error.kind = "invalid_priority", message = %msg, "Invalid priority: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            },
            ApiError::InvalidReviewFunction(msg) => {
                // Client errors are logged at warn level
                warn!(error.kind = "invalid_review_function", message = %msg, "Invalid review function: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            },
            ApiError::MethodNotAllowed => {
                // Client errors are logged at warn level
                warn!(error.kind = "method_not_allowed", "Method not allowed");
                (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed".to_string())
            },
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
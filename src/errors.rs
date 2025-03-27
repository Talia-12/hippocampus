use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
    Json
};
use thiserror::Error;

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
    #[error("Method not allowed")]
    MethodNotAllowed,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Database(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "Item/Card/etc. not found".to_string()),
            ApiError::InvalidRating(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::InvalidPriority(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed".to_string()),
        };

        let body = Json(serde_json::json!({
            "error": message
        }));

        (status, body).into_response()
    }
} 
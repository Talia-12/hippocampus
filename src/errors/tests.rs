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

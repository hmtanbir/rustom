use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Centralized application error enum mapping domain errors to HTTP status codes.
#[derive(Error, Debug)]
pub enum AppError {
    /// Database access errors via SQLx.
    #[error("Internal database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Caching system errors.
    #[error("Cache error: {0}")]
    Cache(String),

    /// Message broker errors via lapin.
    #[error("Message queue error: {0}")]
    Queue(#[from] lapin::Error),

    /// Authentication failure errors.
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Authorization policy failures.
    #[error("Authorization failed: {0}")]
    Authorization(String),

    /// Bad request input validation failures.
    #[error("Invalid request input: {0}")]
    InvalidInput(String),

    /// Resource not found errors.
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Conflicts, e.g. record with duplicate unique key already exists.
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Catch-all handler for unexpected, runtime errors.
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::Database(err) => {
                tracing::error!("Database error occurred: {:?}", err);
                // Return generic message for internal errors to hide DB details from client
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal database error".to_string())
            }
            AppError::Cache(err) => {
                tracing::error!("Cache error occurred: {}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal cache error".to_string())
            }
            AppError::Queue(err) => {
                tracing::error!("Queue error occurred: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal queue error".to_string())
            }
            AppError::Authentication(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Authorization(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::Unexpected(err) => {
                tracing::error!("Unexpected application error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "An unexpected error occurred".to_string())
            }
        };

        let body = Json(json!({
            "success": false,
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

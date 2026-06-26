use crate::services::slack_notification::SlackNotification;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

/// Centralized application error enum mapping domain errors to HTTP status codes.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Internal database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Message queue error: {0}")]
    Queue(#[from] lapin::Error),

    #[error("API Gateway authentication failed: {0}")]
    GatewayAuth(String),

    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Authorization failed: {0}")]
    Authorization(String),

    #[error("Invalid request input: {0}")]
    InvalidInput(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Validation failed")]
    Validation(std::collections::HashMap<String, Vec<String>>),

    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message, data) = match &self {
            AppError::Database(err) => {
                tracing::error!("Database error occurred: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error".to_string(),
                    serde_json::Value::Null,
                )
            }
            AppError::Cache(err) => {
                tracing::error!("Cache error occurred: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error".to_string(),
                    serde_json::Value::Null,
                )
            }
            AppError::Queue(err) => {
                tracing::error!("Queue error occurred: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error".to_string(),
                    serde_json::Value::Null,
                )
            }
            AppError::GatewayAuth(msg) => (
                StatusCode::UNAUTHORIZED,
                msg.clone(),
                serde_json::Value::Null,
            ),
            AppError::Authentication(msg) => (
                StatusCode::UNAUTHORIZED,
                msg.clone(),
                serde_json::Value::Null,
            ),
            AppError::Authorization(msg) => {
                (StatusCode::FORBIDDEN, msg.clone(), serde_json::Value::Null)
            }
            AppError::InvalidInput(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                msg.clone(),
                serde_json::Value::Null,
            ),
            AppError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, msg.clone(), serde_json::Value::Null)
            }
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone(), serde_json::Value::Null),
            AppError::Validation(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Validation failed".to_string(),
                serde_json::to_value(errors).unwrap_or(serde_json::Value::Null),
            ),
            AppError::Unexpected(err) => {
                tracing::error!("Unexpected application error: {:?}", err);
                let msg = err.to_string();

                // Spawn a background task to send Slack notification
                let slack_msg = format!("[Error] Exception occurred\nMessage: {}\n", msg);
                tokio::spawn(async move {
                    let _ = SlackNotification::notify_error(&slack_msg).await;
                });

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error".to_string(),
                    serde_json::Value::Null,
                )
            }
        };

        let body = Json(json!({
            "status": status.as_u16(),
            "message": error_message,
            "data": if data.is_null() { serde_json::Value::Null } else { data }
        }));

        (status, body).into_response()
    }
}

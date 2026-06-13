use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};

use crate::app_state::AppState;
use crate::errors::AppError;
use crate::models::{UserRegisterRequestDto, UserPayloadWrapper};
use crate::services::SlackNotification;
use crate::extractors::AppJson;

pub async fn registration(
    State(state): State<AppState>,
    AppJson(payload_wrapper): AppJson<UserPayloadWrapper<UserRegisterRequestDto>>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let payload = payload_wrapper.into_inner();

    if payload.email.trim().is_empty() || payload.password.trim().is_empty() || payload.name.trim().is_empty() {
        return Err(AppError::InvalidInput("Name, email and password are required".to_string()));
    }

    let user_dto = state.user_service.register(payload).await?;

    // Send Slack notification for user registration asynchronously
    let slack_message = format!("New user registered: {} ({})", user_dto.name, user_dto.email);
    tokio::spawn(async move {
        let _ = SlackNotification::notify_registration(&slack_message).await;
    });

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": StatusCode::CREATED.as_u16(),
            "message": "Successfully data created",
            "data": user_dto
        })),
    ))
}

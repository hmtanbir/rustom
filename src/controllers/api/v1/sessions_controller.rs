use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};

use crate::app_state::AppState;
use crate::errors::AppError;
use crate::models::{UserLoginRequestDto, UserPayloadWrapper};

pub async fn create(
    State(state): State<AppState>,
    Json(payload_wrapper): Json<UserPayloadWrapper<UserLoginRequestDto>>,
) -> Result<Json<Value>, AppError> {
    let payload = payload_wrapper.into_inner();

    if payload.email.trim().is_empty() || payload.password.trim().is_empty() {
        return Err(AppError::InvalidInput("invalid email or password".to_string()));
    }

    // Attempt login via service
    let login_res = state.user_service.login(payload).await?;

    Ok(Json(json!({
        "status": StatusCode::OK.as_u16(),
        "message": "Successfully data fetched",
        "data": {
            "token": login_res.token
        }
    })))
}

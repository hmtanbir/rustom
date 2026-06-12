use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};

use crate::domain::{
    AppError, UserLoginRequestDto, UserLoginResponseDto,
    UserRegisterRequestDto, UserResponseDto,
};
use crate::api::AppState;

/// Register a new user.
///
/// Hashes the password with Argon2id and stores user credentials in PostgreSQL.
#[utoipa::path(
    post,
    path = "/api/auth/register",
    request_body = UserRegisterRequestDto,
    responses(
        (status = 201, description = "User registered successfully", body = UserResponseDto),
        (status = 400, description = "Invalid request input"),
        (status = 499, description = "Email already exists (Conflict)"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn register_handler(
    State(state): State<AppState>,
    Json(payload): Json<UserRegisterRequestDto>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    // Basic email/password check
    if payload.email.trim().is_empty() || payload.password.trim().is_empty() {
        return Err(AppError::InvalidInput("Email and password cannot be empty".to_string()));
    }

    let user_dto = state.user_service.register(payload).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "data": user_dto
        })),
    ))
}

/// Login with existing user credentials.
///
/// Validates password hash and returns a signed JWT authentication bearer token.
#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = UserLoginRequestDto,
    responses(
        (status = 200, description = "User logged in successfully", body = UserLoginResponseDto),
        (status = 401, description = "Invalid email or password"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<UserLoginRequestDto>,
) -> Result<Json<Value>, AppError> {
    if payload.email.trim().is_empty() || payload.password.trim().is_empty() {
        return Err(AppError::InvalidInput("Email and password are required".to_string()));
    }

    let login_res = state.user_service.login(payload).await?;

    Ok(Json(json!({
        "success": true,
        "data": login_res
    })))
}

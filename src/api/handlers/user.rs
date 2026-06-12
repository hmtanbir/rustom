use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};
use utoipa::ToSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::{AppError, UserResponseDto, UserRole};
use crate::api::middleware::AuthenticatedUser;
use crate::api::AppState;

/// Request payload to update user role.
#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct UpdateRoleRequestDto {
    /// Target role to set for the user.
    pub role: UserRole,
}

/// Retrieve authenticated user profile.
///
/// Looks up profile details in the Redis cache first, falling back to PostgreSQL if missing.
#[utoipa::path(
    get,
    path = "/api/users/profile",
    responses(
        (status = 200, description = "Profile details retrieved", body = UserResponseDto),
        (status = 401, description = "Missing or invalid authorization token"),
        (status = 404, description = "User profile not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_profile_handler(
    State(state): State<AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
) -> Result<Json<Value>, AppError> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|e| {
        AppError::Authentication(format!("Invalid token subject UUID format: {}", e))
    })?;

    let user_dto = state.user_service.get_profile(user_id).await?;

    Ok(Json(json!({
        "success": true,
        "data": user_dto
    })))
}

/// Update role of a specific user.
///
/// Modifies user role in PostgreSQL and invalidates the corresponding profile cache entry.
/// Accessible by Admins only.
#[utoipa::path(
    put,
    path = "/api/users/{id}/role",
    request_body = UpdateRoleRequestDto,
    params(
        ("id" = Uuid, Path, description = "UUID of the target user to update")
    ),
    responses(
        (status = 200, description = "User role updated successfully", body = UserResponseDto),
        (status = 401, description = "Missing or invalid authorization token"),
        (status = 403, description = "Access forbidden. Requires admin credentials"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn update_role_handler(
    State(state): State<AppState>,
    Path(target_user_id): Path<Uuid>,
    Json(payload): Json<UpdateRoleRequestDto>,
) -> Result<Json<Value>, AppError> {
    let user_dto = state
        .user_service
        .update_role(target_user_id, payload.role)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": user_dto
    })))
}

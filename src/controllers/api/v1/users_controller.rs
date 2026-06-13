use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::errors::AppError;
use crate::middleware::AuthenticatedUser;
use crate::models::{PaginationParams, UserCreateRequestDto, UserUpdateRequestDto, UserPayloadWrapper};
use crate::policies::UserPolicy;
use crate::extractors::AppJson;

#[utoipa::path(get, path = "/api/v1/users", tag = "Users", security(("bearerAuth" = [])))]

pub async fn index(
    State(state): State<AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, AppError> {
    if !UserPolicy::index(&claims) {
        return Err(AppError::Authorization("unauthorized".to_string()));
    }

    let paginated_res = state.user_service.get_users_paginated(params).await?;
    
    Ok(Json(serde_json::to_value(paginated_res).map_err(|e| {
        AppError::Unexpected(anyhow::anyhow!("Failed to serialize paginated response: {}", e))
    })?))
}

pub async fn show(
    State(state): State<AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    if !UserPolicy::show(&claims, id) {
        return Err(AppError::Authorization("unauthorized".to_string()));
    }

    let user_dto = state.user_service.get_user(id).await?;

    Ok(Json(json!({
        "status": StatusCode::OK.as_u16(),
        "message": "Successfully data fetched",
        "data": user_dto
    })))
}

pub async fn me(
    State(state): State<AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
) -> Result<Json<Value>, AppError> {
    let user_dto = state.user_service.get_user(claims.user_id).await?;

    Ok(Json(json!({
        "status": StatusCode::OK.as_u16(),
        "message": "Successfully data fetched",
        "data": user_dto
    })))
}

pub async fn update_me(
    State(state): State<AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    AppJson(payload_wrapper): AppJson<UserPayloadWrapper<UserUpdateRequestDto>>,
) -> Result<Json<Value>, AppError> {
    let mut payload = payload_wrapper.into_inner();

    // Normal users cannot change their own role or status, ensure they are stripped
    if claims.role != 0 {
        payload.role = None;
        payload.status = None;
    }

    let user_dto = state.user_service.update_user(claims.user_id, payload).await?;

    Ok(Json(json!({
        "status": StatusCode::OK.as_u16(),
        "message": "Successfully data fetched",
        "data": user_dto
    })))
}

pub async fn create(
    State(state): State<AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    AppJson(payload_wrapper): AppJson<UserPayloadWrapper<UserCreateRequestDto>>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    if claims.role != 0 {
        return Err(AppError::Authorization("Only admins can create users directly".to_string()));
    }

    let payload = payload_wrapper.into_inner();
    let user_dto = state.user_service.create_user(payload).await?;

    Ok((StatusCode::CREATED, Json(json!({
        "status": StatusCode::CREATED.as_u16(),
        "message": "Successfully data created",
        "data": user_dto
    }))))
}

pub async fn update(
    State(state): State<AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<Uuid>,
    AppJson(payload_wrapper): AppJson<UserPayloadWrapper<UserUpdateRequestDto>>,
) -> Result<Json<Value>, AppError> {
    if !UserPolicy::update(&claims, id) {
        return Err(AppError::Authorization("unauthorized".to_string()));
    }

    let mut payload = payload_wrapper.into_inner();

    // Strip role and status updates if user is not admin
    if claims.role != 0 {
        payload.role = None;
        payload.status = None;
    }

    let user_dto = state.user_service.update_user(id, payload).await?;

    Ok(Json(json!({
        "status": StatusCode::OK.as_u16(),
        "message": "Successfully data updated",
        "data": user_dto
    })))
}

pub async fn destroy(
    State(state): State<AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    if !UserPolicy::destroy(&claims) {
        return Err(AppError::Authorization("unauthorized".to_string()));
    }

    state.user_service.soft_delete_user(id).await?;

    Ok(Json(json!({
        "status": StatusCode::OK.as_u16(),
        "message": "Successfully data deleted",
        "data": serde_json::Value::Null
    })))
}

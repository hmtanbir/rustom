use axum::{extract::State, Json};
use serde_json::{json, Value};
use uuid::Uuid;
use chrono::Utc;

use crate::domain::{AppError, CreateJobRequestDto, CreateJobResponseDto, JobPayload};
use crate::api::middleware::AuthenticatedUser;
use crate::api::AppState;

/// Enqueue a new background task.
///
/// Publishes a job payload to RabbitMQ to be consumed by the background worker.
#[utoipa::path(
    post,
    path = "/api/jobs",
    request_body = CreateJobRequestDto,
    responses(
        (status = 202, description = "Job accepted and enqueued", body = CreateJobResponseDto),
        (status = 400, description = "Invalid request input"),
        (status = 401, description = "Missing or invalid authorization token"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_job_handler(
    State(state): State<AppState>,
    AuthenticatedUser(_claims): AuthenticatedUser,
    Json(payload): Json<CreateJobRequestDto>,
) -> Result<Json<Value>, AppError> {
    if payload.job_type.trim().is_empty() {
        return Err(AppError::InvalidInput("Job type cannot be empty".to_string()));
    }

    let job_id = Uuid::new_v4().to_string();

    let job_payload = JobPayload {
        job_id: job_id.clone(),
        job_type: payload.job_type,
        payload: payload.payload,
        created_at: Utc::now(),
    };

    // Publish to RabbitMQ
    state.queue_publisher.publish_job(&job_payload).await?;

    let response = CreateJobResponseDto {
        job_id,
        success: true,
        message: "Background job enqueued successfully".to_string(),
    };

    Ok(Json(json!({
        "success": true,
        "data": response
    })))
}

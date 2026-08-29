use async_trait::async_trait;
use deadpool_redis::redis::AsyncCommands;
use std::sync::Arc;
use std::time::Duration;

use crate::errors::AppError;
use crate::infrastructure::{JOBS_QUEUE, RedisPool};
use crate::models::JobPayload;

/// Generic messaging queue service trait.
#[async_trait]
pub trait QueueService: Send + Sync {
    /// Publish a background task to the message queue.
    async fn publish_job(&self, job: &JobPayload) -> Result<(), AppError>;
}

/// Redis-backed implementation of the QueueService trait.
#[derive(Clone)]
pub struct RedisQueueService {
    pool: RedisPool,
}

impl RedisQueueService {
    /// Create a new RedisQueueService.
    pub fn new(pool: RedisPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl QueueService for RedisQueueService {
    async fn publish_job(&self, job: &JobPayload) -> Result<(), AppError> {
        tracing::debug!("Publishing job to Redis queue: {:?}", job);

        let payload = serde_json::to_string(job).map_err(|e| {
            AppError::Unexpected(anyhow::anyhow!("Failed to serialize job payload: {}", e))
        })?;

        let mut conn = self.pool.get().await.map_err(|e| {
            AppError::Queue(format!(
                "Failed to acquire connection from Redis pool: {}",
                e
            ))
        })?;

        // Push job to the right of the list (FIFO: LPUSH to enqueue, BRPOP to consume)
        let _: () = conn
            .lpush(JOBS_QUEUE, payload)
            .await
            .map_err(|e| AppError::Queue(format!("Redis LPUSH command failed: {}", e)))?;

        tracing::info!("Job {} published successfully.", job.job_id);
        Ok(())
    }
}

/// Dynamic trait object for QueueService.
pub type DynQueueService = Arc<dyn QueueService>;

/// Spawns a non-blocking Tokio background worker task to consume and process Redis queue messages.
pub fn start_queue_consumer(pool: RedisPool) {
    tokio::spawn(async move {
        tracing::info!("Starting background Redis queue worker consumer...");

        loop {
            let mut conn = match pool.get().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(
                        "Failed to acquire connection from Redis pool for worker: {:?}",
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };

            // BRPOP with a 2-second timeout to allow smooth task interruption and connection refreshment
            let result: Result<Option<(String, String)>, _> = deadpool_redis::redis::cmd("BRPOP")
                .arg(JOBS_QUEUE)
                .arg(2)
                .query_async(&mut *conn)
                .await;

            match result {
                Ok(Some((_queue, data))) => {
                    let payload: JobPayload = match serde_json::from_str(&data) {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::error!("Failed to deserialize received job payload: {:?}", e);
                            continue;
                        }
                    };

                    tracing::info!(
                        "Worker received Job ID: {} [Type: {}]",
                        payload.job_id,
                        payload.job_type
                    );

                    // Execute asynchronous processing based on the job type
                    match process_job(&payload).await {
                        Ok(_) => {
                            tracing::info!("Job {} completed successfully.", payload.job_id);
                        }
                        Err(e) => {
                            tracing::error!("Failed to process job {}: {:?}", payload.job_id, e);
                        }
                    }
                }
                Ok(None) => {
                    // Queue was empty during timeout period, continue waiting
                }
                Err(e) => {
                    tracing::error!("Error popping job from Redis queue: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });
}

/// Processes a single background job payload.
async fn process_job(job: &JobPayload) -> Result<(), AppError> {
    // Mimic database or network processing latency without blocking the main worker thread
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    match job.job_type.as_str() {
        "email" => {
            let email_to = job
                .payload
                .get("to")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let email_body = job
                .payload
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            tracing::info!(
                "Sending email background job -> TO: {}, BODY: {}",
                email_to,
                email_body
            );
            Ok(())
        }
        "data_process" => {
            tracing::info!("Executing data_process job payload: {:?}", job.payload);
            Ok(())
        }
        unknown_type => {
            let error_msg = format!("Unrecognized job type: {}", unknown_type);
            tracing::warn!("{}", error_msg);
            Err(AppError::Unexpected(anyhow::anyhow!(error_msg)))
        }
    }
}

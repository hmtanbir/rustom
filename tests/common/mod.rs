use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

use rustom::app_state::AppState;
use rustom::config::AppConfig;
use rustom::errors::AppError;
use rustom::models::job::JobPayload;
use rustom::services::{DynCacheService, DynQueueService, QueueService, UserService};
use rustom::router::create_router;

pub struct MockCache;

#[async_trait::async_trait]
impl rustom::services::cache::CacheService for MockCache {
    async fn get(&self, _key: &str) -> Result<Option<String>, AppError> {
        Ok(None)
    }
    async fn set(&self, _key: &str, _value: &str, _ttl_seconds: u64) -> Result<(), AppError> {
        Ok(())
    }
    async fn delete(&self, _key: &str) -> Result<(), AppError> {
        Ok(())
    }
}

pub struct MockQueue;

#[async_trait::async_trait]
impl QueueService for MockQueue {
    async fn publish_job(&self, _job: &JobPayload) -> Result<(), AppError> {
        Ok(())
    }
}

pub async fn setup_app() -> (Router, PgPool) {
    
    // Disable encryption for tests to verify plain JSON outputs
    unsafe {
        std::env::set_var("API_PAYLOAD_ENCRYPTION_ENABLED", "false");
    }
    
    let config = AppConfig {
        host: "127.0.0.1".to_string(),
        port: 3000,
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rustom_development".to_string()),
        redis_url: "redis://127.0.0.1:6379/0".to_string(),
        rabbitmq_url: "amqp://127.0.0.1:5672/%2f".to_string(),
        jwt_secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret_for_tests_123456789".to_string()),
        jwt_expiration_seconds: 3600,
    };

    let db = PgPool::connect(&config.database_url)
        .await
        .expect("Failed to connect to test DB");
    
    let cache_service = Arc::new(MockCache) as DynCacheService;
    let queue_publisher = Arc::new(MockQueue) as DynQueueService;
    let user_service = UserService::new(db.clone(), cache_service, config.clone());

    let state = AppState {
        user_service,
        queue_publisher,
        config,
    };

    (create_router(state), db)
}

pub fn get_gateway_key() -> String {
    std::env::var("API_GATEWAY_KEY").unwrap_or_default()
}

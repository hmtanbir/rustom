use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tower::ServiceExt;
use sqlx::PgPool;

use rustom::api::{create_router, AppState};
use rustom::config::AppConfig;
use rustom::domain::{AppError, JobPayload};
use rustom::services::{DynCacheService, DynQueueService, QueueService, UserService};

struct MockCache;

#[async_trait::async_trait]
impl rustom::services::cache::CacheService for MockCache {
    async fn get<T: serde::de::DeserializeOwned + Send + 'static>(&self, _key: &str) -> Result<Option<T>, AppError> {
        Ok(None)
    }
    async fn set<T: serde::Serialize + Send + Sync + 'static>(&self, _key: &str, _value: &T, _ttl_seconds: u64) -> Result<(), AppError> {
        Ok(())
    }
    async fn delete(&self, _key: &str) -> Result<(), AppError> {
        Ok(())
    }
}

struct MockQueue;

#[async_trait::async_trait]
impl QueueService for MockQueue {
    async fn publish_job(&self, _job: &JobPayload) -> Result<(), AppError> {
        Ok(())
    }
}

#[tokio::test]
async fn test_swagger_docs_endpoint() {
    let config = AppConfig {
        host: "0.0.0.0".to_string(),
        port: 8080,
        database_url: "postgres://localhost/dummy_db".to_string(),
        redis_url: "redis://localhost/dummy_redis".to_string(),
        rabbitmq_url: "amqp://localhost/dummy_rabbit".to_string(),
        jwt_secret: "super_secret_jwt_key_that_is_long_enough_to_be_secure_12345".to_string(),
        jwt_expiration_seconds: 3600,
    };

    let db = PgPool::connect_lazy(&config.database_url).unwrap();
    let cache_service = Arc::new(MockCache) as DynCacheService;
    let user_service = UserService::new(db, cache_service, config.clone());
    let queue_publisher = Arc::new(MockQueue) as DynQueueService;

    let state = AppState {
        user_service,
        queue_publisher,
        config: config.clone(),
    };

    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/docs/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

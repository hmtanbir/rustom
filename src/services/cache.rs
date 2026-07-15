use async_trait::async_trait;
use redis::AsyncCommands;
use redis::cmd;

use crate::errors::AppError;
use crate::infrastructure::RedisPool;
use std::sync::Arc;

/// Generic caching service trait for storage operations.
#[async_trait]
pub trait CacheService: Send + Sync {
    /// Retrieve a value from cache as a string.
    async fn get(&self, key: &str) -> Result<Option<String>, AppError>;

    /// Store a string value in the cache with a Time-To-Live.
    async fn set(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<(), AppError>;

    /// Remove a value from cache by key.
    async fn delete(&self, key: &str) -> Result<(), AppError>;

    /// Atomically increment an integer value in the cache and set a TTL on first increment.
    /// Returns the new value after increment. Uses a Lua script to prevent race conditions
    /// between INCR and EXPIRE.
    async fn incr_with_ttl(&self, key: &str, ttl_seconds: u64) -> Result<i64, AppError>;
}

/// Redis-backed implementation of CacheService.
#[derive(Clone)]
pub struct RedisCacheService {
    pool: RedisPool,
}

impl RedisCacheService {
    /// Create a new RedisCacheService instance.
    pub fn new(pool: RedisPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CacheService for RedisCacheService {
    async fn get(&self, key: &str) -> Result<Option<String>, AppError> {
        let mut conn = self.pool.get().await.map_err(|e| {
            AppError::Cache(format!(
                "Failed to acquire connection from Redis pool: {}",
                e
            ))
        })?;

        let value_str: Option<String> = conn
            .get(key)
            .await
            .map_err(|e| AppError::Cache(format!("Redis GET command failed: {}", e)))?;

        Ok(value_str)
    }

    async fn set(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<(), AppError> {
        let mut conn = self.pool.get().await.map_err(|e| {
            AppError::Cache(format!(
                "Failed to acquire connection from Redis pool: {}",
                e
            ))
        })?;

        let _: () = conn
            .set_ex(key, value, ttl_seconds)
            .await
            .map_err(|e| AppError::Cache(format!("Redis SETEX command failed: {}", e)))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), AppError> {
        let mut conn = self.pool.get().await.map_err(|e| {
            AppError::Cache(format!(
                "Failed to acquire connection from Redis pool: {}",
                e
            ))
        })?;

        let _: () = conn
            .del(key)
            .await
            .map_err(|e| AppError::Cache(format!("Redis DEL command failed: {}", e)))?;

        Ok(())
    }

    async fn incr_with_ttl(&self, key: &str, ttl_seconds: u64) -> Result<i64, AppError> {
        let mut conn = self.pool.get().await.map_err(|e| {
            AppError::Cache(format!(
                "Failed to acquire connection from Redis pool: {}",
                e
            ))
        })?;

        // Lua script: atomically increment and set TTL on first increment.
        // This prevents the race condition where INCR succeeds but EXPIRE never fires.
        let script = r#"
        local val = redis.call('INCR', KEYS[1])
        if val == 1 then
            redis.call('EXPIRE', KEYS[1], ARGV[1])
        end
        return val
        "#;

        let val: i64 = cmd("EVAL")
            .arg(script)
            .arg(1) // number of keys
            .arg(key)
            .arg(ttl_seconds as i64)
            .query_async(&mut *conn)
            .await
            .map_err(|e| AppError::Cache(format!("Redis EVAL command failed: {}", e)))?;

        Ok(val)
    }
}

/// Convenience type alias for shared cache services.
pub type DynCacheService = Arc<dyn CacheService>;

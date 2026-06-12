use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{encode, Header, EncodingKey};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;

use crate::config::AppConfig;
use crate::domain::{
    AppError, Claims, User, UserLoginRequestDto, UserLoginResponseDto,
    UserRegisterRequestDto, UserResponseDto, UserRole,
};
use crate::services::cache::DynCacheService;

/// Service containing business logic for users, auth, password verification, and caching.
#[derive(Clone)]
pub struct UserService {
    db: PgPool,
    cache: DynCacheService,
    config: AppConfig,
}

impl UserService {
    /// Create a new UserService.
    pub fn new(db: PgPool, cache: DynCacheService, config: AppConfig) -> Self {
        Self { db, cache, config }
    }

    /// Registers a new user, hashes their password, and saves them to the database.
    pub async fn register(&self, dto: UserRegisterRequestDto) -> Result<UserResponseDto, AppError> {
        // Validate if user already exists
        let existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(&dto.email)
            .fetch_optional(&self.db)
            .await?;

        if existing.is_some() {
            return Err(AppError::Conflict("Email is already registered".to_string()));
        }

        // Hash password using Argon2id
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(dto.password.as_bytes(), &salt)
            .map_err(|e| AppError::Authentication(format!("Password hashing failure: {}", e)))?
            .to_string();

        let assigned_role = dto.role.unwrap_or(UserRole::User);

        // Save to DB inside transaction
        let mut tx = self.db.begin().await?;
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (email, password_hash, role)
            VALUES ($1, $2, $3)
            RETURNING id, email, password_hash, role, created_at, updated_at
            "#
        )
        .bind(&dto.email)
        .bind(&password_hash)
        .bind(assigned_role)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        tracing::info!("Registered user: {} with role: {}", user.email, user.role);
        Ok(UserResponseDto::from(user))
    }

    /// Authenticates user credentials, validates password hash, and issues a JWT token.
    pub async fn login(&self, dto: UserLoginRequestDto) -> Result<UserLoginResponseDto, AppError> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(&dto.email)
            .fetch_optional(&self.db)
            .await?
            .ok_or(AppError::Authentication("Invalid email or password".to_string()))?;

        // Verify password hash
        let parsed_hash = PasswordHash::new(&user.password_hash).map_err(|e| {
            AppError::Authentication(format!("Invalid password hash representation: {}", e))
        })?;

        Argon2::default()
            .verify_password(dto.password.as_bytes(), &parsed_hash)
            .map_err(|_| AppError::Authentication("Invalid email or password".to_string()))?;

        // Generate JWT claims
        let exp = Utc::now()
            .checked_add_signed(chrono::Duration::seconds(
                self.config.jwt_expiration_seconds as i64,
            ))
            .ok_or_else(|| AppError::Unexpected(anyhow::anyhow!("Time calculation overflow")))?
            .timestamp() as u64;

        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role,
            exp,
        };

        // Sign token
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::Authentication(format!("Token signing failure: {}", e)))?;

        tracing::info!("User logged in successfully: {}", user.email);

        Ok(UserLoginResponseDto {
            token,
            user: UserResponseDto::from(user),
        })
    }

    /// Retrieve user profile, checking Redis cache first.
    pub async fn get_profile(&self, user_id: Uuid) -> Result<UserResponseDto, AppError> {
        let cache_key = format!("user:profile:{}", user_id);

        // Attempt reading from cache
        if let Ok(Some(cached_str)) = self.cache.get(&cache_key).await {
            if let Ok(cached_user) = serde_json::from_str::<UserResponseDto>(&cached_str) {
                tracing::debug!("Cache hit for user profile ID: {}", user_id);
                return Ok(cached_user);
            }
        }

        // Cache miss -> query Postgres database
        tracing::debug!("Cache miss for user profile ID: {}", user_id);
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("User with ID {} not found", user_id)))?;

        let user_dto = UserResponseDto::from(user);

        // Store back in Cache (expires in 5 minutes)
        if let Ok(json_str) = serde_json::to_string(&user_dto) {
            if let Err(e) = self.cache.set(&cache_key, &json_str, 300).await {
                tracing::warn!("Failed to store user profile in cache: {:?}", e);
            }
        }

        Ok(user_dto)
    }

    /// Update user role and invalidate their cached profile.
    pub async fn update_role(&self, user_id: Uuid, role: UserRole) -> Result<UserResponseDto, AppError> {
        let mut tx = self.db.begin().await?;

        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET role = $1, updated_at = NOW()
            WHERE id = $2
            RETURNING id, email, password_hash, role, created_at, updated_at
            "#
        )
        .bind(role)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User ID {} not found to update", user_id)))?;

        tx.commit().await?;

        let user_dto = UserResponseDto::from(user);

        // Invalidate cache key
        let cache_key = format!("user:profile:{}", user_id);
        if let Err(e) = self.cache.delete(&cache_key).await {
            tracing::warn!("Failed to invalidate cache for key {}: {:?}", cache_key, e);
        } else {
            tracing::info!("Invalidated cache key: {}", cache_key);
        }

        Ok(user_dto)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use std::sync::Arc;

    mock! {
        pub Cache {}
        #[async_trait::async_trait]
        impl crate::services::cache::CacheService for Cache {
            async fn get(&self, key: &str) -> Result<Option<String>, AppError>;
            async fn set(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<(), AppError>;
            async fn delete(&self, key: &str) -> Result<(), AppError>;
        }
    }

    #[tokio::test]
    async fn test_get_profile_cache_hit() {
        let mut mock_cache = MockCache::new();

        let target_id = Uuid::new_v4();
        let expected_user = UserResponseDto {
            id: target_id,
            email: "cached@example.com".to_string(),
            role: UserRole::User,
            created_at: Utc::now(),
        };

        let user_str = serde_json::to_string(&expected_user).unwrap();
        mock_cache
            .expect_get()
            .with(mockall::predicate::eq(format!("user:profile:{}", target_id)))
            .times(1)
            .returning(move |_| Ok(Some(user_str.clone())));

        // Since it's a cache hit, we don't query the database.
        // We can pass a lazy/unconnected PgPool.
        let db = PgPool::connect_lazy("postgres://localhost/dummy_db").unwrap();

        let config = AppConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            database_url: "postgres://localhost/dummy_db".to_string(),
            redis_url: "redis://localhost/dummy_redis".to_string(),
            rabbitmq_url: "amqp://localhost/dummy_rabbit".to_string(),
            jwt_secret: "super_secret_jwt_key_that_is_long_enough_to_be_secure_12345".to_string(),
            jwt_expiration_seconds: 3600,
        };

        let user_service = UserService::new(db, Arc::new(mock_cache), config);
        let result = user_service.get_profile(target_id).await.unwrap();

        assert_eq!(result.id, target_id);
        assert_eq!(result.email, "cached@example.com");
        assert_eq!(result.role, UserRole::User);
    }
}


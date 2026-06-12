use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{encode, Header, EncodingKey};
use sqlx::PgPool;
use uuid::Uuid;
use crate::errors::AppError;
use chrono::Utc;

use crate::config::AppConfig;
use crate::models::{
    Claims, User, UserLoginRequestDto, UserLoginResponseDto,
    UserRegisterRequestDto, UserCreateRequestDto, UserUpdateRequestDto,
    PaginationParams, PaginatedResponse
};
use crate::serializers::user_serializer::UserSerializer;
use crate::services::cache::DynCacheService;

#[derive(Clone)]
pub struct UserService {
    db: PgPool,
    cache: DynCacheService,
    config: AppConfig,
}

impl UserService {
    pub fn new(db: PgPool, cache: DynCacheService, config: AppConfig) -> Self {
        Self { db, cache, config }
    }

    pub async fn register(&self, dto: UserRegisterRequestDto) -> Result<UserSerializer, AppError> {
        let existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(&dto.email)
            .fetch_optional(&self.db)
            .await?;

        if existing.is_some() {
            return Err(AppError::Conflict("Email is already registered".to_string()));
        }

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_digest = argon2
            .hash_password(dto.password.as_bytes(), &salt)
            .map_err(|e| AppError::Authentication(format!("Password hashing failure: {}", e)))?
            .to_string();

        let mut tx = self.db.begin().await?;
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (name, email, password_digest, role, status)
            VALUES ($1, $2, $3, 1, 1)
            RETURNING *
            "#
        )
        .bind(&dto.name)
        .bind(&dto.email)
        .bind(&password_digest)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(UserSerializer::from(user))
    }

    pub async fn login(&self, dto: UserLoginRequestDto) -> Result<UserLoginResponseDto, AppError> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(&dto.email)
            .fetch_optional(&self.db)
            .await?
            .ok_or(AppError::NotFound("invalid email".to_string()))?;

        let parsed_hash = PasswordHash::new(&user.password_digest).map_err(|e| {
            AppError::Authentication(format!("Invalid password hash representation: {}", e))
        })?;

        Argon2::default()
            .verify_password(dto.password.as_bytes(), &parsed_hash)
            .map_err(|_| AppError::Authentication("invalid password".to_string()))?;
            
        if user.is_inactive() {
            return Err(AppError::Authentication("User is inactive or deleted".to_string()));
        }

        let exp = Utc::now()
            .checked_add_signed(chrono::Duration::seconds(
                self.config.jwt_expiration_seconds as i64,
            ))
            .ok_or_else(|| AppError::Unexpected(anyhow::anyhow!("Time calculation overflow")))?
            .timestamp() as u64;

        let claims = Claims {
            user_id: user.id,
            role: user.role,
            exp,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::Authentication(format!("Token signing failure: {}", e)))?;

        Ok(UserLoginResponseDto { token })
    }

    pub async fn get_users_paginated(&self, params: PaginationParams) -> Result<PaginatedResponse<UserSerializer>, AppError> {
        let cache_key = format!(
            "users_index/{}/{}/{}/{}/{}",
            params.role.as_deref().unwrap_or("all"),
            params.deleted.unwrap_or(false),
            params.get_page(),
            params.get_per_page(),
            "latest" // In a real app we'd track last_updated_at but simplifying for now
        );

        if let Ok(Some(cached_str)) = self.cache.get(&cache_key).await
            && let Ok(cached) = serde_json::from_str::<PaginatedResponse<UserSerializer>>(&cached_str) {
                return Ok(cached);
            }

        let role_filter = params.role.as_deref().map(|r| if r == "admin" { 0 } else { 1 });
        let deleted_filter = params.deleted.unwrap_or(false);

        let mut query = "SELECT * FROM users WHERE 1=1".to_string();
        let mut count_query = "SELECT COUNT(*) FROM users WHERE 1=1".to_string();
        
        if deleted_filter {
            query.push_str(" AND deleted_at IS NOT NULL");
            count_query.push_str(" AND deleted_at IS NOT NULL");
        } else {
            query.push_str(" AND deleted_at IS NULL");
            count_query.push_str(" AND deleted_at IS NULL");
        }
        
        if let Some(r) = role_filter {
            query.push_str(&format!(" AND role = {}", r));
            count_query.push_str(&format!(" AND role = {}", r));
        }

        let total_count: (i64,) = sqlx::query_as(&count_query).fetch_one(&self.db).await?;
        let total_count = total_count.0 as u32;

        let total_pages = if total_count == 0 { 1 } else { (total_count as f32 / params.get_per_page() as f32).ceil() as u32 };
        
        query.push_str(" ORDER BY created_at DESC LIMIT $1 OFFSET $2");

        let users = sqlx::query_as::<_, User>(&query)
            .bind(params.get_per_page() as i64)
            .bind(params.offset() as i64)
            .fetch_all(&self.db)
            .await?;

        let response = PaginatedResponse {
            status: 200,
            message: "Successfully data fetched".to_string(),
            data: users.into_iter().map(UserSerializer::from).collect(),
            current_page: params.get_page(),
            per_page: params.get_per_page(),
            total_pages,
            total_count,
            next_page: if params.get_page() < total_pages { Some(params.get_page() + 1) } else { None },
            prev_page: if params.get_page() > 1 { Some(params.get_page() - 1) } else { None },
        };

        if let Ok(json_str) = serde_json::to_string(&response) {
            let ttl = std::env::var("API_CACHE_TTL").unwrap_or_else(|_| "3600".to_string()).parse().unwrap_or(3600);
            let _ = self.cache.set(&cache_key, &json_str, ttl).await;
        }

        Ok(response)
    }

    pub async fn get_user(&self, user_id: Uuid) -> Result<UserSerializer, AppError> {
        let cache_key = format!("user:profile:{}", user_id);

        if let Ok(Some(cached_str)) = self.cache.get(&cache_key).await
            && let Ok(cached_user) = serde_json::from_str::<UserSerializer>(&cached_str) {
                return Ok(cached_user);
            }

        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        let user_dto = UserSerializer::from(user);

        if let Ok(json_str) = serde_json::to_string(&user_dto) {
            let ttl = std::env::var("API_CACHE_TTL").unwrap_or_else(|_| "3600".to_string()).parse().unwrap_or(3600);
            let _ = self.cache.set(&cache_key, &json_str, ttl).await;
        }

        Ok(user_dto)
    }

    pub async fn create_user(&self, dto: UserCreateRequestDto) -> Result<UserSerializer, AppError> {
        let salt = SaltString::generate(&mut OsRng);
        let password_digest = Argon2::default()
            .hash_password(dto.password.as_bytes(), &salt)
            .map_err(|e| AppError::Authentication(format!("Hashing error: {}", e)))?
            .to_string();

        let mut tx = self.db.begin().await?;
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (name, email, password_digest, role, status)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#
        )
        .bind(&dto.name)
        .bind(&dto.email)
        .bind(&password_digest)
        .bind(dto.role.unwrap_or(1))
        .bind(dto.status.unwrap_or(1))
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(UserSerializer::from(user))
    }

    pub async fn update_user(&self, user_id: Uuid, dto: UserUpdateRequestDto) -> Result<UserSerializer, AppError> {
        let mut tx = self.db.begin().await?;
        
        let _existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET name = COALESCE($1, name),
                email = COALESCE($2, email),
                role = COALESCE($3, role),
                status = COALESCE($4, status),
                updated_at = NOW()
            WHERE id = $5
            RETURNING *
            "#
        )
        .bind(&dto.name)
        .bind(&dto.email)
        .bind(dto.role)
        .bind(dto.status)
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        let cache_key = format!("user:profile:{}", user_id);
        let _ = self.cache.delete(&cache_key).await;

        Ok(UserSerializer::from(user))
    }

    pub async fn soft_delete_user(&self, user_id: Uuid) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        
        let result = sqlx::query("UPDATE users SET deleted_at = NOW() WHERE id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
            
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("User not found".to_string()));
        }

        tx.commit().await?;
        
        let cache_key = format!("user:profile:{}", user_id);
        let _ = self.cache.delete(&cache_key).await;

        Ok(())
    }
}

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use chrono::Utc;
use jsonwebtoken::{EncodingKey, Header, encode};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::errors::AppError;
use crate::models::{
    Claims, User, UserCreateRequestDto, UserLoginRequestDto, UserLoginResponseDto,
    UserRegisterRequestDto, UserUpdateRequestDto,
};
use crate::queries::UserQueryParams;
use crate::serializers::user_serializer::UserSerializer;
use crate::services::cache::DynCacheService;
use crate::utils::pagination::{PaginatedResponse, PaginationParams};
use crate::validations::{validate_user_create, validate_user_update};

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

    pub fn get_cache(&self) -> &DynCacheService {
        &self.cache
    }

    fn validate_password(&self, password: &str) -> Result<(), AppError> {
        if password.len() < 8 {
            return Err(AppError::InvalidInput(
                "Password must be at least 8 characters long".to_string(),
            ));
        }
        let has_uppercase = password.chars().any(|c| c.is_uppercase());
        let has_lowercase = password.chars().any(|c| c.is_lowercase());
        let has_digit = password.chars().any(|c| c.is_numeric());
        let has_special = password.chars().any(|c| !c.is_alphanumeric());

        if !has_uppercase || !has_lowercase || !has_digit || !has_special {
            return Err(AppError::InvalidInput(
                "Password must contain at least one uppercase letter, one lowercase letter, one digit, and one special character".to_string()
            ));
        }
        Ok(())
    }

    pub async fn register(&self, dto: UserRegisterRequestDto) -> Result<UserSerializer, AppError> {
        self.validate_password(&dto.password)?;
        validate_user_create(
            &self.db,
            Some(&dto.name),
            Some(&dto.email),
            Some(&dto.password),
        )
        .await?;

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
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, name, email, password_digest, role, status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.email)
        .bind(&password_digest)
        .bind(1i32) // role: standard user (cannot be overridden via public registration)
        .bind(1i32) // status: active (cannot be overridden via public registration)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        self.invalidate_users_index().await;

        Ok(UserSerializer::from(user))
    }

    pub async fn login(&self, dto: UserLoginRequestDto) -> Result<UserLoginResponseDto, AppError> {
        let user_result = sqlx::query_as::<_, User>("SELECT id, name, email, password_digest, role, status, created_at, updated_at, deleted_at FROM users WHERE email = $1 AND deleted_at IS NULL")
            .bind(&dto.email)
            .fetch_optional(&self.db)
            .await?;

        let (user, is_valid) = match user_result {
            Some(u) => {
                let parsed_hash = PasswordHash::new(&u.password_digest).map_err(|e| {
                    AppError::Authentication(format!("Invalid password hash representation: {}", e))
                })?;
                let is_valid = Argon2::default()
                    .verify_password(dto.password.as_bytes(), &parsed_hash)
                    .is_ok();
                (Some(u), is_valid)
            }
            None => {
                // Dummy hash for "password" to mitigate timing attacks
                let dummy_hash = "$argon2id$v=19$m=19456,t=2,p=1$mIk38++6ZCEyzKo+edgXEw$/h0anRjDkzS46suJM6/P3+DySS3qp1+6jXtNjd6UMTs";
                let parsed_hash = PasswordHash::new(dummy_hash).unwrap();
                let is_valid = Argon2::default()
                    .verify_password(dto.password.as_bytes(), &parsed_hash)
                    .is_ok();
                (None, is_valid)
            }
        };

        if !is_valid || user.is_none() {
            return Err(AppError::Authentication(
                "Invalid email or password".to_string(),
            ));
        }

        let user = user.unwrap();

        if user.is_inactive() {
            return Err(AppError::Authentication(
                "User is inactive or suspended".to_string(),
            ));
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
            status: user.status,
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

    pub async fn get_users_paginated(
        &self,
        params: PaginationParams,
        filter: UserQueryParams,
    ) -> Result<PaginatedResponse<UserSerializer>, AppError> {
        let version = self
            .cache
            .get("users_index_version")
            .await
            .unwrap_or_default()
            .unwrap_or_else(|| "0".to_string());

        let cache_key = format!(
            "users_index/{}/{}/{}/{}/{}/{}/{}/{}/{}",
            filter.email.as_deref().unwrap_or("all"),
            filter.role.as_deref().unwrap_or("all"),
            filter.status.as_deref().unwrap_or("all"),
            filter.uuid.as_deref().unwrap_or("all"),
            filter.deleted.unwrap_or(false),
            filter.order.as_deref().unwrap_or("desc"),
            params.get_page(),
            params.get_per_page(),
            version
        );

        if let Ok(Some(cached_str)) = self.cache.get(&cache_key).await
            && let Ok(cached) =
                serde_json::from_str::<PaginatedResponse<UserSerializer>>(&cached_str)
        {
            return Ok(cached);
        }

        let mut query = sqlx::QueryBuilder::<'_, sqlx::Postgres>::new(
            "SELECT id, name, email, password_digest, role, status, created_at, updated_at, deleted_at FROM users WHERE 1=1",
        );
        let mut count_query =
            sqlx::QueryBuilder::<'_, sqlx::Postgres>::new("SELECT COUNT(*) FROM users WHERE 1=1");

        let deleted_filter = filter.deleted.unwrap_or(false);
        if deleted_filter {
            query.push(" AND deleted_at IS NOT NULL");
            count_query.push(" AND deleted_at IS NOT NULL");
        } else {
            query.push(" AND deleted_at IS NULL");
            count_query.push(" AND deleted_at IS NULL");
        }

        if let Some(ref email_str) = filter.email {
            let email_pattern = format!("%{}%", email_str);
            query.push(" AND email ILIKE ");
            query.push_bind(email_pattern.clone());
            count_query.push(" AND email ILIKE ");
            count_query.push_bind(email_pattern);
        }

        if let Some(ref role_str) = filter.role {
            let role_str_lower = role_str.to_lowercase();
            let mut matched_roles = Vec::new();
            for (role_name, &role_id) in crate::models::user::ROLES_MAP.iter() {
                if role_name.to_lowercase().contains(&role_str_lower) {
                    matched_roles.push(role_id);
                }
            }
            if matched_roles.is_empty() {
                query.push(" AND 1=0");
                count_query.push(" AND 1=0");
            } else {
                query.push(" AND role IN (");
                count_query.push(" AND role IN (");
                let mut first = true;
                for r in matched_roles {
                    if !first {
                        query.push(", ");
                        count_query.push(", ");
                    }
                    first = false;
                    query.push_bind(r);
                    count_query.push_bind(r);
                }
                query.push(")");
                count_query.push(")");
            }
        }

        if let Some(ref status_str) = filter.status {
            let status_str_lower = status_str.to_lowercase();
            let mut matched_statuses = Vec::new();
            for (status_name, &status_id) in crate::models::user::STATUSES_MAP.iter() {
                if status_name.to_lowercase().contains(&status_str_lower) {
                    matched_statuses.push(status_id);
                }
            }
            if matched_statuses.is_empty() {
                query.push(" AND 1=0");
                count_query.push(" AND 1=0");
            } else {
                query.push(" AND status IN (");
                count_query.push(" AND status IN (");
                let mut first = true;
                for s in matched_statuses {
                    if !first {
                        query.push(", ");
                        count_query.push(", ");
                    }
                    first = false;
                    query.push_bind(s);
                    count_query.push_bind(s);
                }
                query.push(")");
                count_query.push(")");
            }
        }

        if let Some(ref uuid_str) = filter.uuid {
            if let Ok(parsed_uuid) = uuid::Uuid::parse_str(uuid_str.as_str()) {
                query.push(" AND id = ");
                query.push_bind(parsed_uuid);
                count_query.push(" AND id = ");
                count_query.push_bind(parsed_uuid);
            } else {
                query.push(" AND 1=0");
                count_query.push(" AND 1=0");
            }
        }

        let total_count: i64 = count_query.build_query_scalar().fetch_one(&self.db).await?;
        let total_count = total_count as u32;

        let total_pages = if total_count == 0 {
            1
        } else {
            (total_count as f32 / params.get_per_page() as f32).ceil() as u32
        };

        let order_dir = match filter.order.as_deref() {
            Some("asc") => "ASC",
            _ => "DESC",
        };

        query.push(format!(" ORDER BY created_at {}", order_dir));
        query.push(" LIMIT ");
        query.push_bind(params.get_per_page() as i64);
        query.push(" OFFSET ");
        query.push_bind(params.offset() as i64);

        let users = query.build_query_as::<User>().fetch_all(&self.db).await?;

        let response = PaginatedResponse {
            status: 200,
            message: "Successfully data fetched".to_string(),
            data: users.into_iter().map(UserSerializer::from).collect(),
            current_page: params.get_page(),
            per_page: params.get_per_page(),
            total_pages,
            total_count,
            next_page: if params.get_page() < total_pages {
                Some(params.get_page() + 1)
            } else {
                None
            },
            prev_page: if params.get_page() > 1 {
                Some(params.get_page() - 1)
            } else {
                None
            },
        };

        if let Ok(json_str) = serde_json::to_string(&response) {
            let ttl = std::env::var("API_CACHE_TTL")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600);
            let _ = self.cache.set(&cache_key, &json_str, ttl).await;
        }

        Ok(response)
    }

    pub async fn get_user(&self, user_id: Uuid) -> Result<UserSerializer, AppError> {
        let cache_key = format!("user:profile:{}", user_id);

        if let Ok(Some(cached_str)) = self.cache.get(&cache_key).await
            && let Ok(cached_user) = serde_json::from_str::<UserSerializer>(&cached_str)
        {
            return Ok(cached_user);
        }

        let user = sqlx::query_as::<_, User>("SELECT id, name, email, password_digest, role, status, created_at, updated_at, deleted_at FROM users WHERE id = $1 AND deleted_at IS NULL")
            .bind(user_id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Not Found".to_string()))?;

        let user_dto = UserSerializer::from(user);

        if let Ok(json_str) = serde_json::to_string(&user_dto) {
            let ttl = std::env::var("API_CACHE_TTL")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600);
            let _ = self.cache.set(&cache_key, &json_str, ttl).await;
        }

        Ok(user_dto)
    }

    pub async fn create_user(&self, dto: UserCreateRequestDto) -> Result<UserSerializer, AppError> {
        self.validate_password(&dto.password)?;
        validate_user_create(
            &self.db,
            Some(&dto.name),
            Some(&dto.email),
            Some(&dto.password),
        )
        .await?;

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
            RETURNING id, name, email, password_digest, role, status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.email)
        .bind(&password_digest)
        .bind(dto.role.unwrap_or(1))
        .bind(dto.status.unwrap_or(1))
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        self.invalidate_users_index().await;

        Ok(UserSerializer::from(user))
    }

    pub async fn update_user(
        &self,
        user_id: Uuid,
        dto: UserUpdateRequestDto,
    ) -> Result<UserSerializer, AppError> {
        validate_user_update(&self.db, user_id, dto.email.as_deref()).await?;

        let mut tx = self.db.begin().await?;

        let existing = sqlx::query_as::<_, User>("SELECT id, name, email, password_digest, role, status, created_at, updated_at, deleted_at FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("Not Found".to_string()))?;

        if existing.deleted_at.is_some() && dto.deleted_at != Some(None) {
            return Err(AppError::NotFound("Not Found".to_string()));
        }

        let password_digest = if let Some(ref pwd) = dto.password {
            self.validate_password(pwd)?;
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();
            Some(
                argon2
                    .hash_password(pwd.as_bytes(), &salt)
                    .map_err(|e| {
                        AppError::Authentication(format!("Password hashing failure: {}", e))
                    })?
                    .to_string(),
            )
        } else {
            None
        };

        let has_deleted_at = dto.deleted_at.is_some();
        let deleted_at_val = dto.deleted_at.flatten();

        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET name = COALESCE($1, name),
                email = COALESCE($2, email),
                role = COALESCE($3, role),
                status = COALESCE($4, status),
                password_digest = COALESCE($5, password_digest),
                deleted_at = CASE WHEN $6 THEN $7 ELSE deleted_at END,
                updated_at = NOW()
            WHERE id = $8
            RETURNING id, name, email, password_digest, role, status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.email)
        .bind(dto.role)
        .bind(dto.status)
        .bind(&password_digest)
        .bind(has_deleted_at)
        .bind(deleted_at_val)
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        let cache_key = format!("user:profile:{}", user_id);
        let _ = self.cache.delete(&cache_key).await;
        self.invalidate_users_index().await;

        Ok(UserSerializer::from(user))
    }

    pub async fn restore_user(&self, user_id: Uuid) -> Result<UserSerializer, AppError> {
        let mut tx = self.db.begin().await?;

        let existing = sqlx::query_as::<_, User>("SELECT id, name, email, password_digest, role, status, created_at, updated_at, deleted_at FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("Not Found".to_string()))?;

        if existing.deleted_at.is_none() {
            let mut errors = std::collections::HashMap::new();
            errors.insert("deleted_at".to_string(), vec!["is not deleted".to_string()]);
            return Err(AppError::Validation(errors));
        }

        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET deleted_at = NULL,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, email, password_digest, role, status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        let cache_key = format!("user:profile:{}", user_id);
        let _ = self.cache.delete(&cache_key).await;
        self.invalidate_users_index().await;

        Ok(UserSerializer::from(user))
    }

    async fn invalidate_users_index(&self) {
        let _ = self
            .cache
            .set(
                "users_index_version",
                &Utc::now().timestamp().to_string(),
                86400,
            )
            .await;
    }

    pub async fn soft_delete_user(&self, user_id: Uuid) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;

        let result =
            sqlx::query("UPDATE users SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
                .bind(user_id)
                .execute(&mut *tx)
                .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Not Found".to_string()));
        }

        tx.commit().await?;

        let cache_key = format!("user:profile:{}", user_id);
        let _ = self.cache.delete(&cache_key).await;
        self.invalidate_users_index().await;

        Ok(())
    }
}

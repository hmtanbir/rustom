use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct RoleConfig {
    pub int_to_str: HashMap<u8, String>,
    pub str_to_int: HashMap<String, u8>,
}

pub static ROLES: OnceLock<RoleConfig> = OnceLock::new();

fn get_roles() -> &'static RoleConfig {
    ROLES.get_or_init(|| {
        let mut int_to_str = HashMap::new();
        let mut str_to_int = HashMap::new();
        
        if let Ok(content) = std::fs::read_to_string("src/config/data/roles.yml") {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some((key_str, val_str)) = trimmed.split_once(':') {
                    if let Ok(key) = key_str.trim().parse::<u8>() {
                        let val = val_str.trim().trim_matches('"').trim_matches('\'').to_string();
                        int_to_str.insert(key, val.clone());
                        str_to_int.insert(val, key);
                    }
                }
            }
        }
        
        // Fallbacks
        if int_to_str.is_empty() {
            int_to_str.insert(0, "admin".to_string());
            int_to_str.insert(1, "user".to_string());
            str_to_int.insert("admin".to_string(), 0);
            str_to_int.insert("user".to_string(), 1);
        }
        
        RoleConfig { int_to_str, str_to_int }
    })
}

/// Application user roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToSchema)]
pub enum UserRole {
    Admin,
    User,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let config = get_roles();
        let val = match self {
            UserRole::Admin => config.int_to_str.get(&0).map(|s| s.as_str()).unwrap_or("admin"),
            UserRole::User => config.int_to_str.get(&1).map(|s| s.as_str()).unwrap_or("user"),
        };
        write!(f, "{}", val)
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let config = get_roles();
        let s_lower = s.to_lowercase();
        
        let admin_str = config.int_to_str.get(&0).map(|s| s.to_lowercase()).unwrap_or_else(|| "admin".to_string());
        let user_str = config.int_to_str.get(&1).map(|s| s.to_lowercase()).unwrap_or_else(|| "user".to_string());
        
        if s_lower == admin_str {
            Ok(UserRole::Admin)
        } else if s_lower == user_str {
            Ok(UserRole::User)
        } else {
            Err(format!("Unknown role: {}", s))
        }
    }
}

impl Serialize for UserRole {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for UserRole {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct UserRoleVisitor;

        impl<'de> serde::de::Visitor<'de> for UserRoleVisitor {
            type Value = UserRole;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or integer representing UserRole")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value.parse().map_err(serde::de::Error::custom)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(UserRole::Admin),
                    1 => Ok(UserRole::User),
                    _ => Err(serde::de::Error::custom(format!("Invalid integer role: {}", value))),
                }
            }
            
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(UserRole::Admin),
                    1 => Ok(UserRole::User),
                    _ => Err(serde::de::Error::custom(format!("Invalid integer role: {}", value))),
                }
            }
        }

        deserializer.deserialize_any(UserRoleVisitor)
    }
}

impl<'r> sqlx::Type<sqlx::Postgres> for UserRole {
    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &<sqlx::Postgres as sqlx::Database>::TypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for UserRole {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let s = self.to_string();
        <String as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for UserRole {
    fn decode(
        value: <sqlx::Postgres as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        let role = s.parse::<UserRole>().map_err(|e| sqlx::error::Error::Decode(e.into()))?;
        Ok(role)
    }
}

/// Core domain representation of a User in the database.
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    /// Unique identifier for user.
    pub id: Uuid,
    /// Email address of user.
    pub email: String,
    /// Password hash stored securely.
    pub password_hash: String,
    /// Role of the user.
    pub role: UserRole,
    /// Date time when the user registration occurred.
    pub created_at: DateTime<Utc>,
    /// Date time when the user last updated details.
    pub updated_at: DateTime<Utc>,
}

/// Request schema for user registration.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct UserRegisterRequestDto {
    /// Valid email address.
    #[schema(example = "user@example.com")]
    pub email: String,
    /// Plaintext password (hashed internally before storage).
    #[schema(example = "SecretPassword123")]
    pub password: String,
    /// Optional role to assign. Defaults to User.
    #[schema(example = "User")]
    pub role: Option<UserRole>,
}

/// Request schema for user login authentication.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct UserLoginRequestDto {
    /// User email address.
    #[schema(example = "user@example.com")]
    pub email: String,
    /// User plaintext password.
    #[schema(example = "SecretPassword123")]
    pub password: String,
}

/// User details response data transfer object.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct UserResponseDto {
    /// Unique UUID of user.
    pub id: Uuid,
    /// Registered email.
    pub email: String,
    /// App assigned role.
    pub role: UserRole,
    /// Date time of registration.
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponseDto {
    fn from(user: User) -> Self {
        UserResponseDto {
            id: user.id,
            email: user.email,
            role: user.role,
            created_at: user.created_at,
        }
    }
}

/// Response schema for successful user authentication.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct UserLoginResponseDto {
    /// Bearer authentication token.
    pub token: String,
    /// Authenticated user credentials.
    pub user: UserResponseDto,
}

/// JWT Token claim layout.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject - unique user UUID string.
    pub sub: String,
    /// Registered email address.
    pub email: String,
    /// Security user role.
    pub role: UserRole,
    /// Expiration Unix epoch timestamp.
    pub exp: u64,
}

/// Message payload representing a background task job.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct JobPayload {
    /// Unique job identifier.
    pub job_id: String,
    /// Type of job, e.g. "email" or "data_process".
    #[schema(example = "email")]
    pub job_type: String,
    /// Arbitrary job arguments in JSON payload.
    pub payload: serde_json::Value,
    /// Timestamp when job was queued.
    pub created_at: DateTime<Utc>,
}

/// Request schema for queueing a background worker job.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateJobRequestDto {
    /// The category of job, e.g. "email", "data_process".
    #[schema(example = "email")]
    pub job_type: String,
    /// Accompanying key-value attributes.
    #[schema(example = json!({"to": "recipient@example.com", "body": "Welcome message"}))]
    pub payload: serde_json::Value,
}

/// Response schema for queued job operations.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateJobResponseDto {
    /// Unique identifier for the created job.
    pub job_id: String,
    /// Success indicator.
    pub success: bool,
    /// Informational queueing status message.
    pub message: String,
}

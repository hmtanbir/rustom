use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use utoipa::ToSchema;
use uuid::Uuid;
use crate::models::user::User;

/// User details response serialized format.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct UserSerializer {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    #[schema(example = "user")]
    pub role: String,
    #[schema(example = "active")]
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl From<User> for UserSerializer {
    fn from(user: User) -> Self {
        let role_str = match user.role {
            0 => "admin".to_string(),
            1 => "user".to_string(),
            _ => user.role.to_string(),
        };

        let status_str = match user.status {
            0 => "inactive".to_string(),
            1 => "active".to_string(),
            _ => user.status.to_string(),
        };

        UserSerializer {
            id: user.id,
            name: user.name,
            email: user.email,
            role: role_str,
            status: status_str,
            created_at: user.created_at,
            updated_at: user.updated_at,
            deleted_at: user.deleted_at,
        }
    }
}

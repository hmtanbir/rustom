use super::{Validator, is_valid_email};
use crate::errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn validate_user_create(
    db: &PgPool,
    name: Option<&str>,
    email: Option<&str>,
    password: Option<&str>,
) -> Result<(), AppError> {
    let mut v = Validator::new();

    v.check_presence("name", name, "name");

    if let Some(email_str) = v.check_presence("email", email, "email") {
        v.check_length("email", &email_str, 255, "email");
        if !is_valid_email(&email_str) {
            v.add_error("email", "is invalid");
        } else {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 AND deleted_at IS NULL)",
            )
            .bind(email_str)
            .fetch_one(db)
            .await?;
            if exists {
                v.add_error("email", "has already been taken");
            }
        }
    }

    if let Some(pwd) = password {
        if pwd.trim().is_empty() {
            v.add_error("password_digest", "password_digest can't be blank");
        }
    } else {
        v.add_error("password_digest", "password_digest can't be blank");
    }

    v.into_result()
}

pub async fn validate_user_update(
    db: &PgPool,
    id: Uuid,
    email: Option<&str>,
) -> Result<(), AppError> {
    let mut v = Validator::new();

    if let Some(email) = email {
        let email_str = email.trim();
        if email_str.is_empty() {
            v.add_error("email", "email can't be blank");
        } else {
            v.check_length("email", email_str, 255, "email");
            if !is_valid_email(email_str) {
                v.add_error("email", "is invalid");
            } else {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 AND deleted_at IS NULL AND id != $2)",
                )
                .bind(email_str)
                .bind(id)
                .fetch_one(db)
                .await?;
                if exists {
                    v.add_error("email", "has already been taken");
                }
            }
        }
    }

    v.into_result()
}

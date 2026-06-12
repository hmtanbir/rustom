use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
    Extension,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use crate::config::AppConfig;
use crate::domain::{AppError, Claims};

/// Extractor type to enforce and inspect JWT authenticated users.
pub struct AuthenticatedUser(pub Claims);

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Retrieve AppConfig from router Extension layer
        let Extension(config) = Extension::<AppConfig>::from_request_parts(parts, state)
            .await
            .map_err(|_| {
                AppError::Unexpected(anyhow::anyhow!("AppConfig was not injected via Router extensions"))
            })?;

        // Inspect header authorization presence
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::Authentication("Missing Authorization header".to_string()))?;

        if !auth_header.starts_with("Bearer ") {
            return Err(AppError::Authentication(
                "Invalid Authorization header format. Expected Bearer token format".to_string(),
            ));
        }

        let token = &auth_header[7..];

        // Decode claims validating expiration and signature
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| AppError::Authentication(format!("Invalid or expired token: {}", e)))?;

        Ok(AuthenticatedUser(token_data.claims))
    }
}

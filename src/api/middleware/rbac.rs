use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use crate::domain::{AppError, UserRole};
use crate::api::middleware::auth::AuthenticatedUser;

/// Check role access privileges helper.
pub fn authorize_role(claims_role: UserRole, required_role: UserRole) -> Result<(), AppError> {
    // Admin override: admins can access all endpoints
    if claims_role == UserRole::Admin {
        return Ok(());
    }

    if claims_role == required_role {
        return Ok(());
    }

    Err(AppError::Authorization(format!(
        "Requires access role: {}, current user role is: {}",
        required_role, claims_role
    )))
}

/// Axum Middleware ensuring the authenticated user holds the Admin role.
pub async fn require_admin(
    AuthenticatedUser(claims): AuthenticatedUser,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    if claims.role != UserRole::Admin {
        return Err(AppError::Authorization(
            "Access forbidden: Admin credentials are required for this action".to_string(),
        ));
    }

    Ok(next.run(req).await)
}

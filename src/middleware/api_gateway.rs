use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use std::env;

use crate::errors::AppError;

pub async fn verify_api_gateway_key(
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let expected_key = env::var("API_GATEWAY_KEY").unwrap_or_default();
    let error_message = env::var("API_GATEWAY_ERROR_MESSAGE")
        .unwrap_or_else(|_| "Invalid User API Gateway Key".to_string());
    
    if expected_key.is_empty() {
        return Ok(next.run(req).await);
    }

    if let Some(provided_key) = req.headers().get("x-api-gateway-key") {
        if let Ok(key_str) = provided_key.to_str() {
            if key_str == expected_key {
                return Ok(next.run(req).await);
            }
        }
    }

    Err(AppError::Authorization(error_message))
}

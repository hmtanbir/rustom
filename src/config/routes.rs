use axum::{Extension, Router};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa_swagger_ui::SwaggerUi;

use crate::app_state::AppState;
use crate::controllers::api_routes;
use crate::docs::ApiDoc;
use crate::middleware::{payload_encryption, verify_api_gateway_key};
use utoipa::OpenApi;

/// Build the Axum Router configuring routes, CORS, logging, and Swagger UI.
pub fn create_router(state: AppState) -> Router {
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
    let cors = if app_env == "production" {
        CorsLayer::new()
            .allow_origin(tower_http::cors::Any) // In a real app, specify actual origins here like `["https://example.com".parse().unwrap()]`
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any)
    } else {
        CorsLayer::permissive()
    };

    let api_routes = Router::new()
        // Mount all API routes under /api
        .nest("/api", api_routes())
        // Apply Gateway Key validation middleware (Runs inner to encryption)
        .layer(axum::middleware::from_fn(verify_api_gateway_key))
        .layer(axum::middleware::from_fn(payload_encryption));

    Router::new()
        // Serve OpenAPI document & Swagger UI automatically at /api-docs
        .merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(api_routes)
        // Add tracing/logging layer
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        // Injected via Extension so the JWT extractor can access AppConfig
        .layer(Extension(state.config.clone()))
        .with_state(state)
}

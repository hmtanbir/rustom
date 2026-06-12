use axum::{
    Extension, Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa_swagger_ui::SwaggerUi;

use crate::app_state::AppState;
use crate::controllers::api_routes;
use utoipa::OpenApi;
use crate::docs::ApiDoc;
use crate::middleware::{verify_api_gateway_key, payload_encryption};

/// Build the Axum Router configuring routes, CORS, logging, and Swagger UI.
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::permissive();

    Router::new()
        // Serve OpenAPI document & Swagger UI automatically at /api/docs
        .merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", ApiDoc::openapi()))
        // Mount all API routes under /api
        .nest("/api", api_routes())
        // Apply Gateway Key validation middleware (Runs inner to encryption)
        .layer(axum::middleware::from_fn(verify_api_gateway_key))
        .layer(axum::middleware::from_fn(payload_encryption))
        // Add tracing/logging layer
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        // Injected via Extension so the JWT extractor can access AppConfig
        .layer(Extension(state.config.clone()))
        .with_state(state)
}

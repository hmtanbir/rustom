use axum::response::IntoResponse;
use axum::{Extension, Router};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa_swagger_ui::SwaggerUi;

use crate::app_state::AppState;
use crate::controllers::api_routes;
use crate::docs::ApiDoc;
use crate::middleware::{payload_encryption, verify_api_gateway_key};
use utoipa::OpenApi;

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    /// The status of the API
    #[schema(example = "OK")]
    pub status: String,
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "API is healthy", body = HealthResponse)
    ),
    tag = "Health"
)]
pub async fn health_check() -> impl IntoResponse {
    axum::Json(HealthResponse {
        status: "OK".to_string(),
    })
}

/// Build the Axum Router configuring routes, CORS, logging, and Swagger UI.
pub fn create_router(state: AppState) -> Router {
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
    let mut allowed_origins = Vec::new();
    for domain in state.config.domain_name.split(',') {
        let trimmed = domain.trim();
        if !trimmed.is_empty()
            && let Ok(origin) = trimmed.parse::<axum::http::HeaderValue>()
        {
            allowed_origins.push(origin);
        }
    }

    let mut cors = CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            axum::http::header::HeaderName::from_static("x-api-gateway-key"),
        ]);

    // In development, also allow localhost origins for convenience
    if app_env != "production" {
        if let Ok(dev_origin) = "http://localhost:3000".parse::<axum::http::HeaderValue>() {
            cors = cors.allow_origin([dev_origin]);
        }
        if let Ok(dev_origin) = "http://127.0.0.1:3000".parse::<axum::http::HeaderValue>() {
            cors = cors.allow_origin([dev_origin]);
        }
    }

    let api_routes = Router::new()
        // Mount all API routes under /api
        .nest("/api", api_routes())
        // Apply Gateway Key validation middleware (Runs inner to encryption)
        .layer(axum::middleware::from_fn(verify_api_gateway_key))
        .layer(axum::middleware::from_fn(payload_encryption));

    let mut router = Router::new().route("/health", axum::routing::get(health_check));

    // Serve OpenAPI document & Swagger UI automatically at /api-docs ONLY in non-production environments
    if app_env != "production" {
        router = router
            .merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", ApiDoc::openapi()));
    }

    router
        .merge(api_routes)
        // Add tracing/logging layer
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        // Injected via Extension so the JWT extractor can access AppConfig & AppState
        .layer(Extension(state.config.clone()))
        .layer(Extension(state.clone()))
        .with_state(state)
}

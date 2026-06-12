use axum::{
    routing::{get, post, put},
    Extension, Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::AppConfig;
use crate::services::{DynQueueService, UserService};
use crate::docs::ApiDoc;

pub mod handlers;
pub mod middleware;
use self::middleware as api_middleware;

/// Struct containing shared application dependencies.
#[derive(Clone)]
pub struct AppState {
    /// Service logic for user management & password validation.
    pub user_service: UserService,
    /// Publisher service for pushing jobs to RabbitMQ.
    pub queue_publisher: DynQueueService,
    /// Environment variables configuration.
    pub config: AppConfig,
}

/// Build the Axum Router configuring routes, CORS, logging, and Swagger UI.
pub fn create_router(state: AppState) -> Router {
    // Permissive CORS layer for template demo purposes
    let cors = CorsLayer::permissive();

    // Group user routes requiring valid JWT authentication
    let user_routes = Router::new()
        .route("/profile", get(handlers::get_profile_handler))
        // Apply RBAC: Only Admin can update role
        .route(
            "/:id/role",
            put(handlers::update_role_handler)
                .route_layer(axum::middleware::from_fn(api_middleware::require_admin)),
        );

    // Group job routes requiring valid JWT authentication
    let job_routes = Router::new().route("/", post(handlers::create_job_handler));

    // Aggregate routes, attach global middleware, inject config extension and state
    Router::new()
        // Serve OpenAPI document & Swagger UI automatically at /api/docs
        .merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", ApiDoc::openapi()))
        // Public Auth routes
        .route("/api/auth/register", post(handlers::register_handler))
        .route("/api/auth/login", post(handlers::login_handler))
        // Protected sub-routers
        .nest("/api/users", user_routes)
        .nest("/api/jobs", job_routes)
        // Add tracing/logging layer
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        // Injected via Extension so the JWT extractor can access AppConfig
        .layer(Extension(state.config.clone()))
        .with_state(state)
}

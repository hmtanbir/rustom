use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};
use crate::api::handlers;
use crate::domain;

/// Master OpenAPI structure aggregating all handlers, schemas, and security rules.
#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::auth::register_handler,
        handlers::auth::login_handler,
        handlers::user::get_profile_handler,
        handlers::user::update_role_handler,
        handlers::job::create_job_handler,
    ),
    components(
        schemas(
            domain::UserResponseDto,
            domain::UserRegisterRequestDto,
            domain::UserLoginRequestDto,
            domain::UserLoginResponseDto,
            domain::UserRole,
            domain::JobPayload,
            domain::CreateJobRequestDto,
            domain::CreateJobResponseDto,
            handlers::user::UpdateRoleRequestDto,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "Auth", description = "Authentication and Authorization Endpoints"),
        (name = "Users", description = "User management and profile actions"),
        (name = "Jobs", description = "Asynchronous queue triggers")
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearerAuth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("Input your raw JSON Web Token (JWT)".to_string()))
                        .build(),
                ),
            );
        }
    }
}

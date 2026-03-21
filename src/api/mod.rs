#![cfg(not(tarpaulin_include))]

/// Auth module
pub mod auth;
/// Auth middleware module
pub mod auth_middleware;
/// GitHub API module
pub mod github;
/// Orgs API module
pub mod orgs;
/// Repos API module
pub mod repos;
/// RPC module
pub mod rpc;

use actix_web::web;
use actix_web::{HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

/// The response payload for the version endpoint.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct VersionResponse {
    /// The current application version
    pub version: String,
}

/// Get API version
#[utoipa::path(
    get,
    path = "/version",
    responses(
        (status = 200, description = "Version info", body = VersionResponse)
    )
)]
pub async fn version() -> impl Responder {
    HttpResponse::Ok().json(VersionResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Configure all API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/version", web::get().to(version));
    auth::configure(cfg);
    github::configure(cfg);
    orgs::configure(cfg);
    repos::configure(cfg);
    rpc::configure(cfg);
}

/// OpenAPI schema definitions
#[derive(OpenApi)]
#[openapi(
    paths(
        version,
        auth::register,
        auth::login_password,
        auth::login_github,
        github::sync_github_data,
        github::webhook_receiver,
        github::create_release,
        github::trigger_action,
        github::create_secret,
        orgs::create_org,
        repos::create_repo
    ),
    components(
        schemas(
            VersionResponse,
            auth::AuthResponse,
            auth::LoginPayload,
            auth::RegisterPayload,
            auth::OAuthPayload,
            github::SyncStatus,
            github::WebhookResponse,
            github::TriggerWorkflowPayload,
            github::CreateSecretPayload,
            github::ReleasePayload,
            orgs::OrgPayload,
            repos::RepoPayload,
            crate::db::models::Organization,
            crate::db::models::Repository
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "cdd-ctl", description = "Control daemon for CDD JSON-RPC servers")
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            )
        }
    }
}

/// Helper function to create the Swagger UI instance
pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    #[actix_web::test]
    async fn test_version() {
        let app = test::init_service(App::new().route("/version", web::get().to(version))).await;
        let req = test::TestRequest::get().uri("/version").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}

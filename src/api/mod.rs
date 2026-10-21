#![deny(missing_docs, clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_for_each)]
//! HTTP API routing, OpenAPI 3.0 specification, Swagger UI, and health/version endpoints.

/// Authentication endpoints (`/auth/*`).
pub mod auth;
/// Bearer JWT authentication extractor middleware.
pub mod auth_middleware;
/// GitHub synchronization, webhooks, releases, actions, and secrets endpoints (`/github/*`).
pub mod github;
/// Organization management endpoints (`/orgs`).
pub mod orgs;
/// Repository management endpoints (`/repos`).
pub mod repos;
/// JSON-RPC 2.0 dispatcher endpoint (`/rpc`).
pub mod rpc;

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi, ToSchema,
};
use utoipa_swagger_ui::SwaggerUi;

/// Health check response payload returned by `GET /health`.
#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct HealthResponse {
    /// Service health status string (e.g. `"ok"`).
    pub status: String,
}

/// Version response payload returned by `GET /version` and JSON-RPC `version`.
#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct VersionResponse {
    /// Version of the `cdd-gateway` server binary.
    pub gateway: String,
    /// Version strings of ecosystem target tools keyed by tool name.
    pub ecosystem: HashMap<String, String>,
}

/// `OpenAPI` 3.0 specification generator for `cdd-gateway`.
#[derive(OpenApi)]
#[openapi(
    paths(
        health,
        version,
        auth::register,
        auth::login_password,
        auth::login_github,
        orgs::create_org,
        repos::create_repo,
        github::sync_github_data,
        github::webhook_receiver,
        github::create_release,
        github::trigger_action,
        github::create_secret,
        rpc::rpc_handler,
    ),
    components(
        schemas(
            HealthResponse,
            VersionResponse,
            auth::AuthResponse,
            auth::LoginPayload,
            auth::OAuthPayload,
            auth::RegisterPayload,
            orgs::CreateOrgPayload,
            repos::CreateRepoPayload,
            github::SyncStatus,
            github::WebhookResponse,
            github::TriggerWorkflowPayload,
            github::CreateSecretPayload,
            github::ReleasePayload,
            rpc::RpcRequest,
            rpc::RpcResponse,
            rpc::RpcError,
            crate::db::models::User,
            crate::db::models::Organization,
            crate::db::models::Repository,
            crate::db::models::Release,
            crate::db::models::OrgRole,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "cdd-gateway", description = "Ecosystem API and Control Plane")
    )
)]
pub struct ApiDoc;

/// `OpenAPI` security modifier that registers the `bearer_auth` JWT security scheme.
struct SecurityAddon;

impl Modify for SecurityAddon {
    /// Adds the HTTP Bearer JWT security scheme to `openapi.components`.
    ///
    /// # Arguments
    /// * `&self` - The security addon modifier instance.
    /// * `openapi` - Mutable `OpenAPI` specification being modified.
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
            );
        }
    }
}

/// Returns a `200 OK` health status payload (`GET /health`).
///
/// # Returns
/// * `impl Responder` - `200 OK` JSON response containing [`HealthResponse`].
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
pub async fn health() -> impl Responder {
    HttpResponse::Ok().json(HealthResponse {
        status: "ok".to_string(),
    })
}

/// Returns the gateway version and ecosystem compiler versions (`GET /version`).
///
/// # Returns
/// * `impl Responder` - `200 OK` JSON response containing [`VersionResponse`].
#[utoipa::path(
    get,
    path = "/version",
    responses(
        (status = 200, description = "Service and ecosystem versions", body = VersionResponse)
    )
)]
pub async fn version() -> impl Responder {
    let mut ecosystem = HashMap::new();
    for tool in &[
        "cdd-c",
        "cdd-cpp",
        "cdd-csharp",
        "cdd-go",
        "cdd-java",
        "cdd-kotlin",
        "cdd-php",
        "cdd-python-all",
        "cdd-ruby",
        "cdd-rust",
        "cdd-sh",
        "cdd-swift",
        "cdd-ts",
    ] {
        ecosystem.insert((*tool).to_string(), "0.0.1".to_string());
    }

    HttpResponse::Ok().json(VersionResponse {
        gateway: env!("CARGO_PKG_VERSION").to_string(),
        ecosystem,
    })
}

/// Registers all API routes, MCP routes, and Swagger UI on `cfg`.
///
/// # Arguments
/// * `cfg` - Actix-web service configuration builder.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health))
        .route("/version", web::get().to(version))
        .configure(auth::configure)
        .configure(orgs::configure)
        .configure(repos::configure)
        .configure(github::configure)
        .configure(rpc::configure)
        .configure(crate::mcp::configure)
        .service(swagger_ui());
}

/// Constructs the [`SwaggerUi`] service serving `/swagger-ui/*` and `/api-docs/openapi.json`.
///
/// # Returns
/// * [`SwaggerUi`] - Configured Actix-web Swagger UI service.
#[must_use]
pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    #[actix_web::test]
    async fn test_health() {
        let app = test::init_service(App::new().route("/health", web::get().to(health))).await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp: HealthResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.status, "ok");
    }

    #[actix_web::test]
    async fn test_version() {
        let app = test::init_service(App::new().route("/version", web::get().to(version))).await;
        let req = test::TestRequest::get().uri("/version").to_request();
        let resp: VersionResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.gateway, "0.0.1");
        assert!(resp.ecosystem.contains_key("cdd-rust"));
    }

    #[actix_web::test]
    async fn test_configure_and_swagger_ui() {
        let app = test::init_service(App::new().configure(configure)).await;
        let req_health = test::TestRequest::get().uri("/health").to_request();
        let resp_health = test::call_service(&app, req_health).await;
        assert!(resp_health.status().is_success());

        let req_openapi = test::TestRequest::get()
            .uri("/api-docs/openapi.json")
            .to_request();
        let resp_openapi = test::call_service(&app, req_openapi).await;
        assert!(resp_openapi.status().is_success());
    }

    #[actix_web::test]
    async fn test_security_addon_none() {
        let mut openapi = utoipa::openapi::OpenApi::new(
            utoipa::openapi::Info::new("test", "1.0"),
            utoipa::openapi::Paths::new(),
        );
        openapi.components = None;
        let addon = SecurityAddon;
        addon.modify(&mut openapi);
        assert!(openapi.components.is_none());
    }
}

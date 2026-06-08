import os

content = r"""use actix_web::web;

pub mod auth;
pub mod docs;
pub mod github;
pub mod projects;
pub mod system;
pub mod users;
pub mod mcp;

pub use auth::*;
pub use docs::*;
pub use github::*;
pub use projects::*;
pub use system::*;
pub use users::*;
pub use mcp::*;

/// Configure the unified API routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .service(
                web::scope("/auth")
                    .route("/register", web::post().to(auth::register))
                    .route("/login", web::post().to(auth::login))
                    .route("/github", web::get().to(github::github_auth))
                    .route("/github/callback", web::get().to(github::github_callback)),
            )
            .service(
                web::scope("/users")
                    .route("/me", web::get().to(users::get_me))
                    .route("/me", web::put().to(users::update_me))
                    .route("/me/github", web::delete().to(users::unlink_github)),
            )
            .service(
                web::scope("/projects")
                    .route("", web::get().to(projects::list_projects))
                    .route("", web::post().to(projects::create_project))
                    .route("/{id}", web::get().to(projects::get_project))
                    .route("/{id}", web::put().to(projects::update_project))
                    .route("/{id}", web::delete().to(projects::delete_project))
                    .route(
                        "/{id}/generate",
                        web::post().to(projects::generate_project),
                    )
                    .route("/{id}/deploy", web::post().to(projects::deploy_project)),
            )
            .service(web::scope("/system").route("/health", web::get().to(system::health_check))),
    )
    .service(
        web::scope("/mcp")
            .route("/sse", web::get().to(crate::mcp::mcp_sse_handshake))
            .route("/message", web::post().to(crate::mcp::mcp_message_handler)),
    );
}
"""

with open(os.path.expanduser("~/repos/cdd-gateway/src/api/mod.rs"), "w") as f:
    f.write(content)

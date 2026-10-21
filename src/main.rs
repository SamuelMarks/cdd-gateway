#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![deny(missing_docs, clippy::missing_docs_in_private_items)]

//! `cdd-gateway` binary executable.
//!
//! Acts as the entry point for the reverse proxy, control plane API, JSON-RPC 2.0 server,
//! and Model Context Protocol (MCP) transport.

use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};
use cdd_gateway::{
    config::AppConfig,
    db::{
        establish_connection_pool,
        repository::{CddRepository, OfflineRepository, PgRepository},
    },
    error::CddGatewayError,
    github::client::{GitHubClient, ReqwestGitHubClient},
    proxy::proxy_handler,
    rate_limit::RateLimit,
};
use log::info;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;

/// Main entry point for the `cdd-gateway` server.
///
/// # Returns
/// * `Ok(())` - When the HTTP server shuts down gracefully.
///
/// # Errors
/// * [`CddGatewayError`] - If configuration loading, database pool initialization, or socket binding fails.
#[cfg(not(tarpaulin_include))]
#[cfg_attr(coverage_nightly, coverage(off))]
#[actix_web::main]
async fn main() -> Result<(), CddGatewayError> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let config_path = std::env::var("CDD_CONFIG").ok();
    let config = AppConfig::load(config_path.as_deref())?;

    let server_bind = config.server_bind.clone();
    info!("Starting cdd-gateway on {server_bind}");

    let repo_data: Arc<dyn CddRepository> = if config.offline_mode {
        info!("Running in offline mode with in-memory repository");
        Arc::new(OfflineRepository::new())
    } else {
        let pool = establish_connection_pool(&config.database_url)?;
        Arc::new(PgRepository { pool })
    };

    let mut gh_client = ReqwestGitHubClient::new(
        config.github_client_id.clone(),
        config.github_client_secret.clone(),
    )?;
    gh_client
        .api_base_url
        .clone_from(&config.github_api_base_url);
    gh_client
        .github_base_url
        .clone_from(&config.github_html_base_url);
    let github_client: Arc<dyn GitHubClient> = Arc::new(gh_client);

    let reqwest_client = Client::builder().build()?;

    let config_data = web::Data::new(config.clone());
    let reqwest_data = web::Data::new(reqwest_client);
    let repo_app_data = web::Data::new(repo_data);
    let gh_app_data = web::Data::new(github_client);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(RateLimit::new(
                config.rate_limit_max_requests,
                Duration::from_secs(config.rate_limit_window_secs),
            ))
            .wrap(cors)
            .wrap(Logger::default())
            .app_data(config_data.clone())
            .app_data(reqwest_data.clone())
            .app_data(repo_app_data.clone())
            .app_data(gh_app_data.clone())
            .configure(cdd_gateway::api::configure)
            .default_service(web::route().to(proxy_handler))
    })
    .bind(&server_bind)?
    .run()
    .await?;

    Ok(())
}

#![cfg(not(tarpaulin_include))]
#![deny(missing_docs)]

//! cdd-gateway binary executable.
//!
//! This module acts as the entry point for the reverse proxy and ingress controller.

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use cdd_gateway::config::AppConfig;
use cdd_gateway::proxy::proxy_handler;
use cdd_gateway::rate_limit::RateLimit;
use reqwest::Client;
use std::env;
use std::time::Duration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize env_logger for structured logging/tracing
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Load configuration
    let config_path = env::var("CDD_CONFIG_PATH").ok();
    let config = match AppConfig::load(config_path.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to load configuration: {e}");
            std::process::exit(1);
        }
    };

    let server_bind = config.server_bind.clone();

    // Create Reqwest client for proxying
    let client = match Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to build reqwest client: {e}");
            std::process::exit(1);
        }
    };

    let config_data = web::Data::new(config.clone());
    let client_data = web::Data::new(client);

    log::info!("Starting CDD Gateway on {server_bind}");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin() // Should be strict in prod, but for proxy it's complex
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::ACCEPT,
            ])
            .allowed_header(actix_web::http::header::CONTENT_TYPE)
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(RateLimit::new(100, Duration::from_mins(1)))
            // Structured tracing/logging for incoming requests
            .wrap(middleware::Logger::default())
            .app_data(config_data.clone())
            .app_data(client_data.clone())
            // Existing API routes
            .configure(cdd_gateway::api::configure)
            // Unmatched routes are proxied
            .default_service(web::route().to(proxy_handler))
    })
    .bind(&server_bind)?
    .run()
    .await
}

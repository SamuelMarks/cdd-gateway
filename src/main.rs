#![warn(missing_docs)]

//! cdd-ctl: Daemon manage >13 processes and act as API gateway and authentication layer.
#![allow(unused_imports)]
#![allow(missing_docs)]

pub mod api;
pub mod config;
pub mod daemon;
pub mod db;
pub mod github;

use actix_web::{web, App, HttpServer};
use clap::Parser;
use log::{error, info};
use std::sync::Arc;

use crate::config::AppConfig;
use crate::daemon::ProcessManager;
use crate::db::repository::{CddRepository, PgRepository};
use crate::github::client::{GitHubClient, ReqwestGitHubClient};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file (JSON/YAML/TOML)
    #[arg(short, long)]
    config: Option<String>,

    /// Override the bind address
    #[arg(short, long)]
    bind: Option<String>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let args = Args::parse();

    let mut app_config = match AppConfig::load(args.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    if let Some(bind) = args.bind {
        app_config.server_bind = bind;
    }

    info!("Starting cdd-ctl server on {}", app_config.server_bind);

    let process_manager = Arc::new(ProcessManager::new(app_config.servers.clone()));

    let pm_clone = process_manager.clone();
    if let Err(e) = pm_clone.start_all().await {
        error!("Error starting processes: {}", e);
    }

    // Connect to PG Database
    let pool = db::establish_connection_pool(&app_config.database_url);
    let repo = Arc::new(PgRepository { pool });

    // Configure GitHub Client
    let github_client = Arc::new(ReqwestGitHubClient::new(
        std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
        std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
    ));

    let bind_addr = app_config.server_bind.clone();

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(repo.clone() as Arc<dyn CddRepository>))
            .app_data(web::Data::new(
                github_client.clone() as Arc<dyn GitHubClient>
            ))
            .configure(api::configure)
            .service(api::swagger_ui())
    })
    .bind(&bind_addr)?
    .run();

    let result = server.await;

    // Shutdown processes
    process_manager.stop_all().await;

    result
}

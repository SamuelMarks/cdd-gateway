#![cfg(not(tarpaulin_include))]
#![warn(missing_docs)]

//! cdd-ctl: Daemon manage >13 processes and act as API gateway and authentication layer.
#![allow(unused_imports)]

use actix_web::{web, App, HttpServer};
use cdd_ctl::{api, db};
use clap::Parser;
use log::{error, info};
use std::sync::Arc;

use cdd_ctl::AppConfig;
use cdd_ctl::{CddRepository, PgRepository};
use cdd_ctl::{GitHubClient, ReqwestGitHubClient};
use cdd_ctl::{ProcessConfig, ProcessManager};

#[derive(Parser, Debug)]
#[command(name = "cdd-rpc", author, version, about, long_about = None)]
/// Command line arguments
struct Args {
    /// Path to configuration file (JSON/YAML/TOML)
    #[arg(short, long)]
    /// Path to configuration file (JSON/YAML/TOML)
    config: Option<String>,

    /// Override the bind address
    #[arg(short, long)]
    /// Override the bind address
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

    if app_config.servers.is_empty() {
        info!("No servers configured, populating with default native dependencies.");
        let native_tools = [
            "cdd-c",
            "cdd-cpp",
            "cdd-csharp",
            "cdd-go",
            "cdd-java",
            "cdd-kotlin",
            "cdd-php",
            "cdd-python",
            "cdd-ruby",
            "cdd-rust",
            "cdd-sh",
            "cdd-swift",
            "cdd-ts",
        ];
        for tool in native_tools {
            app_config.servers.insert(
                tool.to_string(),
                cdd_ctl::ProcessConfig {
                    command: Some(tool.to_string()),
                    args: Some(vec![]),
                    external_address: None,
                    max_retries: 5,
                    restart_delay_ms: 2000,
                },
            );
        }
    }

    info!("Starting cdd-rpc server on {}", app_config.server_bind);

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

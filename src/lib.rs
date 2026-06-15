#![deny(missing_docs)]

//! cdd-gateway core library

/// API modules
pub mod api;
/// Configuration module
pub mod config;
/// Database module
pub mod db;
/// Error module
pub mod error;
/// GitHub module
pub mod github;
/// Proxy module
pub mod proxy;
/// Rate limiting module
pub mod rate_limit;

pub use config::AppConfig;
pub use db::repository::{CddRepository, PgRepository};
pub use error::CddGatewayError;
pub use github::client::{GitHubClient, ReqwestGitHubClient};
/// MCP Transport Module
pub mod mcp;

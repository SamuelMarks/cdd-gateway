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

pub use config::AppConfig;
pub use db::repository::{CddRepository, PgRepository};
pub use error::CddError;
pub use github::client::{GitHubClient, ReqwestGitHubClient};

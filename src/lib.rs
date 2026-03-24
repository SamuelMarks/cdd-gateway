#![deny(missing_docs)]


//! cdd-ctl core library

/// API modules
pub mod api;
/// Configuration module
pub mod config;
/// Daemon module
pub mod daemon;
/// Database module
pub mod db;
/// GitHub module
pub mod github;

pub use config::AppConfig;
pub use daemon::{ProcessConfig, ProcessManager};
pub use db::repository::{CddRepository, PgRepository};
pub use github::client::{GitHubClient, ReqwestGitHubClient};

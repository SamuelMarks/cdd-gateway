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

/// Error module
pub mod error;

pub use config::AppConfig;
pub use daemon::{ProcessConfig, ProcessManager};
pub use db::repository::{CddRepository, PgRepository};
pub use error::CddError;
pub use github::client::{GitHubClient, ReqwestGitHubClient};
/// GraalVM WASM Linker mock state
pub mod graalvm_linker;
/// WASM execution orchestration
pub mod wasm_executor;

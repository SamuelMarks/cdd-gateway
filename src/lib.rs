#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! `cdd-gateway` core library.
//!
//! Provides HTTP API handlers, database repositories, GitHub client, JSON-RPC, MCP,
//! reverse proxy, and rate limiting.

/// HTTP API route handlers, OpenAPI specification, and authentication middleware.
pub mod api;
/// Application configuration loader and execution mode types.
pub mod config;
/// Database models, schema, connection pool, and repository traits.
pub mod db;
/// Central unified error enumeration (`CddGatewayError`).
pub mod error;
/// GitHub OAuth and REST API v3 client and models.
pub mod github;
/// Model Context Protocol (MCP) Server-Sent Events transport.
pub mod mcp;
/// Reverse proxy handler for upstream UI and control-plane services.
pub mod proxy;
/// Sliding-window per-IP rate limiting middleware.
pub mod rate_limit;

pub use config::{AppConfig, ExecutionMode};
pub use db::repository::{CddRepository, OfflineRepository, PgRepository};
pub use error::CddGatewayError;
pub use github::client::{GitHubClient, ReqwestGitHubClient};

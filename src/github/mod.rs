#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! GitHub REST API and OAuth integration module.
//!
//! Exposes [`client`] for interacting with GitHub's REST and OAuth endpoints and
//! [`models`] for typed serialization and deserialization of GitHub resources.

/// GitHub API client trait and `reqwest`-based implementation.
pub mod client;
/// Data transfer structures representing GitHub API resources.
pub mod models;

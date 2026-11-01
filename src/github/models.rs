#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Data transfer models for GitHub REST API and OAuth responses.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents an authenticated GitHub user profile returned by `GET /user`.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, ToSchema)]
pub struct GitHubUser {
    /// Unique numeric GitHub user ID.
    pub id: i64,
    /// GitHub account login handle (username).
    pub login: String,
    /// URL pointing to the user's profile avatar image.
    pub avatar_url: String,
}

/// Represents a user email entry returned by `GET /user/emails`.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, ToSchema)]
pub struct GitHubEmail {
    /// Email address string.
    pub email: String,
    /// Whether this address is the user's primary GitHub email.
    pub primary: bool,
    /// Whether this email address has been verified by GitHub.
    pub verified: bool,
}

/// Represents a GitHub organization returned by `GET /user/orgs`.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, ToSchema)]
pub struct GitHubOrg {
    /// Unique numeric GitHub organization ID.
    pub id: i64,
    /// Organization login handle.
    pub login: String,
    /// Optional human-readable description of the organization.
    pub description: Option<String>,
}

/// Represents a GitHub repository returned by the GitHub REST API.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, ToSchema)]
pub struct GitHubRepo {
    /// Unique numeric GitHub repository ID.
    pub id: i64,
    /// Short repository name.
    pub name: String,
    /// Full `owner/name` repository identifier.
    pub full_name: String,
    /// Optional human-readable description of the repository.
    pub description: Option<String>,
}

/// Represents a GitHub release returned by `POST /repos/{owner}/{repo}/releases`.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, ToSchema)]
pub struct GitHubRelease {
    /// Unique numeric GitHub release ID.
    pub id: i64,
    /// Git tag name associated with the release.
    pub tag_name: String,
    /// Optional release title.
    pub name: Option<String>,
    /// Optional release notes markdown body.
    pub body: Option<String>,
}

/// Represents a repository Libsodium public key returned by
/// `GET /repos/{owner}/{repo}/actions/secrets/public-key`.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, ToSchema)]
pub struct GitHubPublicKey {
    /// Identifier of the public key used when uploading encrypted secrets.
    pub key_id: String,
    /// Base64-encoded public key value.
    pub key: String,
}

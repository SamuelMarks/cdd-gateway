#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Database models and strong domain types for `PostgreSQL` persistence.
//!
//! Contains Diesel [`Queryable`], [`Selectable`], and [`Insertable`] structures
//! representing users, organizations, organization memberships, repositories, and releases,
//! as well as the strongly-typed [`OrgRole`] enumeration.

use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use utoipa::ToSchema;

use crate::db::schema::{organization_users, organizations, releases, repositories, users};
use crate::error::CddGatewayError;

/// Role assigned to a user within an organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum OrgRole {
    /// Full administrative ownership over the organization and its repositories.
    Owner,
    /// Administrative access to manage organization resources.
    Admin,
    /// Standard member access within the organization.
    Member,
}

impl OrgRole {
    /// Returns the canonical lowercase string representation of the role.
    ///
    /// # Arguments
    /// * `&self` - The role enum variant.
    ///
    /// # Returns
    /// * `&'static str` - The string slice `"owner"`, `"admin"`, or `"member"`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }
}

impl fmt::Display for OrgRole {
    /// Formats the [`OrgRole`] as its canonical lowercase string.
    ///
    /// # Arguments
    /// * `&self` - The role being formatted.
    /// * `f` - The formatter buffer.
    ///
    /// # Returns
    /// * `fmt::Result` - Result of writing to the formatter.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for OrgRole {
    type Err = CddGatewayError;

    /// Parses an [`OrgRole`] from a case-insensitive string slice.
    ///
    /// # Arguments
    /// * `s` - The input role string (`"owner"`, `"admin"`, or `"member"`).
    ///
    /// # Returns
    /// * `Ok(OrgRole)` - The parsed organization role.
    ///
    /// # Errors
    /// * [`CddGatewayError::Validation`] - If the string does not match a known role.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            other => Err(CddGatewayError::Validation(format!(
                "Invalid organization role: {other}"
            ))),
        }
    }
}

/// User model representing an authenticated local user or synced GitHub user.
#[derive(
    Debug,
    Clone,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Serialize,
    Deserialize,
    ToSchema,
    PartialEq,
    Eq,
)]
#[diesel(table_name = users)]
pub struct User {
    /// Primary key identifier of the user.
    pub id: i32,
    /// Optional GitHub numeric user ID.
    pub github_id: Option<i64>,
    /// Unique login username.
    pub username: String,
    /// User's primary email address.
    pub email: String,
    /// Optional Argon2 password hash (never serialized in API responses).
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
}

/// Insertable payload for creating a new [`User`] row.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    /// Optional GitHub numeric user ID.
    pub github_id: Option<i64>,
    /// Unique login username.
    pub username: &'a str,
    /// User's primary email address.
    pub email: &'a str,
    /// Optional Argon2 password hash.
    pub password_hash: Option<&'a str>,
}

/// Organization model representing a logical grouping of users and repositories.
#[derive(
    Debug,
    Clone,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Serialize,
    Deserialize,
    ToSchema,
    PartialEq,
    Eq,
)]
#[diesel(table_name = organizations)]
pub struct Organization {
    /// Primary key identifier of the organization.
    pub id: i32,
    /// Optional GitHub numeric organization ID.
    pub github_id: Option<i64>,
    /// Unique organization login handle.
    pub login: String,
    /// Optional human-readable description of the organization.
    pub description: Option<String>,
}

/// Insertable payload for creating a new [`Organization`] row.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = organizations)]
pub struct NewOrganization<'a> {
    /// Optional GitHub numeric organization ID.
    pub github_id: Option<i64>,
    /// Unique organization login handle.
    pub login: &'a str,
    /// Optional human-readable description of the organization.
    pub description: Option<&'a str>,
}

/// Join record linking a [`User`] to an [`Organization`] with a specific role.
#[derive(
    Debug,
    Clone,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Serialize,
    Deserialize,
    ToSchema,
    PartialEq,
    Eq,
)]
#[diesel(table_name = organization_users)]
#[diesel(primary_key(organization_id, user_id))]
pub struct OrganizationUser {
    /// Foreign key referencing [`Organization::id`].
    pub organization_id: i32,
    /// Foreign key referencing [`User::id`].
    pub user_id: i32,
    /// Assigned role name (e.g. `"owner"`, `"admin"`, `"member"`).
    pub role: String,
}

/// Insertable payload for linking a user to an organization.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = organization_users)]
pub struct NewOrganizationUser<'a> {
    /// Foreign key referencing [`Organization::id`].
    pub organization_id: i32,
    /// Foreign key referencing [`User::id`].
    pub user_id: i32,
    /// Assigned role string slice.
    pub role: &'a str,
}

/// Repository model representing an SDK or code repository owned by an organization.
#[derive(
    Debug,
    Clone,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Serialize,
    Deserialize,
    ToSchema,
    PartialEq,
    Eq,
)]
#[diesel(table_name = repositories)]
pub struct Repository {
    /// Primary key identifier of the repository.
    pub id: i32,
    /// Foreign key referencing the owning [`Organization::id`].
    pub organization_id: i32,
    /// Optional GitHub numeric repository ID.
    pub github_id: Option<i64>,
    /// Repository name within the organization.
    pub name: String,
    /// Optional human-readable description of the repository.
    pub description: Option<String>,
}

/// Insertable payload for creating a new [`Repository`] row.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = repositories)]
pub struct NewRepository<'a> {
    /// Foreign key referencing the owning [`Organization::id`].
    pub organization_id: i32,
    /// Optional GitHub numeric repository ID.
    pub github_id: Option<i64>,
    /// Repository name within the organization.
    pub name: &'a str,
    /// Optional human-readable description of the repository.
    pub description: Option<&'a str>,
}

/// Release model representing a versioned tag/release of a [`Repository`].
#[derive(
    Debug,
    Clone,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Serialize,
    Deserialize,
    ToSchema,
    PartialEq,
    Eq,
)]
#[diesel(table_name = releases)]
pub struct Release {
    /// Primary key identifier of the release.
    pub id: i32,
    /// Foreign key referencing [`Repository::id`].
    pub repository_id: i32,
    /// Optional GitHub numeric release ID.
    pub github_id: Option<i64>,
    /// Git tag name associated with the release (e.g. `"v1.0.0"`).
    pub tag_name: String,
    /// Optional release title.
    pub name: Option<String>,
    /// Optional release notes / changelog body.
    pub body: Option<String>,
}

/// Insertable payload for creating a new [`Release`] row.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = releases)]
pub struct NewRelease<'a> {
    /// Foreign key referencing [`Repository::id`].
    pub repository_id: i32,
    /// Optional GitHub numeric release ID.
    pub github_id: Option<i64>,
    /// Git tag name associated with the release.
    pub tag_name: &'a str,
    /// Optional release title.
    pub name: Option<&'a str>,
    /// Optional release notes / changelog body.
    pub body: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_org_role_display_and_from_str() -> Result<(), CddGatewayError> {
        assert_eq!(OrgRole::Owner.as_str(), "owner");
        assert_eq!(OrgRole::Admin.as_str(), "admin");
        assert_eq!(OrgRole::Member.as_str(), "member");

        assert_eq!(OrgRole::Owner.to_string(), "owner");
        assert_eq!(OrgRole::Admin.to_string(), "admin");
        assert_eq!(OrgRole::Member.to_string(), "member");

        assert_eq!("owner".parse::<OrgRole>()?, OrgRole::Owner);
        assert_eq!("ADMIN".parse::<OrgRole>()?, OrgRole::Admin);
        assert_eq!("Member".parse::<OrgRole>()?, OrgRole::Member);
        assert!("guest".parse::<OrgRole>().is_err());
        Ok(())
    }
}

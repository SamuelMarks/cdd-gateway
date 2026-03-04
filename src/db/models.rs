use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::schema::*;

/// User model representing an authenticated user or synced GitHub user.
#[derive(
    Debug, Clone, Queryable, Selectable, Insertable, Serialize, Deserialize, ToSchema, PartialEq,
)]
#[diesel(table_name = users)]
pub struct User {
    /// Internal DB ID
    pub id: i32,
    /// GitHub ID, if applicable
    pub github_id: Option<i64>,
    /// Username
    pub username: String,
    /// Email
    pub email: String,
    /// Password hash (internal)
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
}

/// New user payload
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    /// Optional GitHub ID
    pub github_id: Option<i64>,
    /// Username
    pub username: &'a str,
    /// Email
    pub email: &'a str,
    /// Hashed password
    pub password_hash: Option<&'a str>,
}

/// Organization model synced from GitHub or created locally
#[derive(
    Debug, Clone, Queryable, Selectable, Insertable, Serialize, Deserialize, ToSchema, PartialEq,
)]
#[diesel(table_name = organizations)]
pub struct Organization {
    /// Internal DB ID
    pub id: i32,
    /// GitHub ID
    pub github_id: Option<i64>,
    /// Login or name
    pub login: String,
    /// Optional description
    pub description: Option<String>,
}

/// Payload for creating an Organization
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = organizations)]
pub struct NewOrganization<'a> {
    /// Optional GitHub ID
    pub github_id: Option<i64>,
    /// Login or name
    pub login: &'a str,
    /// Optional description
    pub description: Option<&'a str>,
}

/// Maps a User to an Organization with a specific role
#[derive(
    Debug, Clone, Queryable, Selectable, Insertable, Serialize, Deserialize, ToSchema, PartialEq,
)]
#[diesel(table_name = organization_users)]
pub struct OrganizationUser {
    /// Organization ID
    pub organization_id: i32,
    /// User ID
    pub user_id: i32,
    /// Role (e.g., owner, member)
    pub role: String,
}

/// Payload for creating an Organization-User link
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = organization_users)]
pub struct NewOrganizationUser<'a> {
    /// Organization ID
    pub organization_id: i32,
    /// User ID
    pub user_id: i32,
    /// Role
    pub role: &'a str,
}

/// Repository model representing SDKs
#[derive(
    Debug, Clone, Queryable, Selectable, Insertable, Serialize, Deserialize, ToSchema, PartialEq,
)]
#[diesel(table_name = repositories)]
pub struct Repository {
    /// Internal DB ID
    pub id: i32,
    /// Organization ID
    pub organization_id: i32,
    /// Optional GitHub ID
    pub github_id: Option<i64>,
    /// Name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
}

/// Payload for creating a Repository
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = repositories)]
pub struct NewRepository<'a> {
    /// Organization ID
    pub organization_id: i32,
    /// Optional GitHub ID
    pub github_id: Option<i64>,
    /// Name
    pub name: &'a str,
    /// Optional description
    pub description: Option<&'a str>,
}

/// Release model for a given repository
#[derive(
    Debug, Clone, Queryable, Selectable, Insertable, Serialize, Deserialize, ToSchema, PartialEq,
)]
#[diesel(table_name = releases)]
pub struct Release {
    /// Internal DB ID
    pub id: i32,
    /// Repository ID
    pub repository_id: i32,
    /// Optional GitHub ID
    pub github_id: Option<i64>,
    /// Tag name
    pub tag_name: String,
    /// Name
    pub name: Option<String>,
    /// Release body/notes
    pub body: Option<String>,
}

/// Payload for creating a Release
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = releases)]
pub struct NewRelease<'a> {
    /// Repository ID
    pub repository_id: i32,
    /// Optional GitHub ID
    pub github_id: Option<i64>,
    /// Tag name
    pub tag_name: &'a str,
    /// Name
    pub name: Option<&'a str>,
    /// Body
    pub body: Option<&'a str>,
}

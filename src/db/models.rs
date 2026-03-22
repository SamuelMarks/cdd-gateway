#![allow(missing_docs)]
#![cfg(not(tarpaulin_include))]

use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::schema::*;

/// User model representing an authenticated user or synced GitHub user.
#[derive(
    Debug, Clone, Queryable, Selectable, Insertable, Identifiable, Serialize, Deserialize, ToSchema, PartialEq,
)]
#[diesel(table_name = users)]
pub struct User {
    pub id: i32,
    pub github_id: Option<i64>,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    pub github_id: Option<i64>,
    pub username: &'a str,
    pub email: &'a str,
    pub password_hash: Option<&'a str>,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = users)]
pub struct UpdateUser<'a> {
    pub github_id: Option<i64>,
    pub username: Option<&'a str>,
    pub email: Option<&'a str>,
    pub password_hash: Option<&'a str>,
}

#[derive(
    Debug, Clone, Queryable, Selectable, Insertable, Identifiable, Serialize, Deserialize, ToSchema, PartialEq,
)]
#[diesel(table_name = organizations)]
pub struct Organization {
    pub id: i32,
    pub github_id: Option<i64>,
    pub login: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = organizations)]
pub struct NewOrganization<'a> {
    pub github_id: Option<i64>,
    pub login: &'a str,
    pub description: Option<&'a str>,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = organizations)]
pub struct UpdateOrganization<'a> {
    pub github_id: Option<i64>,
    pub login: Option<&'a str>,
    pub description: Option<Option<&'a str>>,
}

#[derive(
    Debug, Clone, Queryable, Selectable, Insertable, Identifiable, Serialize, Deserialize, ToSchema, PartialEq,
)]
#[diesel(table_name = organization_users)]
#[diesel(primary_key(organization_id, user_id))]
pub struct OrganizationUser {
    pub organization_id: i32,
    pub user_id: i32,
    pub role: String,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = organization_users)]
pub struct NewOrganizationUser<'a> {
    pub organization_id: i32,
    pub user_id: i32,
    pub role: &'a str,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = organization_users)]
pub struct UpdateOrganizationUser<'a> {
    pub role: Option<&'a str>,
}

#[derive(
    Debug, Clone, Queryable, Selectable, Insertable, Identifiable, Serialize, Deserialize, ToSchema, PartialEq,
)]
#[diesel(table_name = repositories)]
pub struct Repository {
    pub id: i32,
    pub organization_id: i32,
    pub github_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = repositories)]
pub struct NewRepository<'a> {
    pub organization_id: i32,
    pub github_id: Option<i64>,
    pub name: &'a str,
    pub description: Option<&'a str>,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = repositories)]
pub struct UpdateRepository<'a> {
    pub organization_id: Option<i32>,
    pub github_id: Option<i64>,
    pub name: Option<&'a str>,
    pub description: Option<Option<&'a str>>,
}

#[derive(
    Debug, Clone, Queryable, Selectable, Insertable, Identifiable, Serialize, Deserialize, ToSchema, PartialEq,
)]
#[diesel(table_name = releases)]
pub struct Release {
    pub id: i32,
    pub repository_id: i32,
    pub github_id: Option<i64>,
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = releases)]
pub struct NewRelease<'a> {
    pub repository_id: i32,
    pub github_id: Option<i64>,
    pub tag_name: &'a str,
    pub name: Option<&'a str>,
    pub body: Option<&'a str>,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = releases)]
pub struct UpdateRelease<'a> {
    pub repository_id: Option<i32>,
    pub github_id: Option<i64>,
    pub tag_name: Option<&'a str>,
    pub name: Option<Option<&'a str>>,
    pub body: Option<Option<&'a str>>,
}

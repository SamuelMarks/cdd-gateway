#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Database repository trait and concrete implementations (`PgRepository` and `OfflineRepository`).
//!
//! Abstracts all persistence operations behind [`CddRepository`] returning [`CddGatewayError`],
//! backed either by `PostgreSQL` via [`PgRepository`] or by in-memory storage via [`OfflineRepository`].

use crate::db::models::{
    NewOrganization, NewOrganizationUser, NewRelease, NewRepository, NewUser, Organization,
    OrganizationUser, Release, Repository, RepositoryConfigMetadata, User,
};
use crate::db::schema::{organization_users, organizations, releases, repositories, users};
use crate::error::CddGatewayError;
use actix_web::web;
use async_trait::async_trait;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use std::sync::{Arc, Mutex};

/// Database repository trait for handling CRUD and upsert operations.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CddRepository: Send + Sync {
    /// Finds a user by their unique username.
    ///
    /// # Arguments
    /// * `username` - The login username to search for.
    ///
    /// # Returns
    /// * `Ok(Some(User))` - If a matching user exists.
    /// * `Ok(None)` - If no user matches `username`.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn find_user_by_username(
        &self,
        username: String,
    ) -> Result<Option<User>, CddGatewayError>;

    /// Finds a user by their primary email address.
    ///
    /// # Arguments
    /// * `email` - The email address to search for.
    ///
    /// # Returns
    /// * `Ok(Some(User))` - If a matching user exists.
    /// * `Ok(None)` - If no user matches `email`.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn find_user_by_email(&self, email: String) -> Result<Option<User>, CddGatewayError>;

    /// Finds a user by their Google OAuth subject identifier (`sub`).
    ///
    /// # Arguments
    /// * `google_id` - The Google OAuth subject ID to search for.
    ///
    /// # Returns
    /// * `Ok(Some(User))` - If a matching user exists.
    /// * `Ok(None)` - If no user matches `google_id`.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn find_user_by_google_id(
        &self,
        google_id: String,
    ) -> Result<Option<User>, CddGatewayError>;

    /// Finds a user by their numeric primary key ID.
    ///
    /// # Arguments
    /// * `id` - The numeric user ID to search for.
    ///
    /// # Returns
    /// * `Ok(Some(User))` - If a matching user exists.
    /// * `Ok(None)` - If no user matches `id`.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn find_user_by_id(&self, id: i32) -> Result<Option<User>, CddGatewayError>;

    /// Creates a new user record.
    ///
    /// # Arguments
    /// * `github_id` - Optional GitHub numeric user ID.
    /// * `username` - Unique login username.
    /// * `email` - Primary email address.
    /// * `password_hash` - Optional Argon2 password hash.
    ///
    /// # Returns
    /// * `Ok(User)` - The newly created user record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or constraint violation.
    async fn create_user(
        &self,
        github_id: Option<i64>,
        username: String,
        email: String,
        password_hash: Option<String>,
    ) -> Result<User, CddGatewayError>;

    /// Inserts or updates a user keyed by their GitHub numeric ID.
    ///
    /// # Arguments
    /// * `github_id` - GitHub numeric user ID.
    /// * `username` - Updated login username.
    /// * `email` - Updated primary email address.
    ///
    /// # Returns
    /// * `Ok(User)` - The inserted or updated user record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn upsert_user(
        &self,
        github_id: i64,
        username: String,
        email: String,
    ) -> Result<User, CddGatewayError>;

    /// Inserts or updates a user keyed by their Google OAuth subject ID (`google_id`).
    ///
    /// # Arguments
    /// * `google_id` - Google OAuth subject identifier (`sub`).
    /// * `username` - Login username.
    /// * `email` - Primary email address.
    ///
    /// # Returns
    /// * `Ok(User)` - The inserted or updated user record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn upsert_google_user(
        &self,
        google_id: String,
        username: String,
        email: String,
    ) -> Result<User, CddGatewayError>;

    /// Links a GitHub numeric ID to an existing user account.
    ///
    /// # Arguments
    /// * `user_id` - Primary key ID of the user to update.
    /// * `github_id` - GitHub numeric user ID to attach.
    ///
    /// # Returns
    /// * `Ok(User)` - The updated user record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection, missing user, or uniqueness conflict.
    async fn link_github_id(&self, user_id: i32, github_id: i64) -> Result<User, CddGatewayError>;

    /// Links a Google OAuth subject ID to an existing user account.
    ///
    /// # Arguments
    /// * `user_id` - Primary key ID of the user to update.
    /// * `google_id` - Google OAuth subject ID (`sub`) to attach.
    ///
    /// # Returns
    /// * `Ok(User)` - The updated user record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection, missing user, or uniqueness conflict.
    async fn link_google_id(
        &self,
        user_id: i32,
        google_id: String,
    ) -> Result<User, CddGatewayError>;

    /// Sets or updates the Argon2 password hash on an existing user account.
    ///
    /// # Arguments
    /// * `user_id` - Primary key ID of the user to update.
    /// * `password_hash` - PHC-formatted Argon2 password hash string.
    ///
    /// # Returns
    /// * `Ok(User)` - The updated user record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or missing user.
    async fn set_password_hash(
        &self,
        user_id: i32,
        password_hash: String,
    ) -> Result<User, CddGatewayError>;

    /// Unlinks an authentication provider (`"github"`, `"google"`, or `"password"`) from a user.
    ///
    /// Refuses with [`CddGatewayError::Validation`] if unlinking would leave the user with zero
    /// authentication methods.
    ///
    /// # Arguments
    /// * `user_id` - Primary key ID of the user to update.
    /// * `provider` - Provider identifier (`"github"`, `"google"`, or `"password"`).
    ///
    /// # Returns
    /// * `Ok(User)` - The updated user record.
    ///
    /// # Errors
    /// * [`CddGatewayError::Validation`] - If `provider` is unknown or is the user's sole login method.
    /// * [`CddGatewayError`] - On database connection or missing user.
    async fn unlink_provider(
        &self,
        user_id: i32,
        provider: String,
    ) -> Result<User, CddGatewayError>;

    /// Creates a new organization record.
    ///
    /// # Arguments
    /// * `github_id` - Optional GitHub numeric organization ID.
    /// * `login` - Unique organization handle.
    /// * `description` - Optional organization description.
    ///
    /// # Returns
    /// * `Ok(Organization)` - The newly created organization record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or constraint violation.
    async fn create_organization(
        &self,
        github_id: Option<i64>,
        login: String,
        description: Option<String>,
    ) -> Result<Organization, CddGatewayError>;

    /// Inserts or updates an organization keyed by its GitHub numeric ID.
    ///
    /// # Arguments
    /// * `github_id` - GitHub numeric organization ID.
    /// * `login` - Organization handle.
    /// * `description` - Optional organization description.
    ///
    /// # Returns
    /// * `Ok(Organization)` - The inserted or updated organization record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn upsert_organization(
        &self,
        github_id: i64,
        login: String,
        description: Option<String>,
    ) -> Result<Organization, CddGatewayError>;

    /// Retrieves an organization by its numeric primary key ID.
    ///
    /// # Arguments
    /// * `org_id` - Primary key ID of the organization.
    ///
    /// # Returns
    /// * `Ok(Some(Organization))` - If the organization exists.
    /// * `Ok(None)` - If no organization matches `org_id`.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn get_organization(&self, org_id: i32) -> Result<Option<Organization>, CddGatewayError>;

    /// Links a user to an organization with the given role, updating the role on conflict.
    ///
    /// # Arguments
    /// * `org_id` - Primary key ID of the organization.
    /// * `user_id` - Primary key ID of the user.
    /// * `role` - Role name to assign (e.g. `"owner"`, `"admin"`, `"member"`).
    ///
    /// # Returns
    /// * `Ok(OrganizationUser)` - The created or updated membership record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn add_user_to_organization(
        &self,
        org_id: i32,
        user_id: i32,
        role: String,
    ) -> Result<OrganizationUser, CddGatewayError>;

    /// Retrieves a user's role within an organization, if any.
    ///
    /// # Arguments
    /// * `org_id` - Primary key ID of the organization.
    /// * `user_id` - Primary key ID of the user.
    ///
    /// # Returns
    /// * `Ok(Some(String))` - The user's role name in the organization.
    /// * `Ok(None)` - If the user is not a member of the organization.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn get_user_role(
        &self,
        org_id: i32,
        user_id: i32,
    ) -> Result<Option<String>, CddGatewayError>;

    /// Creates a new repository under an organization with default topology metadata.
    ///
    /// # Arguments
    /// * `org_id` - Primary key ID of the owning organization.
    /// * `github_id` - Optional GitHub numeric repository ID.
    /// * `name` - Repository name.
    /// * `description` - Optional repository description.
    ///
    /// # Returns
    /// * `Ok(Repository)` - The newly created repository record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or constraint violation.
    async fn create_repository(
        &self,
        org_id: i32,
        github_id: Option<i64>,
        name: String,
        description: Option<String>,
    ) -> Result<Repository, CddGatewayError>;

    /// Creates a new repository under an organization with full topology, language, registry, and docs metadata.
    ///
    /// # Arguments
    /// * `org_id` - Primary key ID of the owning organization.
    /// * `github_id` - Optional GitHub numeric repository ID.
    /// * `name` - Repository name.
    /// * `description` - Optional repository description.
    /// * `metadata` - Strongly-typed [`RepositoryConfigMetadata`] describing topology, languages, and registries.
    ///
    /// # Returns
    /// * `Ok(Repository)` - The newly created repository record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On JSON serialization, database connection, or constraint violation.
    async fn create_repository_with_config(
        &self,
        org_id: i32,
        github_id: Option<i64>,
        name: String,
        description: Option<String>,
        metadata: RepositoryConfigMetadata,
    ) -> Result<Repository, CddGatewayError>;

    /// Updates an existing repository's name, description, and topology/language metadata.
    ///
    /// # Arguments
    /// * `repo_id` - Primary key ID of the repository to update.
    /// * `name` - Updated repository name.
    /// * `description` - Updated optional repository description.
    /// * `metadata` - Updated [`RepositoryConfigMetadata`].
    ///
    /// # Returns
    /// * `Ok(Repository)` - The updated repository record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On JSON serialization, missing repository, or database failure.
    async fn update_repository_with_config(
        &self,
        repo_id: i32,
        name: String,
        description: Option<String>,
        metadata: RepositoryConfigMetadata,
    ) -> Result<Repository, CddGatewayError>;

    /// Inserts or updates a repository keyed by its GitHub numeric ID.
    ///
    /// # Arguments
    /// * `org_id` - Primary key ID of the owning organization.
    /// * `github_id` - GitHub numeric repository ID.
    /// * `name` - Repository name.
    /// * `description` - Optional repository description.
    ///
    /// # Returns
    /// * `Ok(Repository)` - The inserted or updated repository record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn upsert_repository(
        &self,
        org_id: i32,
        github_id: i64,
        name: String,
        description: Option<String>,
    ) -> Result<Repository, CddGatewayError>;

    /// Retrieves a repository by its numeric primary key ID.
    ///
    /// # Arguments
    /// * `repo_id` - Primary key ID of the repository.
    ///
    /// # Returns
    /// * `Ok(Some(Repository))` - If the repository exists.
    /// * `Ok(None)` - If no repository matches `repo_id`.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn get_repository(&self, repo_id: i32) -> Result<Option<Repository>, CddGatewayError>;

    /// Creates a new release record for a repository.
    ///
    /// # Arguments
    /// * `repo_id` - Primary key ID of the repository.
    /// * `github_id` - Optional GitHub numeric release ID.
    /// * `tag_name` - Git tag name for the release.
    /// * `name` - Optional release title.
    /// * `body` - Optional release notes body.
    ///
    /// # Returns
    /// * `Ok(Release)` - The newly created release record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or constraint violation.
    async fn create_release(
        &self,
        repo_id: i32,
        github_id: Option<i64>,
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<Release, CddGatewayError>;

    /// Inserts or updates a release keyed by its GitHub numeric ID.
    ///
    /// # Arguments
    /// * `repo_id` - Primary key ID of the repository.
    /// * `github_id` - GitHub numeric release ID.
    /// * `tag_name` - Git tag name for the release.
    /// * `name` - Optional release title.
    /// * `body` - Optional release notes body.
    ///
    /// # Returns
    /// * `Ok(Release)` - The inserted or updated release record.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - On database connection or query failure.
    async fn upsert_release(
        &self,
        repo_id: i32,
        github_id: i64,
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<Release, CddGatewayError>;
}

/// Validates whether unlinking `provider` from `user` is permissible and returns which field to clear.
///
/// # Arguments
/// * `user` - The current user record.
/// * `provider` - The provider name (`"github"`, `"google"`, or `"password"`).
///
/// # Returns
/// * `Ok(&'static str)` - Canonical provider identifier (`"github"`, `"google"`, or `"password"`).
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `provider` is invalid, not linked, or is the last remaining login method.
fn validate_unlink_provider(user: &User, provider: &str) -> Result<&'static str, CddGatewayError> {
    let canonical = match provider.to_ascii_lowercase().as_str() {
        "github" => "github",
        "google" => "google",
        "password" => "password",
        other => {
            return Err(CddGatewayError::Validation(format!(
                "Unsupported authentication provider: {other}"
            )));
        }
    };

    let has_github = user.github_id.is_some();
    let has_google = user.google_id.is_some();
    let has_password = user.password_hash.is_some();

    let is_linked = match canonical {
        "github" => has_github,
        "google" => has_google,
        _ => has_password,
    };

    if !is_linked {
        return Err(CddGatewayError::Validation(format!(
            "Provider '{canonical}' is not linked to this account"
        )));
    }

    let total_methods =
        usize::from(has_github) + usize::from(has_google) + usize::from(has_password);
    if total_methods <= 1 {
        return Err(CddGatewayError::Validation(
            "Cannot unlink the last remaining authentication method".to_string(),
        ));
    }

    Ok(canonical)
}

/// `PostgreSQL` implementation of [`CddRepository`] backed by an `r2d2` connection pool.
pub struct PgRepository {
    /// The `PostgreSQL` database connection pool.
    pub pool: crate::db::DbPool,
}

impl PgRepository {
    /// Retrieves a pooled `PostgreSQL` connection from `self.pool`.
    ///
    /// # Arguments
    /// * `&self` - The repository instance.
    ///
    /// # Returns
    /// * `Ok(PooledConnection)` - A checked-out database connection.
    ///
    /// # Errors
    /// * [`CddGatewayError::Pool`] - If checking out a connection from the pool fails or times out.
    pub fn get_conn(
        &self,
    ) -> Result<r2d2::PooledConnection<ConnectionManager<PgConnection>>, CddGatewayError> {
        self.pool.get().map_err(CddGatewayError::Pool)
    }
}

#[async_trait]
impl CddRepository for PgRepository {
    async fn find_user_by_username(
        &self,
        username: String,
    ) -> Result<Option<User>, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            users::table
                .filter(users::username.eq(username))
                .first::<User>(&mut conn)
                .optional()
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn find_user_by_email(&self, email: String) -> Result<Option<User>, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            users::table
                .filter(users::email.eq(email))
                .first::<User>(&mut conn)
                .optional()
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn find_user_by_google_id(
        &self,
        google_id: String,
    ) -> Result<Option<User>, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            users::table
                .filter(users::google_id.eq(Some(google_id)))
                .first::<User>(&mut conn)
                .optional()
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn find_user_by_id(&self, id: i32) -> Result<Option<User>, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || users::table.find(id).first::<User>(&mut conn).optional())
            .await
            .map_err(CddGatewayError::Blocking)?
            .map_err(CddGatewayError::Database)
    }

    async fn create_user(
        &self,
        github_id: Option<i64>,
        username: String,
        email: String,
        password_hash: Option<String>,
    ) -> Result<User, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_user = NewUser {
                github_id,
                username: &username,
                email: &email,
                password_hash: password_hash.as_deref(),
                google_id: None,
            };
            diesel::insert_into(users::table)
                .values(&new_user)
                .get_result::<User>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn upsert_user(
        &self,
        github_id: i64,
        username: String,
        email: String,
    ) -> Result<User, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_user = NewUser {
                github_id: Some(github_id),
                username: &username,
                email: &email,
                password_hash: None,
                google_id: None,
            };
            diesel::insert_into(users::table)
                .values(&new_user)
                .on_conflict(users::github_id)
                .do_update()
                .set((users::username.eq(&username), users::email.eq(&email)))
                .get_result::<User>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn upsert_google_user(
        &self,
        google_id: String,
        username: String,
        email: String,
    ) -> Result<User, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_user = NewUser {
                github_id: None,
                username: &username,
                email: &email,
                password_hash: None,
                google_id: Some(&google_id),
            };
            diesel::insert_into(users::table)
                .values(&new_user)
                .on_conflict(users::google_id)
                .do_update()
                .set((users::username.eq(&username), users::email.eq(&email)))
                .get_result::<User>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn link_github_id(&self, user_id: i32, github_id: i64) -> Result<User, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            diesel::update(users::table.find(user_id))
                .set(users::github_id.eq(Some(github_id)))
                .get_result::<User>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn link_google_id(
        &self,
        user_id: i32,
        google_id: String,
    ) -> Result<User, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            diesel::update(users::table.find(user_id))
                .set(users::google_id.eq(Some(google_id)))
                .get_result::<User>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn set_password_hash(
        &self,
        user_id: i32,
        password_hash: String,
    ) -> Result<User, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            diesel::update(users::table.find(user_id))
                .set(users::password_hash.eq(Some(password_hash)))
                .get_result::<User>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn unlink_provider(
        &self,
        user_id: i32,
        provider: String,
    ) -> Result<User, CddGatewayError> {
        let user = self
            .find_user_by_id(user_id)
            .await?
            .ok_or_else(|| CddGatewayError::NotFound(format!("User {user_id} not found")))?;
        let canonical = validate_unlink_provider(&user, &provider)?;

        let mut conn = self.get_conn()?;
        web::block(move || match canonical {
            "github" => diesel::update(users::table.find(user_id))
                .set(users::github_id.eq(None::<i64>))
                .get_result::<User>(&mut conn),
            "google" => diesel::update(users::table.find(user_id))
                .set(users::google_id.eq(None::<String>))
                .get_result::<User>(&mut conn),
            _ => diesel::update(users::table.find(user_id))
                .set(users::password_hash.eq(None::<String>))
                .get_result::<User>(&mut conn),
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn create_organization(
        &self,
        github_id: Option<i64>,
        login: String,
        description: Option<String>,
    ) -> Result<Organization, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_org = NewOrganization {
                github_id,
                login: &login,
                description: description.as_deref(),
            };
            diesel::insert_into(organizations::table)
                .values(&new_org)
                .get_result::<Organization>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn upsert_organization(
        &self,
        github_id: i64,
        login: String,
        description: Option<String>,
    ) -> Result<Organization, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_org = NewOrganization {
                github_id: Some(github_id),
                login: &login,
                description: description.as_deref(),
            };
            diesel::insert_into(organizations::table)
                .values(&new_org)
                .on_conflict(organizations::github_id)
                .do_update()
                .set((
                    organizations::login.eq(&login),
                    organizations::description.eq(description.as_deref()),
                ))
                .get_result::<Organization>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn get_organization(&self, org_id: i32) -> Result<Option<Organization>, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            organizations::table
                .find(org_id)
                .first::<Organization>(&mut conn)
                .optional()
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn add_user_to_organization(
        &self,
        org_id: i32,
        user_id: i32,
        role: String,
    ) -> Result<OrganizationUser, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_link = NewOrganizationUser {
                organization_id: org_id,
                user_id,
                role: &role,
            };
            diesel::insert_into(organization_users::table)
                .values(&new_link)
                .on_conflict((
                    organization_users::organization_id,
                    organization_users::user_id,
                ))
                .do_update()
                .set(organization_users::role.eq(&role))
                .get_result::<OrganizationUser>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn get_user_role(
        &self,
        org_id: i32,
        user_id: i32,
    ) -> Result<Option<String>, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            organization_users::table
                .filter(organization_users::organization_id.eq(org_id))
                .filter(organization_users::user_id.eq(user_id))
                .select(organization_users::role)
                .first::<String>(&mut conn)
                .optional()
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn create_repository(
        &self,
        org_id: i32,
        github_id: Option<i64>,
        name: String,
        description: Option<String>,
    ) -> Result<Repository, CddGatewayError> {
        self.create_repository_with_config(
            org_id,
            github_id,
            name,
            description,
            RepositoryConfigMetadata::default(),
        )
        .await
    }

    async fn create_repository_with_config(
        &self,
        org_id: i32,
        github_id: Option<i64>,
        name: String,
        description: Option<String>,
        metadata: RepositoryConfigMetadata,
    ) -> Result<Repository, CddGatewayError> {
        let target_languages_json = serde_json::to_string(&metadata.target_languages)?;
        let language_options_json = serde_json::to_string(&metadata.language_options)?;
        let release_registries_json = serde_json::to_string(&metadata.release_registries)?;
        let docs_config_json = serde_json::to_string(&metadata.docs_config)?;
        let topology_mode = metadata.topology_mode.to_string();
        let github_org_name = metadata.github_org_name;
        let enable_pgp_signing = metadata.enable_pgp_signing;

        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_repo = NewRepository {
                organization_id: org_id,
                github_id,
                name: &name,
                description: description.as_deref(),
                github_org_name: &github_org_name,
                topology_mode: &topology_mode,
                target_languages_json: &target_languages_json,
                language_options_json: &language_options_json,
                release_registries_json: &release_registries_json,
                enable_pgp_signing,
                docs_config_json: &docs_config_json,
            };
            diesel::insert_into(repositories::table)
                .values(&new_repo)
                .get_result::<Repository>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn update_repository_with_config(
        &self,
        repo_id: i32,
        name: String,
        description: Option<String>,
        metadata: RepositoryConfigMetadata,
    ) -> Result<Repository, CddGatewayError> {
        let target_languages_json = serde_json::to_string(&metadata.target_languages)?;
        let language_options_json = serde_json::to_string(&metadata.language_options)?;
        let release_registries_json = serde_json::to_string(&metadata.release_registries)?;
        let docs_config_json = serde_json::to_string(&metadata.docs_config)?;
        let topology_mode = metadata.topology_mode.to_string();
        let github_org_name = metadata.github_org_name;
        let enable_pgp_signing = metadata.enable_pgp_signing;

        let mut conn = self.get_conn()?;
        web::block(move || {
            diesel::update(repositories::table.find(repo_id))
                .set((
                    repositories::name.eq(&name),
                    repositories::description.eq(description.as_deref()),
                    repositories::github_org_name.eq(&github_org_name),
                    repositories::topology_mode.eq(&topology_mode),
                    repositories::target_languages_json.eq(&target_languages_json),
                    repositories::language_options_json.eq(&language_options_json),
                    repositories::release_registries_json.eq(&release_registries_json),
                    repositories::enable_pgp_signing.eq(enable_pgp_signing),
                    repositories::docs_config_json.eq(&docs_config_json),
                ))
                .get_result::<Repository>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn upsert_repository(
        &self,
        org_id: i32,
        github_id: i64,
        name: String,
        description: Option<String>,
    ) -> Result<Repository, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_repo = NewRepository {
                organization_id: org_id,
                github_id: Some(github_id),
                name: &name,
                description: description.as_deref(),
                github_org_name: "",
                topology_mode: "monorepo",
                target_languages_json: "[]",
                language_options_json: "{}",
                release_registries_json: "[]",
                enable_pgp_signing: false,
                docs_config_json: "{}",
            };
            diesel::insert_into(repositories::table)
                .values(&new_repo)
                .on_conflict(repositories::github_id)
                .do_update()
                .set((
                    repositories::name.eq(&name),
                    repositories::description.eq(description.as_deref()),
                    repositories::organization_id.eq(org_id),
                ))
                .get_result::<Repository>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn get_repository(&self, repo_id: i32) -> Result<Option<Repository>, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            repositories::table
                .find(repo_id)
                .first::<Repository>(&mut conn)
                .optional()
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn create_release(
        &self,
        repo_id: i32,
        github_id: Option<i64>,
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<Release, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_release = NewRelease {
                repository_id: repo_id,
                github_id,
                tag_name: &tag_name,
                name: name.as_deref(),
                body: body.as_deref(),
            };
            diesel::insert_into(releases::table)
                .values(&new_release)
                .get_result::<Release>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }

    async fn upsert_release(
        &self,
        repo_id: i32,
        github_id: i64,
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<Release, CddGatewayError> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_release = NewRelease {
                repository_id: repo_id,
                github_id: Some(github_id),
                tag_name: &tag_name,
                name: name.as_deref(),
                body: body.as_deref(),
            };
            diesel::insert_into(releases::table)
                .values(&new_release)
                .on_conflict(releases::github_id)
                .do_update()
                .set((
                    releases::tag_name.eq(&tag_name),
                    releases::name.eq(name.as_deref()),
                    releases::body.eq(body.as_deref()),
                    releases::repository_id.eq(repo_id),
                ))
                .get_result::<Release>(&mut conn)
        })
        .await
        .map_err(CddGatewayError::Blocking)?
        .map_err(CddGatewayError::Database)
    }
}

/// Internal state for the in-memory [`OfflineRepository`].
#[derive(Default)]
struct OfflineStore {
    /// Stored users.
    users: Vec<User>,
    /// Stored organizations.
    organizations: Vec<Organization>,
    /// Stored organization-user role links.
    organization_users: Vec<OrganizationUser>,
    /// Stored repositories.
    repositories: Vec<Repository>,
    /// Stored releases.
    releases: Vec<Release>,
}

/// Thread-safe in-memory implementation of [`CddRepository`] used when `offline_mode` is enabled.
#[derive(Clone, Default)]
pub struct OfflineRepository {
    /// Shared in-memory store protected by a mutex.
    store: Arc<Mutex<OfflineStore>>,
}

impl OfflineRepository {
    /// Creates a new empty [`OfflineRepository`].
    ///
    /// # Returns
    /// * [`OfflineRepository`] - Fresh in-memory repository instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl CddRepository for OfflineRepository {
    async fn find_user_by_username(
        &self,
        username: String,
    ) -> Result<Option<User>, CddGatewayError> {
        let guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard.users.iter().find(|u| u.username == username).cloned())
    }

    async fn find_user_by_email(&self, email: String) -> Result<Option<User>, CddGatewayError> {
        let guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard.users.iter().find(|u| u.email == email).cloned())
    }

    async fn find_user_by_google_id(
        &self,
        google_id: String,
    ) -> Result<Option<User>, CddGatewayError> {
        let guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard
            .users
            .iter()
            .find(|u| u.google_id.as_deref() == Some(google_id.as_str()))
            .cloned())
    }

    async fn find_user_by_id(&self, id: i32) -> Result<Option<User>, CddGatewayError> {
        let guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard.users.iter().find(|u| u.id == id).cloned())
    }

    async fn create_user(
        &self,
        github_id: Option<i64>,
        username: String,
        email: String,
        password_hash: Option<String>,
    ) -> Result<User, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let next_id = i32::try_from(guard.users.len()).unwrap_or(0) + 1;
        let user = User {
            id: next_id,
            github_id,
            username,
            email,
            password_hash,
            google_id: None,
        };
        guard.users.push(user.clone());
        drop(guard);
        Ok(user)
    }

    async fn upsert_user(
        &self,
        github_id: i64,
        username: String,
        email: String,
    ) -> Result<User, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = guard
            .users
            .iter_mut()
            .find(|u| u.github_id == Some(github_id))
        {
            existing.username = username;
            existing.email = email;
            return Ok(existing.clone());
        }
        let next_id = i32::try_from(guard.users.len()).unwrap_or(0) + 1;
        let user = User {
            id: next_id,
            github_id: Some(github_id),
            username,
            email,
            password_hash: None,
            google_id: None,
        };
        guard.users.push(user.clone());
        drop(guard);
        Ok(user)
    }

    async fn upsert_google_user(
        &self,
        google_id: String,
        username: String,
        email: String,
    ) -> Result<User, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = guard
            .users
            .iter_mut()
            .find(|u| u.google_id.as_deref() == Some(google_id.as_str()))
        {
            existing.username = username;
            existing.email = email;
            return Ok(existing.clone());
        }
        let next_id = i32::try_from(guard.users.len()).unwrap_or(0) + 1;
        let user = User {
            id: next_id,
            github_id: None,
            username,
            email,
            password_hash: None,
            google_id: Some(google_id),
        };
        guard.users.push(user.clone());
        drop(guard);
        Ok(user)
    }

    async fn link_github_id(&self, user_id: i32, github_id: i64) -> Result<User, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let user = guard
            .users
            .iter_mut()
            .find(|u| u.id == user_id)
            .ok_or_else(|| CddGatewayError::NotFound(format!("User {user_id} not found")))?;
        user.github_id = Some(github_id);
        let cloned = user.clone();
        drop(guard);
        Ok(cloned)
    }

    async fn link_google_id(
        &self,
        user_id: i32,
        google_id: String,
    ) -> Result<User, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let user = guard
            .users
            .iter_mut()
            .find(|u| u.id == user_id)
            .ok_or_else(|| CddGatewayError::NotFound(format!("User {user_id} not found")))?;
        user.google_id = Some(google_id);
        let cloned = user.clone();
        drop(guard);
        Ok(cloned)
    }

    async fn set_password_hash(
        &self,
        user_id: i32,
        password_hash: String,
    ) -> Result<User, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let user = guard
            .users
            .iter_mut()
            .find(|u| u.id == user_id)
            .ok_or_else(|| CddGatewayError::NotFound(format!("User {user_id} not found")))?;
        user.password_hash = Some(password_hash);
        let cloned = user.clone();
        drop(guard);
        Ok(cloned)
    }

    async fn unlink_provider(
        &self,
        user_id: i32,
        provider: String,
    ) -> Result<User, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let user = guard
            .users
            .iter_mut()
            .find(|u| u.id == user_id)
            .ok_or_else(|| CddGatewayError::NotFound(format!("User {user_id} not found")))?;
        let canonical = validate_unlink_provider(user, &provider)?;
        match canonical {
            "github" => user.github_id = None,
            "google" => user.google_id = None,
            _ => user.password_hash = None,
        }
        let cloned = user.clone();
        drop(guard);
        Ok(cloned)
    }

    async fn create_organization(
        &self,
        github_id: Option<i64>,
        login: String,
        description: Option<String>,
    ) -> Result<Organization, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let next_id = i32::try_from(guard.organizations.len()).unwrap_or(0) + 1;
        let org = Organization {
            id: next_id,
            github_id,
            login,
            description,
        };
        guard.organizations.push(org.clone());
        drop(guard);
        Ok(org)
    }

    async fn upsert_organization(
        &self,
        github_id: i64,
        login: String,
        description: Option<String>,
    ) -> Result<Organization, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = guard
            .organizations
            .iter_mut()
            .find(|o| o.github_id == Some(github_id))
        {
            existing.login = login;
            existing.description = description;
            return Ok(existing.clone());
        }
        let next_id = i32::try_from(guard.organizations.len()).unwrap_or(0) + 1;
        let org = Organization {
            id: next_id,
            github_id: Some(github_id),
            login,
            description,
        };
        guard.organizations.push(org.clone());
        drop(guard);
        Ok(org)
    }

    async fn get_organization(&self, org_id: i32) -> Result<Option<Organization>, CddGatewayError> {
        let guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard.organizations.iter().find(|o| o.id == org_id).cloned())
    }

    async fn add_user_to_organization(
        &self,
        org_id: i32,
        user_id: i32,
        role: String,
    ) -> Result<OrganizationUser, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = guard
            .organization_users
            .iter_mut()
            .find(|ou| ou.organization_id == org_id && ou.user_id == user_id)
        {
            existing.role = role;
            return Ok(existing.clone());
        }
        let link = OrganizationUser {
            organization_id: org_id,
            user_id,
            role,
        };
        guard.organization_users.push(link.clone());
        drop(guard);
        Ok(link)
    }

    async fn get_user_role(
        &self,
        org_id: i32,
        user_id: i32,
    ) -> Result<Option<String>, CddGatewayError> {
        let guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard
            .organization_users
            .iter()
            .find(|ou| ou.organization_id == org_id && ou.user_id == user_id)
            .map(|ou| ou.role.clone()))
    }

    async fn create_repository(
        &self,
        org_id: i32,
        github_id: Option<i64>,
        name: String,
        description: Option<String>,
    ) -> Result<Repository, CddGatewayError> {
        self.create_repository_with_config(
            org_id,
            github_id,
            name,
            description,
            RepositoryConfigMetadata::default(),
        )
        .await
    }

    async fn create_repository_with_config(
        &self,
        org_id: i32,
        github_id: Option<i64>,
        name: String,
        description: Option<String>,
        metadata: RepositoryConfigMetadata,
    ) -> Result<Repository, CddGatewayError> {
        let target_languages_json = serde_json::to_string(&metadata.target_languages)?;
        let language_options_json = serde_json::to_string(&metadata.language_options)?;
        let release_registries_json = serde_json::to_string(&metadata.release_registries)?;
        let docs_config_json = serde_json::to_string(&metadata.docs_config)?;

        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let next_id = i32::try_from(guard.repositories.len()).unwrap_or(0) + 1;
        let repo = Repository {
            id: next_id,
            organization_id: org_id,
            github_id,
            name,
            description,
            github_org_name: metadata.github_org_name,
            topology_mode: metadata.topology_mode.to_string(),
            target_languages_json,
            language_options_json,
            release_registries_json,
            enable_pgp_signing: metadata.enable_pgp_signing,
            docs_config_json,
        };
        guard.repositories.push(repo.clone());
        drop(guard);
        Ok(repo)
    }

    async fn update_repository_with_config(
        &self,
        repo_id: i32,
        name: String,
        description: Option<String>,
        metadata: RepositoryConfigMetadata,
    ) -> Result<Repository, CddGatewayError> {
        let target_languages_json = serde_json::to_string(&metadata.target_languages)?;
        let language_options_json = serde_json::to_string(&metadata.language_options)?;
        let release_registries_json = serde_json::to_string(&metadata.release_registries)?;
        let docs_config_json = serde_json::to_string(&metadata.docs_config)?;

        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let existing = guard
            .repositories
            .iter_mut()
            .find(|r| r.id == repo_id)
            .ok_or_else(|| CddGatewayError::NotFound(format!("Repository {repo_id} not found")))?;
        existing.name = name;
        existing.description = description;
        existing.github_org_name = metadata.github_org_name;
        existing.topology_mode = metadata.topology_mode.to_string();
        existing.target_languages_json = target_languages_json;
        existing.language_options_json = language_options_json;
        existing.release_registries_json = release_registries_json;
        existing.enable_pgp_signing = metadata.enable_pgp_signing;
        existing.docs_config_json = docs_config_json;
        let cloned = existing.clone();
        drop(guard);
        Ok(cloned)
    }

    async fn upsert_repository(
        &self,
        org_id: i32,
        github_id: i64,
        name: String,
        description: Option<String>,
    ) -> Result<Repository, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = guard
            .repositories
            .iter_mut()
            .find(|r| r.github_id == Some(github_id))
        {
            existing.organization_id = org_id;
            existing.name = name;
            existing.description = description;
            return Ok(existing.clone());
        }
        let next_id = i32::try_from(guard.repositories.len()).unwrap_or(0) + 1;
        let repo = Repository {
            id: next_id,
            organization_id: org_id,
            github_id: Some(github_id),
            name,
            description,
            github_org_name: String::new(),
            topology_mode: "monorepo".to_string(),
            target_languages_json: "[]".to_string(),
            language_options_json: "{}".to_string(),
            release_registries_json: "[]".to_string(),
            enable_pgp_signing: false,
            docs_config_json: "{}".to_string(),
        };
        guard.repositories.push(repo.clone());
        drop(guard);
        Ok(repo)
    }

    async fn get_repository(&self, repo_id: i32) -> Result<Option<Repository>, CddGatewayError> {
        let guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard.repositories.iter().find(|r| r.id == repo_id).cloned())
    }

    async fn create_release(
        &self,
        repo_id: i32,
        github_id: Option<i64>,
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<Release, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let next_id = i32::try_from(guard.releases.len()).unwrap_or(0) + 1;
        let release = Release {
            id: next_id,
            repository_id: repo_id,
            github_id,
            tag_name,
            name,
            body,
        };
        guard.releases.push(release.clone());
        drop(guard);
        Ok(release)
    }

    async fn upsert_release(
        &self,
        repo_id: i32,
        github_id: i64,
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<Release, CddGatewayError> {
        let mut guard = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = guard
            .releases
            .iter_mut()
            .find(|r| r.github_id == Some(github_id))
        {
            existing.repository_id = repo_id;
            existing.tag_name = tag_name;
            existing.name = name;
            existing.body = body;
            return Ok(existing.clone());
        }
        let next_id = i32::try_from(guard.releases.len()).unwrap_or(0) + 1;
        let release = Release {
            id: next_id,
            repository_id: repo_id,
            github_id: Some(github_id),
            tag_name,
            name,
            body,
        };
        guard.releases.push(release.clone());
        drop(guard);
        Ok(release)
    }
}

#[cfg(test)]
mod offline_tests {
    use super::*;

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn test_offline_repository_crud_and_upserts() -> Result<(), CddGatewayError> {
        let repo = OfflineRepository::new();

        let u1 = repo
            .create_user(
                Some(10),
                "alice".into(),
                "a@b.com".into(),
                Some("pw".into()),
            )
            .await?;
        assert_eq!(u1.id, 1);
        assert_eq!(
            repo.find_user_by_username("alice".into()).await?,
            Some(u1.clone())
        );
        assert_eq!(
            repo.find_user_by_email("a@b.com".into()).await?,
            Some(u1.clone())
        );
        assert_eq!(repo.find_user_by_id(1).await?, Some(u1));

        let u1_google = repo.link_google_id(1, "g_100".into()).await?;
        assert_eq!(u1_google.google_id.as_deref(), Some("g_100"));
        assert_eq!(
            repo.find_user_by_google_id("g_100".into()).await?,
            Some(u1_google)
        );
        assert!(repo.link_google_id(999, "g_none".into()).await.is_err());

        let u1_gh = repo.link_github_id(1, 777).await?;
        assert_eq!(u1_gh.github_id, Some(777));
        assert!(repo.link_github_id(999, 777).await.is_err());

        let u1_pw = repo.set_password_hash(1, "new_hash".into()).await?;
        assert_eq!(u1_pw.password_hash.as_deref(), Some("new_hash"));
        assert!(repo.set_password_hash(999, "h".into()).await.is_err());

        // Unlink providers
        assert!(repo.unlink_provider(999, "github".into()).await.is_err());
        assert!(repo.unlink_provider(1, "unknown".into()).await.is_err());
        let u1_no_gh = repo.unlink_provider(1, "github".into()).await?;
        assert!(u1_no_gh.github_id.is_none());
        // Unlinking github again fails because it is already unlinked
        assert!(repo.unlink_provider(1, "github".into()).await.is_err());
        let u1_no_g = repo.unlink_provider(1, "google".into()).await?;
        assert!(u1_no_g.google_id.is_none());
        // Cannot unlink last remaining provider (password)
        assert!(repo.unlink_provider(1, "password".into()).await.is_err());
        // Re-link github so we can unlink password
        let _ = repo.link_github_id(1, 888).await?;
        let u1_no_pw = repo.unlink_provider(1, "password".into()).await?;
        assert!(u1_no_pw.password_hash.is_none());

        let u2 = repo
            .upsert_user(20, "bob".into(), "bob@b.com".into())
            .await?;
        assert_eq!(u2.username, "bob");
        let u2_up = repo
            .upsert_user(20, "bob2".into(), "bob2@b.com".into())
            .await?;
        assert_eq!(u2_up.id, u2.id);
        assert_eq!(u2_up.username, "bob2");

        let google_user = repo
            .upsert_google_user("g_sub_1".into(), "carol".into(), "carol@b.com".into())
            .await?;
        assert_eq!(google_user.username, "carol");
        let google_updated = repo
            .upsert_google_user("g_sub_1".into(), "carol2".into(), "carol2@b.com".into())
            .await?;
        assert_eq!(google_updated.id, google_user.id);
        assert_eq!(google_updated.username, "carol2");

        let org1 = repo
            .create_organization(None, "org1".into(), Some("d".into()))
            .await?;
        assert_eq!(repo.get_organization(org1.id).await?, Some(org1.clone()));

        let org2 = repo.upsert_organization(99, "org99".into(), None).await?;
        let org2_up = repo
            .upsert_organization(99, "org99_new".into(), Some("d2".into()))
            .await?;
        assert_eq!(org2_up.id, org2.id);
        assert_eq!(org2_up.login, "org99_new");

        let link = repo
            .add_user_to_organization(org1.id, 1, "owner".into())
            .await?;
        assert_eq!(link.role, "owner");
        assert!(repo.get_user_role(org2.id, 1).await?.is_none());
        assert!(repo.get_user_role(org1.id, 2).await?.is_none());
        assert_eq!(
            repo.get_user_role(org1.id, 1).await?,
            Some("owner".to_string())
        );
        let _ = repo
            .add_user_to_organization(org2.id, 1, "member".into())
            .await?;
        let _ = repo
            .add_user_to_organization(org1.id, 2, "member".into())
            .await?;
        let link_up = repo
            .add_user_to_organization(org1.id, 1, "admin".into())
            .await?;
        assert_eq!(link_up.role, "admin");

        let r1 = repo
            .create_repository(org1.id, None, "r1".into(), None)
            .await?;
        assert_eq!(repo.get_repository(r1.id).await?, Some(r1.clone()));
        let r1_updated = repo
            .update_repository_with_config(
                r1.id,
                "r1_updated".into(),
                Some("updated desc".into()),
                RepositoryConfigMetadata::default(),
            )
            .await?;
        assert_eq!(r1_updated.name, "r1_updated");
        assert!(repo
            .update_repository_with_config(
                999,
                "none".into(),
                None,
                RepositoryConfigMetadata::default(),
            )
            .await
            .is_err());

        let r2 = repo
            .upsert_repository(org1.id, 500, "r500".into(), None)
            .await?;
        let r2_up = repo
            .upsert_repository(org1.id, 500, "r500_new".into(), Some("desc".into()))
            .await?;
        assert_eq!(r2_up.id, r2.id);
        assert_eq!(r2_up.name, "r500_new");

        let rel1 = repo
            .create_release(r1.id, None, "v1.0.0".into(), None, None)
            .await?;
        assert_eq!(rel1.tag_name, "v1.0.0");

        let rel2 = repo
            .upsert_release(r1.id, 900, "v2.0.0".into(), None, None)
            .await?;
        let rel2_up = repo
            .upsert_release(
                r1.id,
                900,
                "v2.0.1".into(),
                Some("Rel".into()),
                Some("Body".into()),
            )
            .await?;
        assert_eq!(rel2_up.id, rel2.id);
        assert_eq!(rel2_up.tag_name, "v2.0.1");

        Ok(())
    }
}

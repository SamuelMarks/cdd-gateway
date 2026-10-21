#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Database repository trait and concrete implementations (`PgRepository` and `OfflineRepository`).
//!
//! Abstracts all persistence operations behind [`CddRepository`] returning [`CddGatewayError`],
//! backed either by `PostgreSQL` via [`PgRepository`] or by in-memory storage via [`OfflineRepository`].

use crate::db::models::{
    NewOrganization, NewOrganizationUser, NewRelease, NewRepository, NewUser, Organization,
    OrganizationUser, Release, Repository, User,
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

    /// Creates a new repository under an organization.
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
        let mut conn = self.get_conn()?;
        web::block(move || {
            let new_repo = NewRepository {
                organization_id: org_id,
                github_id,
                name: &name,
                description: description.as_deref(),
            };
            diesel::insert_into(repositories::table)
                .values(&new_repo)
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
        };
        guard.users.push(user.clone());
        drop(guard);
        Ok(user)
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
        };
        guard.repositories.push(repo.clone());
        drop(guard);
        Ok(repo)
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
        assert_eq!(repo.find_user_by_id(1).await?, Some(u1));

        let u2 = repo
            .upsert_user(20, "bob".into(), "bob@b.com".into())
            .await?;
        assert_eq!(u2.username, "bob");
        let u2_up = repo
            .upsert_user(20, "bob2".into(), "bob2@b.com".into())
            .await?;
        assert_eq!(u2_up.id, u2.id);
        assert_eq!(u2_up.username, "bob2");

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
        assert_eq!(
            repo.get_user_role(org1.id, 1).await?,
            Some("owner".to_string())
        );
        let link_up = repo
            .add_user_to_organization(org1.id, 1, "admin".into())
            .await?;
        assert_eq!(link_up.role, "admin");

        let r1 = repo
            .create_repository(org1.id, None, "r1".into(), None)
            .await?;
        assert_eq!(repo.get_repository(r1.id).await?, Some(r1.clone()));

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

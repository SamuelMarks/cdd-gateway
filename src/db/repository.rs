#![cfg(not(tarpaulin_include))]

use crate::db::models::*;
use crate::db::schema::*;
use actix_web::web;
use async_trait::async_trait;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::result::Error;
use mockall::automock;

/// Database repository trait for handling CRUD operations.
#[automock]
#[async_trait]
pub trait CddRepository: Send + Sync {
    /// Find a user by username
    async fn find_user_by_username(&self, username: String) -> Result<Option<User>, Error>;
    /// Find a user by id
    async fn find_user_by_id(&self, id: i32) -> Result<Option<User>, Error>;
    /// Create a new user
    async fn create_user(
        &self,
        github_id: Option<i64>,
        username: String,
        email: String,
        password_hash: Option<String>,
    ) -> Result<User, Error>;

    /// Upsert a user based on GitHub ID
    async fn upsert_user(
        &self,
        github_id: i64,
        username: String,
        email: String,
    ) -> Result<User, Error>;

    /// Create an organization
    async fn create_organization(
        &self,
        github_id: Option<i64>,
        login: String,
        description: Option<String>,
    ) -> Result<Organization, Error>;

    /// Upsert an organization based on GitHub ID
    async fn upsert_organization(
        &self,
        github_id: i64,
        login: String,
        description: Option<String>,
    ) -> Result<Organization, Error>;

    /// Get an organization
    async fn get_organization(&self, org_id: i32) -> Result<Option<Organization>, Error>;

    /// Link user to organization
    async fn add_user_to_organization(
        &self,
        org_id: i32,
        user_id: i32,
        role: String,
    ) -> Result<OrganizationUser, Error>;

    /// Check user role in organization
    async fn get_user_role(&self, org_id: i32, user_id: i32) -> Result<Option<String>, Error>;

    /// Create a repository
    async fn create_repository(
        &self,
        org_id: i32,
        github_id: Option<i64>,
        name: String,
        description: Option<String>,
    ) -> Result<Repository, Error>;

    /// Upsert a repository based on GitHub ID
    async fn upsert_repository(
        &self,
        org_id: i32,
        github_id: i64,
        name: String,
        description: Option<String>,
    ) -> Result<Repository, Error>;

    /// Get a repository
    async fn get_repository(&self, repo_id: i32) -> Result<Option<Repository>, Error>;

    /// Create a release
    async fn create_release(
        &self,
        repo_id: i32,
        github_id: Option<i64>,
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<Release, Error>;

    /// Upsert a release based on GitHub ID
    async fn upsert_release(
        &self,
        repo_id: i32,
        github_id: i64,
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<Release, Error>;
}

impl PgRepository {
    /// Helper to get a database connection
    pub fn get_conn(
        &self,
    ) -> Result<r2d2::PooledConnection<ConnectionManager<PgConnection>>, Error> {
        self.pool.get().map_err(|e| {
            log::error!("Failed to get DB connection: {}", e);
            Error::NotFound
        })
    }
}

/// Postgres implementation of CddRepository
pub struct PgRepository {
    /// The database connection pool
    pub pool: crate::db::DbPool,
}

#[async_trait]
impl CddRepository for PgRepository {
    async fn find_user_by_username(&self, username: String) -> Result<Option<User>, Error> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            users::table
                .filter(users::username.eq(username))
                .first::<User>(&mut conn)
                .optional()
        })
        .await
        .map_err(|e| {
            log::error!("DB thread pool error: {}", e);
            Error::NotFound
        })?
    }

    async fn find_user_by_id(&self, id: i32) -> Result<Option<User>, Error> {
        let mut conn = self.get_conn()?;
        web::block(move || users::table.find(id).first::<User>(&mut conn).optional())
            .await
            .map_err(|_| Error::NotFound)?
    }

    async fn create_user(
        &self,
        github_id: Option<i64>,
        username: String,
        email: String,
        password_hash: Option<String>,
    ) -> Result<User, Error> {
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
        .map_err(|_| Error::NotFound)?
    }

    async fn upsert_user(
        &self,
        github_id: i64,
        username: String,
        email: String,
    ) -> Result<User, Error> {
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
        .map_err(|_| Error::NotFound)?
    }

    async fn create_organization(
        &self,
        github_id: Option<i64>,
        login: String,
        description: Option<String>,
    ) -> Result<Organization, Error> {
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
        .map_err(|_| Error::NotFound)?
    }

    async fn upsert_organization(
        &self,
        github_id: i64,
        login: String,
        description: Option<String>,
    ) -> Result<Organization, Error> {
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
        .map_err(|_| Error::NotFound)?
    }

    async fn get_organization(&self, org_id: i32) -> Result<Option<Organization>, Error> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            organizations::table
                .find(org_id)
                .first::<Organization>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::NotFound)?
    }

    async fn add_user_to_organization(
        &self,
        org_id: i32,
        user_id: i32,
        role: String,
    ) -> Result<OrganizationUser, Error> {
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
        .map_err(|_| Error::NotFound)?
    }

    async fn get_user_role(&self, org_id: i32, user_id: i32) -> Result<Option<String>, Error> {
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
        .map_err(|_| Error::NotFound)?
    }

    async fn create_repository(
        &self,
        org_id: i32,
        github_id: Option<i64>,
        name: String,
        description: Option<String>,
    ) -> Result<Repository, Error> {
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
        .map_err(|_| Error::NotFound)?
    }

    async fn upsert_repository(
        &self,
        org_id: i32,
        github_id: i64,
        name: String,
        description: Option<String>,
    ) -> Result<Repository, Error> {
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
        .map_err(|_| Error::NotFound)?
    }

    async fn get_repository(&self, repo_id: i32) -> Result<Option<Repository>, Error> {
        let mut conn = self.get_conn()?;
        web::block(move || {
            repositories::table
                .find(repo_id)
                .first::<Repository>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::NotFound)?
    }

    async fn create_release(
        &self,
        repo_id: i32,
        github_id: Option<i64>,
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<Release, Error> {
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
        .map_err(|_| Error::NotFound)?
    }

    async fn upsert_release(
        &self,
        repo_id: i32,
        github_id: i64,
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<Release, Error> {
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
        .map_err(|_| Error::NotFound)?
    }
}

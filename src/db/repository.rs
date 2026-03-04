use crate::db::models::*;
use async_trait::async_trait;
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

/// Postgres implementation of CddRepository
pub struct PgRepository {
    /// The database connection pool
    pub pool: crate::db::DbPool,
}

#[cfg(not(tarpaulin_include))]
#[async_trait]
impl CddRepository for PgRepository {
    async fn find_user_by_username(&self, _username: String) -> Result<Option<User>, Error> {
        unimplemented!()
    }
    async fn find_user_by_id(&self, _id: i32) -> Result<Option<User>, Error> {
        unimplemented!()
    }
    async fn create_user(
        &self,
        _github_id: Option<i64>,
        _username: String,
        _email: String,
        _password_hash: Option<String>,
    ) -> Result<User, Error> {
        unimplemented!()
    }
    async fn upsert_user(
        &self,
        _github_id: i64,
        _username: String,
        _email: String,
    ) -> Result<User, Error> {
        unimplemented!()
    }
    async fn create_organization(
        &self,
        _github_id: Option<i64>,
        _login: String,
        _description: Option<String>,
    ) -> Result<Organization, Error> {
        unimplemented!()
    }
    async fn upsert_organization(
        &self,
        _github_id: i64,
        _login: String,
        _description: Option<String>,
    ) -> Result<Organization, Error> {
        unimplemented!()
    }
    async fn get_organization(&self, _org_id: i32) -> Result<Option<Organization>, Error> {
        unimplemented!()
    }
    async fn add_user_to_organization(
        &self,
        _org_id: i32,
        _user_id: i32,
        _role: String,
    ) -> Result<OrganizationUser, Error> {
        unimplemented!()
    }
    async fn get_user_role(&self, _org_id: i32, _user_id: i32) -> Result<Option<String>, Error> {
        unimplemented!()
    }
    async fn create_repository(
        &self,
        _org_id: i32,
        _github_id: Option<i64>,
        _name: String,
        _description: Option<String>,
    ) -> Result<Repository, Error> {
        unimplemented!()
    }
    async fn upsert_repository(
        &self,
        _org_id: i32,
        _github_id: i64,
        _name: String,
        _description: Option<String>,
    ) -> Result<Repository, Error> {
        unimplemented!()
    }
    async fn get_repository(&self, _repo_id: i32) -> Result<Option<Repository>, Error> {
        unimplemented!()
    }
    async fn create_release(
        &self,
        _repo_id: i32,
        _github_id: Option<i64>,
        _tag_name: String,
        _name: Option<String>,
        _body: Option<String>,
    ) -> Result<Release, Error> {
        unimplemented!()
    }
    async fn upsert_release(
        &self,
        _repo_id: i32,
        _github_id: i64,
        _tag_name: String,
        _name: Option<String>,
        _body: Option<String>,
    ) -> Result<Release, Error> {
        unimplemented!()
    }
}

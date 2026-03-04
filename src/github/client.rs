use crate::github::models::*;
use async_trait::async_trait;
use mockall::automock;
use reqwest::{header, Client};

/// A trait abstracting GitHub API calls
#[automock]
#[async_trait]
pub trait GitHubClient: Send + Sync {
    /// Exchanges an OAuth code for an access token
    async fn exchange_code(&self, _code: &str) -> Result<String, String>;

    /// Gets the authenticated user's profile
    async fn get_user(&self, token: &str) -> Result<GitHubUser, String>;

    /// Gets the authenticated user's emails
    async fn get_emails(&self, token: &str) -> Result<Vec<GitHubEmail>, String>;

    /// Lists organizations the user is a member of
    async fn list_orgs(&self, token: &str) -> Result<Vec<GitHubOrg>, String>;

    /// Lists repositories for an organization
    async fn list_repos(&self, token: &str, org: &str) -> Result<Vec<GitHubRepo>, String>;

    /// Create a release for a repository
    async fn create_release(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        tag_name: &str,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<GitHubRelease, String>;

    /// Trigger a GitHub Actions workflow
    async fn trigger_workflow(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        workflow_id: &str,
        ref_branch: &str,
    ) -> Result<(), String>;

    /// Get the repository's public key for secrets
    async fn get_repo_public_key(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<GitHubPublicKey, String>;

    /// Create or update a repository secret
    async fn create_repo_secret(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        secret_name: &str,
        encrypted_value: &str,
        key_id: &str,
    ) -> Result<(), String>;
}

/// Reqwest implementation of GitHubClient
#[allow(dead_code)]
pub struct ReqwestGitHubClient {
    client: Client,
    client_id: String,
    client_secret: String,
}

#[cfg(not(tarpaulin_include))]
impl ReqwestGitHubClient {
    /// Create a new ReqwestGitHubClient
    pub fn new(client_id: String, client_secret: String) -> Self {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("cdd-ctl"),
        );
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/vnd.github.v3+json"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            client_id,
            client_secret,
        }
    }
}

#[cfg(not(tarpaulin_include))]
#[async_trait]
impl GitHubClient for ReqwestGitHubClient {
    async fn exchange_code(&self, _code: &str) -> Result<String, String> {
        unimplemented!("Physical I/O bound. Use Mock in tests.")
    }

    async fn get_user(&self, _token: &str) -> Result<GitHubUser, String> {
        unimplemented!("Physical I/O bound. Use Mock in tests.")
    }

    async fn get_emails(&self, _token: &str) -> Result<Vec<GitHubEmail>, String> {
        unimplemented!("Physical I/O bound. Use Mock in tests.")
    }

    async fn list_orgs(&self, _token: &str) -> Result<Vec<GitHubOrg>, String> {
        unimplemented!("Physical I/O bound. Use Mock in tests.")
    }

    async fn list_repos(&self, _token: &str, _org: &str) -> Result<Vec<GitHubRepo>, String> {
        unimplemented!("Physical I/O bound. Use Mock in tests.")
    }

    async fn create_release(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _tag_name: &str,
        _name: Option<String>,
        _body: Option<String>,
    ) -> Result<GitHubRelease, String> {
        unimplemented!("Physical I/O bound. Use Mock in tests.")
    }

    async fn trigger_workflow(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _workflow_id: &str,
        _ref_branch: &str,
    ) -> Result<(), String> {
        unimplemented!("Physical I/O bound. Use Mock in tests.")
    }

    async fn get_repo_public_key(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
    ) -> Result<GitHubPublicKey, String> {
        unimplemented!("Physical I/O bound. Use Mock in tests.")
    }

    async fn create_repo_secret(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _secret_name: &str,
        _encrypted_value: &str,
        _key_id: &str,
    ) -> Result<(), String> {
        unimplemented!("Physical I/O bound. Use Mock in tests.")
    }
}

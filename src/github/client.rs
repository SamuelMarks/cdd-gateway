#![cfg(not(tarpaulin_include))]

use crate::github::models::*;
use async_trait::async_trait;
use mockall::automock;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

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

#[derive(Serialize)]
struct ExchangeRequest<'a> {
    client_id: &'a str,
    client_secret: &'a str,
    code: &'a str,
}

#[derive(Deserialize)]
struct ExchangeResponse {
    access_token: Option<String>,
    error_description: Option<String>,
}

#[derive(Serialize)]
struct CreateReleaseRequest<'a> {
    tag_name: &'a str,
    name: Option<&'a str>,
    body: Option<&'a str>,
}

#[derive(Serialize)]
struct TriggerWorkflowRequest<'a> {
    #[serde(rename = "ref")]
    ref_branch: &'a str,
}

#[derive(Serialize)]
struct CreateSecretRequest<'a> {
    encrypted_value: &'a str,
    key_id: &'a str,
}

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
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            client_id,
            client_secret,
        }
    }

    fn map_err(e: reqwest::Error) -> String {
        log::error!("GitHub API Error: {}", e);
        e.to_string()
    }

    fn build_request(
        &self,
        method: reqwest::Method,
        url: &str,
        token: &str,
    ) -> reqwest::RequestBuilder {
        self.client.request(method, url).bearer_auth(token)
    }
}

#[async_trait]
impl GitHubClient for ReqwestGitHubClient {
    async fn exchange_code(&self, code: &str) -> Result<String, String> {
        let req = ExchangeRequest {
            client_id: &self.client_id,
            client_secret: &self.client_secret,
            code,
        };
        let res: ExchangeResponse = self
            .client
            .post("https://github.com/login/oauth/access_token")
            .json(&req)
            .send()
            .await
            .map_err(Self::map_err)?
            .json()
            .await
            .map_err(Self::map_err)?;

        if let Some(token) = res.access_token {
            Ok(token)
        } else {
            Err(res
                .error_description
                .unwrap_or_else(|| "Unknown exchange error".into()))
        }
    }

    async fn get_user(&self, token: &str) -> Result<GitHubUser, String> {
        self.build_request(reqwest::Method::GET, "https://api.github.com/user", token)
            .send()
            .await
            .map_err(Self::map_err)?
            .error_for_status()
            .map_err(Self::map_err)?
            .json()
            .await
            .map_err(Self::map_err)
    }

    async fn get_emails(&self, token: &str) -> Result<Vec<GitHubEmail>, String> {
        self.build_request(
            reqwest::Method::GET,
            "https://api.github.com/user/emails",
            token,
        )
        .send()
        .await
        .map_err(Self::map_err)?
        .error_for_status()
        .map_err(Self::map_err)?
        .json()
        .await
        .map_err(Self::map_err)
    }

    async fn list_orgs(&self, token: &str) -> Result<Vec<GitHubOrg>, String> {
        self.build_request(
            reqwest::Method::GET,
            "https://api.github.com/user/orgs",
            token,
        )
        .send()
        .await
        .map_err(Self::map_err)?
        .error_for_status()
        .map_err(Self::map_err)?
        .json()
        .await
        .map_err(Self::map_err)
    }

    async fn list_repos(&self, token: &str, org: &str) -> Result<Vec<GitHubRepo>, String> {
        let url = format!("https://api.github.com/orgs/{}/repos", org);
        self.build_request(reqwest::Method::GET, &url, token)
            .send()
            .await
            .map_err(Self::map_err)?
            .error_for_status()
            .map_err(Self::map_err)?
            .json()
            .await
            .map_err(Self::map_err)
    }

    async fn create_release(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        tag_name: &str,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<GitHubRelease, String> {
        let url = format!("https://api.github.com/repos/{}/{}/releases", owner, repo);
        let req = CreateReleaseRequest {
            tag_name,
            name: name.as_deref(),
            body: body.as_deref(),
        };
        self.build_request(reqwest::Method::POST, &url, token)
            .json(&req)
            .send()
            .await
            .map_err(Self::map_err)?
            .error_for_status()
            .map_err(Self::map_err)?
            .json()
            .await
            .map_err(Self::map_err)
    }

    async fn trigger_workflow(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        workflow_id: &str,
        ref_branch: &str,
    ) -> Result<(), String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/actions/workflows/{}/dispatches",
            owner, repo, workflow_id
        );
        let req = TriggerWorkflowRequest { ref_branch };
        self.build_request(reqwest::Method::POST, &url, token)
            .json(&req)
            .send()
            .await
            .map_err(Self::map_err)?
            .error_for_status()
            .map_err(Self::map_err)?;
        Ok(())
    }

    async fn get_repo_public_key(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<GitHubPublicKey, String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/actions/secrets/public-key",
            owner, repo
        );
        self.build_request(reqwest::Method::GET, &url, token)
            .send()
            .await
            .map_err(Self::map_err)?
            .error_for_status()
            .map_err(Self::map_err)?
            .json()
            .await
            .map_err(Self::map_err)
    }

    async fn create_repo_secret(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        secret_name: &str,
        encrypted_value: &str,
        key_id: &str,
    ) -> Result<(), String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/actions/secrets/{}",
            owner, repo, secret_name
        );
        let req = CreateSecretRequest {
            encrypted_value,
            key_id,
        };
        self.build_request(reqwest::Method::PUT, &url, token)
            .json(&req)
            .send()
            .await
            .map_err(Self::map_err)?
            .error_for_status()
            .map_err(Self::map_err)?;
        Ok(())
    }
}

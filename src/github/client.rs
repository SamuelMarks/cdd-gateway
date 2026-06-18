#![cfg(not(tarpaulin_include))]

use crate::github::models::{
    GitHubEmail, GitHubOrg, GitHubPublicKey, GitHubRelease, GitHubRepo, GitHubUser,
};
use async_trait::async_trait;
use mockall::automock;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// A trait abstracting GitHub API calls
#[automock]
#[async_trait]
pub trait GitHubClient: Send + Sync {
    /// Exchanges an OAuth code for an access token
    async fn exchange_code(&self, code: &str) -> Result<String, String>;

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

/// Reqwest implementation of `GitHubClient`
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
    /// Create a new `ReqwestGitHubClient`
    /// # Errors
    /// error
    pub fn new(
        client_id: String,
        client_secret: String,
    ) -> Result<Self, crate::error::CddGatewayError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static("cdd-gateway"),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/vnd.github.v3+json"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()?;

        Ok(Self {
            client,
            client_id,
            client_secret,
        })
    }

    fn map_err(e: &reqwest::Error) -> String {
        log::error!("GitHub API Error: {e}");
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
            .map_err(|e| Self::map_err(&e))?
            .json()
            .await
            .map_err(|e| Self::map_err(&e))?;

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
            .map_err(|e| Self::map_err(&e))?
            .error_for_status()
            .map_err(|e| Self::map_err(&e))?
            .json()
            .await
            .map_err(|e| Self::map_err(&e))
    }

    async fn get_emails(&self, token: &str) -> Result<Vec<GitHubEmail>, String> {
        self.build_request(
            reqwest::Method::GET,
            "https://api.github.com/user/emails",
            token,
        )
        .send()
        .await
        .map_err(|e| Self::map_err(&e))?
        .error_for_status()
        .map_err(|e| Self::map_err(&e))?
        .json()
        .await
        .map_err(|e| Self::map_err(&e))
    }

    async fn list_orgs(&self, token: &str) -> Result<Vec<GitHubOrg>, String> {
        self.build_request(
            reqwest::Method::GET,
            "https://api.github.com/user/orgs",
            token,
        )
        .send()
        .await
        .map_err(|e| Self::map_err(&e))?
        .error_for_status()
        .map_err(|e| Self::map_err(&e))?
        .json()
        .await
        .map_err(|e| Self::map_err(&e))
    }

    async fn list_repos(&self, token: &str, org: &str) -> Result<Vec<GitHubRepo>, String> {
        let url = format!("https://api.github.com/orgs/{org}/repos");
        self.build_request(reqwest::Method::GET, &url, token)
            .send()
            .await
            .map_err(|e| Self::map_err(&e))?
            .error_for_status()
            .map_err(|e| Self::map_err(&e))?
            .json()
            .await
            .map_err(|e| Self::map_err(&e))
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
        let url = format!("https://api.github.com/repos/{owner}/{repo}/releases");
        let req = CreateReleaseRequest {
            tag_name,
            name: name.as_deref(),
            body: body.as_deref(),
        };
        self.build_request(reqwest::Method::POST, &url, token)
            .json(&req)
            .send()
            .await
            .map_err(|e| Self::map_err(&e))?
            .error_for_status()
            .map_err(|e| Self::map_err(&e))?
            .json()
            .await
            .map_err(|e| Self::map_err(&e))
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
            "https://api.github.com/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches"
        );
        let req = TriggerWorkflowRequest { ref_branch };
        self.build_request(reqwest::Method::POST, &url, token)
            .json(&req)
            .send()
            .await
            .map_err(|e| Self::map_err(&e))?
            .error_for_status()
            .map_err(|e| Self::map_err(&e))?;
        Ok(())
    }

    async fn get_repo_public_key(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<GitHubPublicKey, String> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/actions/secrets/public-key");
        self.build_request(reqwest::Method::GET, &url, token)
            .send()
            .await
            .map_err(|e| Self::map_err(&e))?
            .error_for_status()
            .map_err(|e| Self::map_err(&e))?
            .json()
            .await
            .map_err(|e| Self::map_err(&e))
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
        let url =
            format!("https://api.github.com/repos/{owner}/{repo}/actions/secrets/{secret_name}");
        let req = CreateSecretRequest {
            encrypted_value,
            key_id,
        };
        self.build_request(reqwest::Method::PUT, &url, token)
            .json(&req)
            .send()
            .await
            .map_err(|e| Self::map_err(&e))?
            .error_for_status()
            .map_err(|e| Self::map_err(&e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn test_new_client() {
        let client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())
            .unwrap_or_else(|_| panic!("valid test client"));
        assert_eq!(client.client_id, "id");
    }

    #[actix_web::test]
    async fn test_get_user() {
        let client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())
            .unwrap_or_else(|_| panic!("valid test client"));
        let res = client.get_user("bad_token").await;
        assert!(res.is_err());
    }

    #[actix_web::test]
    async fn test_get_user_emails() {
        let client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())
            .unwrap_or_else(|_| panic!("valid test client"));
        let res = client.get_emails("bad_token").await;
        assert!(res.is_err());
    }

    #[actix_web::test]
    async fn test_exchange_code() {
        let client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())
            .unwrap_or_else(|_| panic!("valid test client"));
        let res = client.exchange_code("bad_code").await;
        assert!(res.is_err());
    }

    #[actix_web::test]
    async fn test_list_orgs() {
        let client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())
            .unwrap_or_else(|_| panic!("valid test client"));
        let res = client.list_orgs("bad_token").await;
        assert!(res.is_err());
    }

    #[actix_web::test]
    async fn test_list_repos() {
        let client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())
            .unwrap_or_else(|_| panic!("valid test client"));
        let res = client.list_repos("org", "bad_token").await;
        assert!(res.is_err());
    }

    #[actix_web::test]
    async fn test_get_repo_public_key() {
        let client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())
            .unwrap_or_else(|_| panic!("valid test client"));
        let res = client
            .get_repo_public_key("owner", "repo", "bad_token")
            .await;
        assert!(res.is_err());
    }

    #[actix_web::test]
    async fn test_create_repo_secret() {
        let client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())
            .unwrap_or_else(|_| panic!("valid test client"));
        let res = client
            .create_repo_secret("owner", "repo", "key", "val", "kid", "bad_token")
            .await;
        assert!(res.is_err());
    }

    #[actix_web::test]
    async fn test_trigger_workflow() {
        let client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())
            .unwrap_or_else(|_| panic!("valid test client"));
        let res = client
            .trigger_workflow("owner", "repo", "wf", "ref", "bad_token")
            .await;
        assert!(res.is_err());
    }

    #[actix_web::test]
    async fn test_create_release() {
        let client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())
            .unwrap_or_else(|_| panic!("valid test client"));
        let res = client
            .create_release("token", "owner", "repo", "tag", None, None)
            .await;
        assert!(res.is_err());
    }
}

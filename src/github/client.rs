#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! GitHub REST API and OAuth client trait and `reqwest` implementation.

use crate::error::CddGatewayError;
use crate::github::models::{
    GitHubEmail, GitHubOrg, GitHubPublicKey, GitHubRelease, GitHubRepo, GitHubUser,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Trait abstracting interactions with the GitHub OAuth and REST v3 APIs.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait GitHubClient: Send + Sync {
    /// Exchanges an OAuth authorization code for a GitHub access token.
    ///
    /// # Arguments
    /// * `code` - Temporary OAuth authorization code received from GitHub.
    ///
    /// # Returns
    /// * `Ok(String)` - The issued GitHub OAuth access token.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the HTTP request fails or GitHub rejects the code.
    async fn exchange_code(&self, code: &str) -> Result<String, CddGatewayError>;

    /// Fetches the authenticated user's GitHub profile (`GET /user`).
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    ///
    /// # Returns
    /// * `Ok(GitHubUser)` - The authenticated user's profile.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns a non-success status.
    async fn get_user(&self, token: &str) -> Result<GitHubUser, CddGatewayError>;

    /// Fetches the authenticated user's email addresses (`GET /user/emails`).
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    ///
    /// # Returns
    /// * `Ok(Vec<GitHubEmail>)` - List of email records associated with the account.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns a non-success status.
    async fn get_emails(&self, token: &str) -> Result<Vec<GitHubEmail>, CddGatewayError>;

    /// Lists organizations the authenticated user belongs to (`GET /user/orgs`).
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    ///
    /// # Returns
    /// * `Ok(Vec<GitHubOrg>)` - List of organizations.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns a non-success status.
    async fn list_orgs(&self, token: &str) -> Result<Vec<GitHubOrg>, CddGatewayError>;

    /// Lists repositories belonging to the specified organization (`GET /orgs/{org}/repos`).
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    /// * `org` - Organization login handle.
    ///
    /// # Returns
    /// * `Ok(Vec<GitHubRepo>)` - List of repositories in the organization.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns a non-success status.
    async fn list_repos(&self, token: &str, org: &str) -> Result<Vec<GitHubRepo>, CddGatewayError>;

    /// Fetches a single GitHub repository (`GET /repos/{owner}/{repo}`), returning `Ok(None)` on `404 Not Found`.
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    /// * `owner` - Repository owner or organization login.
    /// * `repo` - Repository name.
    ///
    /// # Returns
    /// * `Ok(Some(GitHubRepo))` - If the repository exists on GitHub.
    /// * `Ok(None)` - If GitHub responds with `404 Not Found`.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns another non-success status.
    async fn get_repo(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<Option<GitHubRepo>, CddGatewayError>;

    /// Creates a new repository inside a GitHub organization (`POST /orgs/{org}/repos`).
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    /// * `org` - Organization login handle (`${GITHUB_ORG_NAME}`).
    /// * `name` - Repository name.
    /// * `description` - Optional repository description.
    /// * `private` - Whether the created repository should be private.
    ///
    /// # Returns
    /// * `Ok(GitHubRepo)` - The created GitHub repository metadata.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns a non-success status.
    async fn create_org_repo(
        &self,
        token: &str,
        org: &str,
        name: &str,
        description: Option<String>,
        private: bool,
    ) -> Result<GitHubRepo, CddGatewayError>;

    /// Creates a new repository under the authenticated user's personal namespace (`POST /user/repos`).
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    /// * `name` - Repository name.
    /// * `description` - Optional repository description.
    /// * `private` - Whether the created repository should be private.
    ///
    /// # Returns
    /// * `Ok(GitHubRepo)` - The created GitHub repository metadata.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns a non-success status.
    async fn create_user_repo(
        &self,
        token: &str,
        name: &str,
        description: Option<String>,
        private: bool,
    ) -> Result<GitHubRepo, CddGatewayError>;

    /// Creates a new release on a repository (`POST /repos/{owner}/{repo}/releases`).
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    /// * `owner` - Repository owner or organization login.
    /// * `repo` - Repository name.
    /// * `tag_name` - Git tag name for the release.
    /// * `name` - Optional release title.
    /// * `body` - Optional release notes markdown body.
    ///
    /// # Returns
    /// * `Ok(GitHubRelease)` - The created GitHub release metadata.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns a non-success status.
    async fn create_release(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        tag_name: &str,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<GitHubRelease, CddGatewayError>;

    /// Dispatches a GitHub Actions workflow run
    /// (`POST /repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches`).
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    /// * `owner` - Repository owner or organization login.
    /// * `repo` - Repository name.
    /// * `workflow_id` - Workflow filename or numeric ID.
    /// * `ref_branch` - Git branch or tag reference to run the workflow on.
    ///
    /// # Returns
    /// * `Ok(())` - If the workflow dispatch succeeded.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns a non-success status.
    async fn trigger_workflow(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        workflow_id: &str,
        ref_branch: &str,
    ) -> Result<(), CddGatewayError>;

    /// Retrieves a repository's public key for encrypting GitHub Actions secrets
    /// (`GET /repos/{owner}/{repo}/actions/secrets/public-key`).
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    /// * `owner` - Repository owner or organization login.
    /// * `repo` - Repository name.
    ///
    /// # Returns
    /// * `Ok(GitHubPublicKey)` - The repository public key metadata.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns a non-success status.
    async fn get_repo_public_key(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<GitHubPublicKey, CddGatewayError>;

    /// Creates or updates a repository GitHub Actions secret
    /// (`PUT /repos/{owner}/{repo}/actions/secrets/{secret_name}`).
    ///
    /// # Arguments
    /// * `token` - GitHub OAuth or personal access token.
    /// * `owner` - Repository owner or organization login.
    /// * `repo` - Repository name.
    /// * `secret_name` - Name of the secret to create or update.
    /// * `encrypted_value` - Libsodium sealed-box Base64-encoded secret value.
    /// * `key_id` - ID of the public key used to encrypt `encrypted_value`.
    ///
    /// # Returns
    /// * `Ok(())` - If the secret was created or updated.
    ///
    /// # Errors
    /// * [`CddGatewayError`] - If the request fails or returns a non-success status.
    async fn create_repo_secret(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        secret_name: &str,
        encrypted_value: &str,
        key_id: &str,
    ) -> Result<(), CddGatewayError>;
}

/// Concrete [`GitHubClient`] implementation backed by [`reqwest::Client`].
pub struct ReqwestGitHubClient {
    /// Underlying asynchronous HTTP client.
    client: Client,
    /// GitHub OAuth application client ID.
    pub client_id: String,
    /// GitHub OAuth application client secret.
    pub client_secret: String,
    /// Base URL for the GitHub REST API (`https://api.github.com` by default).
    pub api_base_url: String,
    /// Base URL for GitHub OAuth endpoints (`https://github.com` by default).
    pub github_base_url: String,
}

impl ReqwestGitHubClient {
    /// Creates a new [`ReqwestGitHubClient`] configured with a `cdd-gateway` user agent.
    ///
    /// # Arguments
    /// * `client_id` - GitHub OAuth application client ID.
    /// * `client_secret` - GitHub OAuth application client secret.
    ///
    /// # Returns
    /// * `Ok(ReqwestGitHubClient)` - Initialized GitHub HTTP client.
    ///
    /// # Errors
    /// * [`CddGatewayError::Http`] - If building the underlying `reqwest::Client` fails.
    pub fn new(client_id: String, client_secret: String) -> Result<Self, CddGatewayError> {
        let client = Client::builder()
            .user_agent(concat!("cdd-gateway/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            client_id,
            client_secret,
            api_base_url: "https://api.github.com".to_string(),
            github_base_url: "https://github.com".to_string(),
        })
    }
}

/// Request body sent to `POST /login/oauth/access_token`.
#[derive(Serialize)]
struct ExchangeRequest<'a> {
    /// GitHub OAuth client ID.
    client_id: &'a str,
    /// GitHub OAuth client secret.
    client_secret: &'a str,
    /// Authorization code being exchanged.
    code: &'a str,
}

/// Response body returned by `POST /login/oauth/access_token`.
#[derive(Deserialize)]
struct ExchangeResponse {
    /// Issued OAuth access token, if successful.
    access_token: Option<String>,
    /// Error code returned by GitHub OAuth, if failed.
    error: Option<String>,
    /// Detailed error description returned by GitHub OAuth, if failed.
    error_description: Option<String>,
}

/// Request body sent to `POST /orgs/{org}/repos` or `POST /user/repos`.
#[derive(Serialize)]
struct CreateRepoReq<'a> {
    /// Repository name.
    name: &'a str,
    /// Optional repository description.
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// Whether the repository is private.
    private: bool,
}

/// Request body sent to `POST /repos/{owner}/{repo}/releases`.
#[derive(Serialize)]
struct CreateReleaseReq<'a> {
    /// Release Git tag name.
    tag_name: &'a str,
    /// Optional release title.
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// Optional release notes body.
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
}

/// Request body sent to `POST /repos/{owner}/{repo}/actions/workflows/{id}/dispatches`.
#[derive(Serialize)]
struct TriggerWorkflowReq<'a> {
    /// Target Git branch or tag reference.
    #[serde(rename = "ref")]
    ref_branch: &'a str,
}

/// Request body sent to `PUT /repos/{owner}/{repo}/actions/secrets/{secret_name}`.
#[derive(Serialize)]
struct CreateSecretReq<'a> {
    /// Encrypted secret value.
    encrypted_value: &'a str,
    /// ID of the public key used for encryption.
    key_id: &'a str,
}

#[async_trait]
impl GitHubClient for ReqwestGitHubClient {
    async fn exchange_code(&self, code: &str) -> Result<String, CddGatewayError> {
        let req_body = ExchangeRequest {
            client_id: &self.client_id,
            client_secret: &self.client_secret,
            code,
        };

        let res = self
            .client
            .post(format!("{}/login/oauth/access_token", self.github_base_url))
            .header("Accept", "application/json")
            .json(&req_body)
            .send()
            .await?
            .error_for_status()?;

        let parsed: ExchangeResponse = res.json().await?;

        if let Some(token) = parsed.access_token {
            Ok(token)
        } else {
            Err(CddGatewayError::Unauthorized(
                parsed
                    .error_description
                    .or(parsed.error)
                    .unwrap_or_else(|| "Unknown exchange error".to_string()),
            ))
        }
    }

    async fn get_user(&self, token: &str) -> Result<GitHubUser, CddGatewayError> {
        let res = self
            .client
            .get(format!("{}/user", self.api_base_url))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json::<GitHubUser>().await?)
    }

    async fn get_emails(&self, token: &str) -> Result<Vec<GitHubEmail>, CddGatewayError> {
        let res = self
            .client
            .get(format!("{}/user/emails", self.api_base_url))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json::<Vec<GitHubEmail>>().await?)
    }

    async fn list_orgs(&self, token: &str) -> Result<Vec<GitHubOrg>, CddGatewayError> {
        let res = self
            .client
            .get(format!("{}/user/orgs", self.api_base_url))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json::<Vec<GitHubOrg>>().await?)
    }

    async fn list_repos(&self, token: &str, org: &str) -> Result<Vec<GitHubRepo>, CddGatewayError> {
        let url = format!("{}/orgs/{org}/repos", self.api_base_url);
        let res = self
            .client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json::<Vec<GitHubRepo>>().await?)
    }

    async fn get_repo(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<Option<GitHubRepo>, CddGatewayError> {
        let url = format!("{}/repos/{owner}/{repo}", self.api_base_url);
        let res = self
            .client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let res = res.error_for_status()?;
        Ok(Some(res.json::<GitHubRepo>().await?))
    }

    async fn create_org_repo(
        &self,
        token: &str,
        org: &str,
        name: &str,
        description: Option<String>,
        private: bool,
    ) -> Result<GitHubRepo, CddGatewayError> {
        let url = format!("{}/orgs/{org}/repos", self.api_base_url);
        let req_body = CreateRepoReq {
            name,
            description,
            private,
        };

        let res = self
            .client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .json(&req_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json::<GitHubRepo>().await?)
    }

    async fn create_user_repo(
        &self,
        token: &str,
        name: &str,
        description: Option<String>,
        private: bool,
    ) -> Result<GitHubRepo, CddGatewayError> {
        let url = format!("{}/user/repos", self.api_base_url);
        let req_body = CreateRepoReq {
            name,
            description,
            private,
        };

        let res = self
            .client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .json(&req_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json::<GitHubRepo>().await?)
    }

    async fn create_release(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        tag_name: &str,
        name: Option<String>,
        body: Option<String>,
    ) -> Result<GitHubRelease, CddGatewayError> {
        let url = format!("{}/repos/{owner}/{repo}/releases", self.api_base_url);
        let req_body = CreateReleaseReq {
            tag_name,
            name,
            body,
        };

        let res = self
            .client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .json(&req_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json::<GitHubRelease>().await?)
    }

    async fn trigger_workflow(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        workflow_id: &str,
        ref_branch: &str,
    ) -> Result<(), CddGatewayError> {
        let url = format!(
            "{}/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches",
            self.api_base_url
        );
        let req_body = TriggerWorkflowReq { ref_branch };

        self.client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .json(&req_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn get_repo_public_key(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<GitHubPublicKey, CddGatewayError> {
        let url = format!(
            "{}/repos/{owner}/{repo}/actions/secrets/public-key",
            self.api_base_url
        );
        let res = self
            .client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json::<GitHubPublicKey>().await?)
    }

    async fn create_repo_secret(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        secret_name: &str,
        encrypted_value: &str,
        key_id: &str,
    ) -> Result<(), CddGatewayError> {
        let url = format!(
            "{}/repos/{owner}/{repo}/actions/secrets/{secret_name}",
            self.api_base_url
        );
        let req_body = CreateSecretReq {
            encrypted_value,
            key_id,
        };

        self.client
            .put(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github.v3+json")
            .json(&req_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use httpmock::MockServer;

    fn make_test_client(server: &MockServer) -> Result<ReqwestGitHubClient, CddGatewayError> {
        let mut client = ReqwestGitHubClient::new("id".to_string(), "sec".to_string())?;
        client.api_base_url = server.base_url();
        client.github_base_url = server.base_url();
        Ok(client)
    }

    #[actix_web::test]
    async fn test_exchange_code_success_and_errors() -> Result<(), CddGatewayError> {
        let server = MockServer::start();
        let client = make_test_client(&server)?;

        let mut m_ok = server.mock(|when, then| {
            when.method(POST).path("/login/oauth/access_token");
            then.status(200)
                .json_body(serde_json::json!({ "access_token": "gho_valid123" }));
        });
        let token = client.exchange_code("code_ok").await?;
        assert_eq!(token, "gho_valid123");
        m_ok.delete();

        let mut m_desc = server.mock(|when, then| {
            when.method(POST).path("/login/oauth/access_token");
            then.status(200).json_body(serde_json::json!({
                "error": "bad_verification_code",
                "error_description": "The code passed is incorrect or expired."
            }));
        });
        let err_desc = client.exchange_code("code_bad").await;
        assert!(err_desc.is_err());
        if let Err(e) = err_desc {
            assert!(e
                .to_string()
                .contains("The code passed is incorrect or expired."));
        }
        m_desc.delete();

        let mut m_unknown = server.mock(|when, then| {
            when.method(POST).path("/login/oauth/access_token");
            then.status(200).json_body(serde_json::json!({}));
        });
        let err_unknown = client.exchange_code("code_empty").await;
        assert!(err_unknown.is_err());
        if let Err(e) = err_unknown {
            assert!(e.to_string().contains("Unknown exchange error"));
        }
        m_unknown.delete();

        let _m_500 = server.mock(|when, then| {
            when.method(POST).path("/login/oauth/access_token");
            then.status(500);
        });
        assert!(client.exchange_code("code_500").await.is_err());

        Ok(())
    }

    #[actix_web::test]
    async fn test_get_user_and_emails_success_and_errors() -> Result<(), CddGatewayError> {
        let server = MockServer::start();
        let client = make_test_client(&server)?;

        let mut m_user = server.mock(|when, then| {
            when.method(GET).path("/user");
            then.status(200).json_body(serde_json::json!({
                "id": 42,
                "login": "octocat",
                "avatar_url": "https://example.com/a.png"
            }));
        });
        let user = client.get_user("tok").await?;
        assert_eq!(user.id, 42);
        assert_eq!(user.login, "octocat");
        m_user.delete();

        let mut m_user_bad = server.mock(|when, then| {
            when.method(GET).path("/user");
            then.status(200).body("bad json");
        });
        assert!(client.get_user("tok").await.is_err());
        m_user_bad.delete();

        let _m_user_500 = server.mock(|when, then| {
            when.method(GET).path("/user");
            then.status(500);
        });
        assert!(client.get_user("tok").await.is_err());

        let mut m_emails = server.mock(|when, then| {
            when.method(GET).path("/user/emails");
            then.status(200).json_body(serde_json::json!([
                {
                    "email": "octocat@github.com",
                    "primary": true,
                    "verified": true
                }
            ]));
        });
        let emails = client.get_emails("tok").await?;
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].email, "octocat@github.com");
        m_emails.delete();

        let mut m_emails_bad = server.mock(|when, then| {
            when.method(GET).path("/user/emails");
            then.status(200).body("bad json");
        });
        assert!(client.get_emails("tok").await.is_err());
        m_emails_bad.delete();

        let _m_emails_500 = server.mock(|when, then| {
            when.method(GET).path("/user/emails");
            then.status(500);
        });
        assert!(client.get_emails("tok").await.is_err());

        Ok(())
    }

    #[actix_web::test]
    async fn test_orgs_and_repos_success_and_errors() -> Result<(), CddGatewayError> {
        let server = MockServer::start();
        let client = make_test_client(&server)?;

        let mut m_orgs = server.mock(|when, then| {
            when.method(GET).path("/user/orgs");
            then.status(200).json_body(serde_json::json!([
                {
                    "id": 100,
                    "login": "my-org",
                    "description": "Org desc"
                }
            ]));
        });
        let orgs = client.list_orgs("tok").await?;
        assert_eq!(orgs.len(), 1);
        assert_eq!(orgs[0].login, "my-org");
        m_orgs.delete();

        let mut m_orgs_bad = server.mock(|when, then| {
            when.method(GET).path("/user/orgs");
            then.status(200).body("bad json");
        });
        assert!(client.list_orgs("tok").await.is_err());
        m_orgs_bad.delete();

        let _m_orgs_500 = server.mock(|when, then| {
            when.method(GET).path("/user/orgs");
            then.status(500);
        });
        assert!(client.list_orgs("tok").await.is_err());

        let mut m_repos = server.mock(|when, then| {
            when.method(GET).path("/orgs/my-org/repos");
            then.status(200).json_body(serde_json::json!([
                {
                    "id": 200,
                    "name": "my-repo",
                    "full_name": "my-org/my-repo",
                    "description": null
                }
            ]));
        });
        let repos = client.list_repos("tok", "my-org").await?;
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name, "my-org/my-repo");
        m_repos.delete();

        let mut m_repos_bad = server.mock(|when, then| {
            when.method(GET).path("/orgs/my-org/repos");
            then.status(200).body("bad json");
        });
        assert!(client.list_repos("tok", "my-org").await.is_err());
        m_repos_bad.delete();

        let mut m_repos_500 = server.mock(|when, then| {
            when.method(GET).path("/orgs/my-org/repos");
            then.status(500);
        });
        assert!(client.list_repos("tok", "my-org").await.is_err());
        m_repos_500.delete();

        // get_repo (200, 404, bad json, 500)
        let mut m_get_repo = server.mock(|when, then| {
            when.method(GET).path("/repos/my-org/my-repo");
            then.status(200).json_body(serde_json::json!({
                "id": 200,
                "name": "my-repo",
                "full_name": "my-org/my-repo",
                "description": null
            }));
        });
        let found_repo = client.get_repo("tok", "my-org", "my-repo").await?;
        assert!(found_repo.is_some());
        m_get_repo.delete();

        let mut m_get_repo_404 = server.mock(|when, then| {
            when.method(GET).path("/repos/my-org/missing");
            then.status(404);
        });
        let missing_repo = client.get_repo("tok", "my-org", "missing").await?;
        assert!(missing_repo.is_none());
        m_get_repo_404.delete();

        let mut m_get_repo_bad = server.mock(|when, then| {
            when.method(GET).path("/repos/my-org/bad");
            then.status(200).body("bad json");
        });
        assert!(client.get_repo("tok", "my-org", "bad").await.is_err());
        m_get_repo_bad.delete();

        let mut m_get_repo_500 = server.mock(|when, then| {
            when.method(GET).path("/repos/my-org/err");
            then.status(500);
        });
        assert!(client.get_repo("tok", "my-org", "err").await.is_err());
        m_get_repo_500.delete();

        // create_org_repo
        let mut m_create_org = server.mock(|when, then| {
            when.method(POST).path("/orgs/my-org/repos");
            then.status(201).json_body(serde_json::json!({
                "id": 201,
                "name": "my-org-sdk",
                "full_name": "my-org/my-org-sdk",
                "description": "SDK"
            }));
        });
        let created_org_repo = client
            .create_org_repo("tok", "my-org", "my-org-sdk", Some("SDK".into()), false)
            .await?;
        assert_eq!(created_org_repo.id, 201);
        assert_eq!(created_org_repo.name, "my-org-sdk");
        m_create_org.delete();

        let mut m_create_org_bad = server.mock(|when, then| {
            when.method(POST).path("/orgs/my-org/repos");
            then.status(200).body("bad json");
        });
        assert!(client
            .create_org_repo("tok", "my-org", "my-org-sdk", None, false)
            .await
            .is_err());
        m_create_org_bad.delete();

        let _m_create_org_500 = server.mock(|when, then| {
            when.method(POST).path("/orgs/my-org/repos");
            then.status(500);
        });
        assert!(client
            .create_org_repo("tok", "my-org", "my-org-sdk", None, false)
            .await
            .is_err());

        // create_user_repo
        let mut m_create_user = server.mock(|when, then| {
            when.method(POST).path("/user/repos");
            then.status(201).json_body(serde_json::json!({
                "id": 202,
                "name": "octocat-sdk",
                "full_name": "octocat/octocat-sdk",
                "description": "User SDK"
            }));
        });
        let created_user_repo = client
            .create_user_repo("tok", "octocat-sdk", Some("User SDK".into()), true)
            .await?;
        assert_eq!(created_user_repo.id, 202);
        assert_eq!(created_user_repo.name, "octocat-sdk");
        m_create_user.delete();

        let mut m_create_user_bad = server.mock(|when, then| {
            when.method(POST).path("/user/repos");
            then.status(200).body("bad json");
        });
        assert!(client
            .create_user_repo("tok", "octocat-sdk", None, false)
            .await
            .is_err());
        m_create_user_bad.delete();

        let _m_create_user_500 = server.mock(|when, then| {
            when.method(POST).path("/user/repos");
            then.status(500);
        });
        assert!(client
            .create_user_repo("tok", "octocat-sdk", None, false)
            .await
            .is_err());

        Ok(())
    }

    #[actix_web::test]
    async fn test_releases_workflows_and_secrets_success_and_errors() -> Result<(), CddGatewayError>
    {
        let server = MockServer::start();
        let client = make_test_client(&server)?;

        let mut m_rel = server.mock(|when, then| {
            when.method(POST).path("/repos/owner/repo/releases");
            then.status(201).json_body(serde_json::json!({
                "id": 300,
                "tag_name": "v1.0.0",
                "name": "v1.0.0",
                "body": "notes"
            }));
        });
        let rel = client
            .create_release(
                "tok",
                "owner",
                "repo",
                "v1.0.0",
                Some("v1.0.0".into()),
                Some("notes".into()),
            )
            .await?;
        assert_eq!(rel.id, 300);
        assert_eq!(rel.tag_name, "v1.0.0");
        m_rel.delete();

        let mut m_rel_bad = server.mock(|when, then| {
            when.method(POST).path("/repos/owner/repo/releases");
            then.status(200).body("bad json");
        });
        assert!(client
            .create_release("tok", "owner", "repo", "v1.0.0", None, None)
            .await
            .is_err());
        m_rel_bad.delete();

        let _m_rel_500 = server.mock(|when, then| {
            when.method(POST).path("/repos/owner/repo/releases");
            then.status(500);
        });
        assert!(client
            .create_release("tok", "owner", "repo", "v1.0.0", None, None)
            .await
            .is_err());

        let mut m_wf = server.mock(|when, then| {
            when.method(POST)
                .path("/repos/owner/repo/actions/workflows/ci.yml/dispatches");
            then.status(204);
        });
        client
            .trigger_workflow("tok", "owner", "repo", "ci.yml", "main")
            .await?;
        m_wf.delete();

        let _m_wf_500 = server.mock(|when, then| {
            when.method(POST)
                .path("/repos/owner/repo/actions/workflows/ci.yml/dispatches");
            then.status(500);
        });
        assert!(client
            .trigger_workflow("tok", "owner", "repo", "ci.yml", "main")
            .await
            .is_err());

        let mut m_pk = server.mock(|when, then| {
            when.method(GET)
                .path("/repos/owner/repo/actions/secrets/public-key");
            then.status(200).json_body(serde_json::json!({
                "key_id": "kid_1",
                "key": "base64key=="
            }));
        });
        let pk = client.get_repo_public_key("tok", "owner", "repo").await?;
        assert_eq!(pk.key_id, "kid_1");
        m_pk.delete();

        let mut m_pk_bad = server.mock(|when, then| {
            when.method(GET)
                .path("/repos/owner/repo/actions/secrets/public-key");
            then.status(200).body("bad json");
        });
        assert!(client
            .get_repo_public_key("tok", "owner", "repo")
            .await
            .is_err());
        m_pk_bad.delete();

        let _m_pk_500 = server.mock(|when, then| {
            when.method(GET)
                .path("/repos/owner/repo/actions/secrets/public-key");
            then.status(500);
        });
        assert!(client
            .get_repo_public_key("tok", "owner", "repo")
            .await
            .is_err());

        let mut m_sec = server.mock(|when, then| {
            when.method(PUT)
                .path("/repos/owner/repo/actions/secrets/MY_SECRET");
            then.status(201);
        });
        client
            .create_repo_secret("tok", "owner", "repo", "MY_SECRET", "enc_val", "kid_1")
            .await?;
        m_sec.delete();

        let _m_sec_500 = server.mock(|when, then| {
            when.method(PUT)
                .path("/repos/owner/repo/actions/secrets/MY_SECRET");
            then.status(500);
        });
        assert!(client
            .create_repo_secret("tok", "owner", "repo", "MY_SECRET", "enc_val", "kid_1")
            .await
            .is_err());

        Ok(())
    }
}

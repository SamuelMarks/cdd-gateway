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
    /// The reqwest client
    client: Client,
    /// The GitHub OAuth client ID
    client_id: String,
    /// The GitHub OAuth client secret
    client_secret: String,
    /// API base URL
    pub api_base_url: String,
    /// HTML base URL
    pub html_base_url: String,
}

/// Exchange request payload
#[derive(Serialize)]
struct ExchangeRequest<'a> {
    /// The client ID
    client_id: &'a str,
    /// The client secret
    client_secret: &'a str,
    /// The authorization code
    code: &'a str,
}

/// Exchange response payload
#[derive(Deserialize)]
struct ExchangeResponse {
    /// The access token
    access_token: Option<String>,
    /// Error description if the exchange failed
    error_description: Option<String>,
}

/// Request payload for creating a release
#[derive(Serialize)]
struct CreateReleaseRequest<'a> {
    /// The tag name for the release
    tag_name: &'a str,
    /// The name of the release
    name: Option<&'a str>,
    /// The body/description of the release
    body: Option<&'a str>,
}

/// Request payload for triggering a workflow
#[derive(Serialize)]
struct TriggerWorkflowRequest<'a> {
    /// The ref/branch to trigger the workflow on
    #[serde(rename = "ref")]
    ref_branch: &'a str,
}

/// Request payload for creating a secret
#[derive(Serialize)]
struct CreateSecretRequest<'a> {
    /// The encrypted value of the secret
    encrypted_value: &'a str,
    /// The key ID used for encryption
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
            api_base_url: "https://api.github.com".to_string(),
            html_base_url: "https://github.com".to_string(),
        })
    }

    /// Helper to map reqwest errors to strings
    fn map_err(e: &reqwest::Error) -> String {
        log::error!("GitHub API Error: {e}");
        e.to_string()
    }

    /// Helper to build a request with the required headers
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
            .post(format!("{}/login/oauth/access_token", self.html_base_url))
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
        self.build_request(
            reqwest::Method::GET,
            &format!("{}/user", self.api_base_url),
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

    async fn get_emails(&self, token: &str) -> Result<Vec<GitHubEmail>, String> {
        self.build_request(
            reqwest::Method::GET,
            &format!("{}/user/emails", self.api_base_url),
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
            &format!("{}/user/orgs", self.api_base_url),
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
        let url = format!("{}/orgs/{org}/repos", self.api_base_url);
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
        let url = format!("{}/repos/{owner}/{repo}/releases", self.api_base_url);
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
            "{}/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches",
            self.api_base_url
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
        let url = format!(
            "{}/repos/{owner}/{repo}/actions/secrets/public-key",
            self.api_base_url
        );
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
        let url = format!(
            "{}/repos/{owner}/{repo}/actions/secrets/{secret_name}",
            self.api_base_url
        );
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

    #[actix_web::test]
    async fn test_network_errors() {
        let mut client = match ReqwestGitHubClient::new("id".to_string(), "sec".to_string()) {
            Ok(c) => c,
            Err(e) => panic!("Failed to create client: {e}"),
        };
        client.api_base_url = "http://127.0.0.1:1".to_string();
        client.html_base_url = "http://127.0.0.1:1".to_string();

        let token = "dummy";

        let res = client.exchange_code("code").await;
        assert!(res.is_err());

        let res = client.get_user(token).await;
        assert!(res.is_err());

        let res = client.get_emails(token).await;
        assert!(res.is_err());

        let res = client.list_orgs(token).await;
        assert!(res.is_err());

        let res = client.list_repos("org", token).await;
        assert!(res.is_err());

        let res = client.get_repo_public_key("owner", "repo", token).await;
        assert!(res.is_err());

        let res = client
            .create_repo_secret("owner", "repo", "key", "val", "kid", token)
            .await;
        assert!(res.is_err());

        let res = client
            .trigger_workflow("owner", "repo", "wf", "ref", token)
            .await;
        assert!(res.is_err());

        let res = client
            .create_release(token, "owner", "repo", "tag", None, None)
            .await;
        assert!(res.is_err());
    }

    #[actix_web::test]
    async fn test_exchange_code_no_token() {
        use httpmock::prelude::*;
        use httpmock::MockServer;

        let server = MockServer::start();
        let mut client = match ReqwestGitHubClient::new("id".to_string(), "sec".to_string()) {
            Ok(c) => c,
            Err(e) => panic!("Failed to create client: {e}"),
        };
        client.html_base_url = server.base_url();

        let _mock = server.mock(|when, then| {
            when.method(POST).path("/login/oauth/access_token");
            then.status(200).json_body(serde_json::json!({
                "error_description": "bad code"
            }));
        });

        let res = client.exchange_code("code").await;
        let Err(err) = res else {
            panic!("Expected error")
        };
        assert_eq!(err, "bad code");

        let server2 = MockServer::start();
        let mut client2 = match ReqwestGitHubClient::new("id".to_string(), "sec".to_string()) {
            Ok(c) => c,
            Err(e) => panic!("Failed to create client: {e}"),
        };
        client2.html_base_url = server2.base_url();

        let _mock2 = server2.mock(|when, then| {
            when.method(POST).path("/login/oauth/access_token");
            then.status(200).json_body(serde_json::json!({
                "error": "true"
            }));
        });

        let res2 = client2.exchange_code("code2").await;
        let Err(err2) = res2 else {
            panic!("Expected error")
        };
        assert_eq!(err2, "Unknown exchange error");
    }

    #[actix_web::test]
    async fn test_api_error_responses() {
        use httpmock::prelude::*;
        use httpmock::MockServer;

        let server = MockServer::start();
        let mut client = match ReqwestGitHubClient::new("id".to_string(), "sec".to_string()) {
            Ok(c) => c,
            Err(e) => panic!("Failed to create client: {e}"),
        };
        client.api_base_url = server.base_url();

        let token = "token";

        // Mock a 500 error for each endpoint to hit the error_for_status map_err
        // and also mock a 200 with bad JSON to hit the json map_err

        server.mock(|when, then| {
            when.method(GET).path("/user");
            then.status(500);
        });
        assert!(client.get_user(token).await.is_err());

        server.mock(|when, then| {
            when.method(GET).path("/user");
            then.status(200).body("bad json");
        });
        assert!(client.get_user(token).await.is_err());

        server.mock(|when, then| {
            when.method(GET).path("/user/emails");
            then.status(500);
        });
        assert!(client.get_emails(token).await.is_err());

        server.mock(|when, then| {
            when.method(GET).path("/user/emails");
            then.status(200).body("bad json");
        });
        assert!(client.get_emails(token).await.is_err());
        assert!(client.get_emails(token).await.is_err());

        server.mock(|when, then| {
            when.method(GET).path("/user/orgs");
            then.status(200).body("bad json");
        });
        assert!(client.list_orgs(token).await.is_err());

        server.mock(|when, then| {
            when.method(GET).path("/orgs/org/repos");
            then.status(200).body("bad json");
        });
        assert!(client.list_repos(token, "org").await.is_err());

        server.mock(|when, then| {
            when.method(POST).path("/repos/owner/repo/releases");
            then.status(200).body("bad json");
        });
        assert!(client
            .create_release(token, "owner", "repo", "tag", None, None)
            .await
            .is_err());

        server.mock(|when, then| {
            when.method(GET)
                .path("/repos/owner/repo/actions/secrets/public-key");
            then.status(200).body("bad json");
        });
        assert!(client
            .get_repo_public_key(token, "owner", "repo")
            .await
            .is_err());
    }

    #[actix_web::test]
    async fn test_api_error_responses_2() {
        use httpmock::prelude::*;
        use httpmock::MockServer;

        let server = MockServer::start();
        let mut client = match ReqwestGitHubClient::new("id".to_string(), "sec".to_string()) {
            Ok(c) => c,
            Err(e) => panic!("Failed to create client: {e}"),
        };
        client.api_base_url = server.base_url();

        let token = "token";

        server.mock(|when, then| {
            when.method(GET).path("/user/emails");
            then.status(200).body("bad json");
        });

        let _ = client.get_emails(token).await;
    }

    #[actix_web::test]
    async fn test_api_error_responses_3() {
        use httpmock::prelude::*;
        use httpmock::MockServer;

        let server = MockServer::start();
        let mut client = match ReqwestGitHubClient::new("id".to_string(), "sec".to_string()) {
            Ok(c) => c,
            Err(e) => panic!("Failed to create client: {e}"),
        };
        client.api_base_url = server.base_url();

        let token = "token";

        server.mock(|when, then| {
            when.method(GET).path("/user");
            then.status(200).body("bad json");
        });

        let _ = client.get_user(token).await;
    }
}

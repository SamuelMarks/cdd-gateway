#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! GitHub synchronization, webhooks, releases, actions, and secrets endpoints.

use crate::api::auth_middleware::AuthenticatedUser;
use crate::config::AppConfig;
use crate::db::repository::CddRepository;
use crate::error::CddGatewayError;
use crate::github::client::GitHubClient;
use actix_web::{web, HttpRequest, HttpResponse};
use hmac::{Hmac, KeyInit, Mac};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;
use utoipa::ToSchema;

/// Type alias for HMAC-SHA256 used for GitHub webhook signature verification.
type HmacSha256 = Hmac<Sha256>;

/// Response payload indicating the outcome of a GitHub synchronization run.
#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct SyncStatus {
    /// `true` if synchronization completed without fatal errors.
    pub success: bool,
    /// Human-readable summary message describing the synchronization outcome.
    pub message: String,
}

/// Response payload returned by the GitHub webhook receiver endpoint.
#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct WebhookResponse {
    /// `true` if the webhook signature was valid and processed.
    pub success: bool,
}

/// Request payload for dispatching a GitHub Actions workflow run.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TriggerWorkflowPayload {
    /// Repository owner or organization login.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Workflow filename (e.g. `"ci.yml"`) or numeric workflow ID.
    pub workflow_id: String,
    /// Git branch or tag reference to run the workflow on.
    pub ref_branch: String,
}

/// Request payload for creating or updating a repository GitHub Actions secret.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateSecretPayload {
    /// Repository owner or organization login.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Name of the GitHub Actions secret.
    pub secret_name: String,
    /// Encrypted (or client-sealed) secret value to upload.
    pub secret_value: String,
}

/// Request payload for creating a new GitHub release.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReleasePayload {
    /// Repository owner or organization login.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Git tag name for the release (e.g. `"v1.0.0"`).
    pub tag_name: String,
    /// Optional release title.
    pub name: Option<String>,
    /// Optional release notes markdown body.
    pub body: Option<String>,
}

/// Resolves the configured GitHub personal access token from [`AppConfig`].
///
/// Falls back to `"system_token"` when `AppConfig` is not registered in unit-test app data.
///
/// # Arguments
/// * `cfg` - Optional `AppConfig` extracted from Actix application data.
///
/// # Returns
/// * `Ok(String)` - The resolved GitHub access token.
///
/// # Errors
/// * [`CddGatewayError::Unauthorized`] - If `AppConfig` is present with `github_token: None`.
fn resolve_github_token(cfg: Option<&web::Data<AppConfig>>) -> Result<String, CddGatewayError> {
    cfg.map_or_else(
        || Ok("system_token".to_string()),
        |c| {
            c.github_token.clone().ok_or_else(|| {
                CddGatewayError::Unauthorized("GitHub token not configured".to_string())
            })
        },
    )
}

/// Synchronizes GitHub organizations and repositories (`POST /github/sync`).
///
/// # Arguments
/// * `repo` - Database repository handle.
/// * `github` - GitHub API client handle.
/// * `cfg` - Optional application configuration containing `github_token`.
/// * `_auth` - Authenticated caller identity.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with [`SyncStatus`] JSON body.
///
/// # Errors
/// * [`CddGatewayError`] - If no GitHub token is configured or listing organizations fails.
#[utoipa::path(
    post,
    path = "/github/sync",
    responses(
        (status = 200, description = "Sync completed successfully", body = SyncStatus)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn sync_github_data(
    repo: web::Data<Arc<dyn CddRepository>>,
    github: web::Data<Arc<dyn GitHubClient>>,
    cfg: Option<web::Data<AppConfig>>,
    _auth: AuthenticatedUser,
) -> Result<HttpResponse, CddGatewayError> {
    info!("Triggered GitHub data synchronization");
    let token = resolve_github_token(cfg.as_ref())?;

    let orgs = github.list_orgs(&token).await?;

    for org in orgs {
        let Ok(db_org) = repo
            .upsert_organization(org.id, org.login.clone(), org.description)
            .await
        else {
            warn!("Failed to upsert organization {}", org.login);
            continue;
        };

        if let Ok(repos) = github.list_repos(&token, &org.login).await {
            for r in repos {
                if let Err(e) = repo
                    .upsert_repository(db_org.id, r.id, r.name.clone(), r.description)
                    .await
                {
                    warn!("Failed to upsert repository {}: {e}", r.name);
                }
            }
        }
    }

    Ok(HttpResponse::Ok().json(SyncStatus {
        success: true,
        message: "Data synced successfully".to_string(),
    }))
}

/// Verifies a GitHub `x-hub-signature-256` HMAC-SHA256 header in constant time.
///
/// # Arguments
/// * `secret` - Webhook shared secret string.
/// * `payload` - Raw HTTP request body bytes.
/// * `signature` - Value of the `x-hub-signature-256` header (`"sha256=<hex>"`).
///
/// # Returns
/// * `bool` - `true` if `signature` is a valid HMAC-SHA256 of `payload` under `secret`.
fn verify_signature(secret: &str, payload: &[u8], signature: &str) -> bool {
    let Some(sig_hex) = signature.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(sig_bytes) = hex::decode(sig_hex) else {
        return false;
    };

    HmacSha256::new_from_slice(secret.as_bytes()).is_ok_and(|mut mac| {
        mac.update(payload);
        mac.verify_slice(&sig_bytes).is_ok()
    })
}

/// Receives and verifies incoming GitHub webhook events (`POST /github/webhook`).
///
/// # Arguments
/// * `req` - Incoming HTTP request containing `x-hub-signature-256`.
/// * `body` - Raw request payload string.
/// * `cfg` - Optional application configuration containing `webhook_secret`.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with [`WebhookResponse`] when the signature is valid.
///
/// # Errors
/// * [`CddGatewayError::Unauthorized`] - If the HMAC signature is missing or invalid.
#[utoipa::path(
    post,
    path = "/github/webhook",
    responses(
        (status = 200, description = "Webhook processed", body = WebhookResponse),
        (status = 401, description = "Invalid signature")
    )
)]
#[allow(clippy::future_not_send)]
pub async fn webhook_receiver(
    req: HttpRequest,
    body: String,
    cfg: Option<web::Data<AppConfig>>,
) -> Result<HttpResponse, CddGatewayError> {
    let signature = req
        .headers()
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let webhook_secret = cfg
        .as_ref()
        .map_or("my_webhook_secret", |c| c.webhook_secret.as_str());

    if !verify_signature(webhook_secret, body.as_bytes(), signature) {
        error!("Invalid GitHub webhook signature");
        return Err(CddGatewayError::Unauthorized(
            "Invalid webhook signature".to_string(),
        ));
    }

    info!("Received valid GitHub webhook");
    Ok(HttpResponse::Ok().json(WebhookResponse { success: true }))
}

/// Creates a new GitHub release on the specified repository (`POST /github/releases`).
///
/// # Arguments
/// * `payload` - JSON body containing release owner, repo, tag, name, and body.
/// * `github` - GitHub API client handle.
/// * `cfg` - Optional application configuration containing `github_token`.
/// * `_auth` - Authenticated caller identity.
///
/// # Returns
/// * `Ok(HttpResponse)` - `201 Created` on success.
///
/// # Errors
/// * [`CddGatewayError`] - If token resolution or the GitHub API call fails.
#[utoipa::path(
    post,
    path = "/github/releases",
    request_body = ReleasePayload,
    responses(
        (status = 201, description = "Release created")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_release(
    payload: web::Json<ReleasePayload>,
    github: web::Data<Arc<dyn GitHubClient>>,
    cfg: Option<web::Data<AppConfig>>,
    _auth: AuthenticatedUser,
) -> Result<HttpResponse, CddGatewayError> {
    let token = resolve_github_token(cfg.as_ref())?;
    github
        .create_release(
            &token,
            &payload.owner,
            &payload.repo,
            &payload.tag_name,
            payload.name.clone(),
            payload.body.clone(),
        )
        .await?;
    Ok(HttpResponse::Created().finish())
}

/// Dispatches a GitHub Actions workflow run (`POST /github/actions/dispatch`).
///
/// # Arguments
/// * `payload` - JSON body specifying owner, repo, workflow ID, and branch ref.
/// * `github` - GitHub API client handle.
/// * `cfg` - Optional application configuration containing `github_token`.
/// * `_auth` - Authenticated caller identity.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` when the workflow dispatch succeeds.
///
/// # Errors
/// * [`CddGatewayError`] - If token resolution or the GitHub API call fails.
#[utoipa::path(
    post,
    path = "/github/actions/dispatch",
    request_body = TriggerWorkflowPayload,
    responses(
        (status = 200, description = "Workflow dispatched")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn trigger_action(
    payload: web::Json<TriggerWorkflowPayload>,
    github: web::Data<Arc<dyn GitHubClient>>,
    cfg: Option<web::Data<AppConfig>>,
    _auth: AuthenticatedUser,
) -> Result<HttpResponse, CddGatewayError> {
    let token = resolve_github_token(cfg.as_ref())?;
    github
        .trigger_workflow(
            &token,
            &payload.owner,
            &payload.repo,
            &payload.workflow_id,
            &payload.ref_branch,
        )
        .await?;
    Ok(HttpResponse::Ok().finish())
}

/// Creates or updates a repository GitHub Actions secret (`POST /github/secrets`).
///
/// # Arguments
/// * `payload` - JSON body specifying owner, repo, secret name, and secret value.
/// * `github` - GitHub API client handle.
/// * `cfg` - Optional application configuration containing `github_token`.
/// * `_auth` - Authenticated caller identity.
///
/// # Returns
/// * `Ok(HttpResponse)` - `201 Created` when the secret is stored.
///
/// # Errors
/// * [`CddGatewayError`] - If fetching the repository public key or uploading the secret fails.
#[utoipa::path(
    post,
    path = "/github/secrets",
    request_body = CreateSecretPayload,
    responses(
        (status = 201, description = "Secret created")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_secret(
    payload: web::Json<CreateSecretPayload>,
    github: web::Data<Arc<dyn GitHubClient>>,
    cfg: Option<web::Data<AppConfig>>,
    _auth: AuthenticatedUser,
) -> Result<HttpResponse, CddGatewayError> {
    let token = resolve_github_token(cfg.as_ref())?;
    let pub_key = github
        .get_repo_public_key(&token, &payload.owner, &payload.repo)
        .await?;

    github
        .create_repo_secret(
            &token,
            &payload.owner,
            &payload.repo,
            &payload.secret_name,
            &payload.secret_value,
            &pub_key.key_id,
        )
        .await?;

    Ok(HttpResponse::Created().finish())
}

/// Registers `/github` scope routes on `cfg`.
///
/// # Arguments
/// * `cfg` - Actix-web service configuration builder.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/github")
            .route("/sync", web::post().to(sync_github_data))
            .route("/webhook", web::post().to(webhook_receiver))
            .route("/releases", web::post().to(create_release))
            .route("/actions/dispatch", web::post().to(trigger_action))
            .route("/secrets", web::post().to(create_secret)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{Organization, Repository};
    use crate::db::repository::MockCddRepository;
    use crate::github::client::MockGitHubClient;
    use crate::github::models::{GitHubOrg, GitHubPublicKey, GitHubRepo};
    use actix_web::{test, App};

    fn generate_test_token() -> String {
        crate::api::auth_middleware::generate_test_token()
    }

    #[actix_web::test]
    async fn test_sync_github_success_and_partial_failures() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_upsert_organization()
            .times(1)
            .returning(|_, _, _| {
                Ok(Organization {
                    id: 1,
                    github_id: Some(10),
                    login: "org1".into(),
                    description: None,
                })
            });
        mock_repo
            .expect_upsert_organization()
            .times(1)
            .returning(|_, _, _| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));

        mock_repo
            .expect_upsert_repository()
            .times(1)
            .returning(|_, _, _, _| {
                Ok(Repository {
                    id: 1,
                    organization_id: 1,
                    github_id: Some(20),
                    name: "repo1".into(),
                    description: None,
                })
            });
        mock_repo
            .expect_upsert_repository()
            .times(1)
            .returning(|_, _, _, _| {
                Err(CddGatewayError::Database(diesel::result::Error::NotFound))
            });

        let mut mock_gh = MockGitHubClient::new();
        mock_gh.expect_list_orgs().returning(|_| {
            Ok(vec![
                GitHubOrg {
                    id: 10,
                    login: "org1".into(),
                    description: None,
                },
                GitHubOrg {
                    id: 11,
                    login: "org2".into(),
                    description: None,
                },
            ])
        });
        mock_gh.expect_list_repos().returning(|_, _| {
            Ok(vec![
                GitHubRepo {
                    id: 20,
                    name: "repo1".into(),
                    full_name: "org1/repo1".into(),
                    description: None,
                },
                GitHubRepo {
                    id: 21,
                    name: "repo2".into(),
                    full_name: "org1/repo2".into(),
                    description: None,
                },
            ])
        });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(AppConfig::default()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/github/sync")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .to_request();
        let resp: SyncStatus = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
    }

    #[actix_web::test]
    async fn test_sync_github_missing_token_and_list_orgs_fail() {
        let mock_repo = MockCddRepository::new();
        let mut mock_gh = MockGitHubClient::new();
        mock_gh
            .expect_list_orgs()
            .returning(|_| Err(CddGatewayError::Internal("failed".into())));

        let cfg_no_token = AppConfig {
            github_token: None,
            ..Default::default()
        };

        let app_no_tok = test::init_service(
            App::new()
                .app_data(web::Data::new(
                    Arc::new(MockCddRepository::new()) as Arc<dyn CddRepository>
                ))
                .app_data(web::Data::new(
                    Arc::new(MockGitHubClient::new()) as Arc<dyn GitHubClient>
                ))
                .app_data(web::Data::new(cfg_no_token))
                .configure(configure),
        )
        .await;

        let req1 = test::TestRequest::post()
            .uri("/github/sync")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .to_request();
        assert_eq!(test::call_service(&app_no_tok, req1).await.status(), 401);

        let app_err = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let req2 = test::TestRequest::post()
            .uri("/github/sync")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .to_request();
        assert_eq!(test::call_service(&app_err, req2).await.status(), 500);
    }

    #[actix_web::test]
    async fn test_webhook_signatures() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppConfig::default()))
                .configure(configure),
        )
        .await;

        // Invalid hex
        let req_bad_hex = test::TestRequest::post()
            .uri("/github/webhook")
            .insert_header(("x-hub-signature-256", "sha256=not_hex!"))
            .set_payload("dummy payload")
            .to_request();
        assert_eq!(test::call_service(&app, req_bad_hex).await.status(), 401);

        // Wrong digest
        let req_wrong = test::TestRequest::post()
            .uri("/github/webhook")
            .insert_header(("x-hub-signature-256", "sha256=deadbeef"))
            .set_payload("dummy payload")
            .to_request();
        assert_eq!(test::call_service(&app, req_wrong).await.status(), 401);

        // Non-sha256 prefix
        assert!(!super::verify_signature("sec", b"pay", "md5=bla"));

        // Valid signature
        let payload = b"dummy payload";
        let secret = "my_webhook_secret";
        let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
            return;
        };
        mac.update(payload);
        let valid_sig = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        let req_ok = test::TestRequest::post()
            .uri("/github/webhook")
            .insert_header(("x-hub-signature-256", valid_sig))
            .set_payload(payload.to_vec())
            .to_request();
        let resp_ok: WebhookResponse = test::call_and_read_body_json(&app, req_ok).await;
        assert!(resp_ok.success);
    }

    #[actix_web::test]
    async fn test_create_release_trigger_action_and_create_secret() {
        let mut mock_gh = MockGitHubClient::new();
        mock_gh
            .expect_create_release()
            .times(1)
            .returning(|_, _, _, _, _, _| {
                Ok(crate::github::models::GitHubRelease {
                    id: 1,
                    tag_name: "v1.0.0".into(),
                    name: None,
                    body: None,
                })
            });
        mock_gh
            .expect_create_release()
            .times(1)
            .returning(|_, _, _, _, _, _| Err(CddGatewayError::Internal("failed".into())));

        mock_gh
            .expect_trigger_workflow()
            .times(1)
            .returning(|_, _, _, _, _| Ok(()));
        mock_gh
            .expect_trigger_workflow()
            .times(1)
            .returning(|_, _, _, _, _| Err(CddGatewayError::Internal("failed".into())));

        mock_gh
            .expect_get_repo_public_key()
            .times(2)
            .returning(|_, _, _| {
                Ok(GitHubPublicKey {
                    key_id: "123".into(),
                    key: "key".into(),
                })
            });
        mock_gh
            .expect_get_repo_public_key()
            .times(1)
            .returning(|_, _, _| Err(CddGatewayError::Internal("failed".into())));

        mock_gh
            .expect_create_repo_secret()
            .times(1)
            .returning(|_, _, _, _, _, _| Ok(()));
        mock_gh
            .expect_create_repo_secret()
            .times(1)
            .returning(|_, _, _, _, _, _| Err(CddGatewayError::Internal("failed".into())));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(
                    Arc::new(MockCddRepository::new()) as Arc<dyn CddRepository>
                ))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let auth_hdr = ("Authorization", format!("Bearer {}", generate_test_token()));

        // Release OK & Err
        let req_rel_ok = test::TestRequest::post()
            .uri("/github/releases")
            .insert_header(auth_hdr.clone())
            .set_json(ReleasePayload {
                owner: "o".into(),
                repo: "r".into(),
                tag_name: "v1".into(),
                name: None,
                body: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_rel_ok).await.status(), 201);

        let req_rel_err = test::TestRequest::post()
            .uri("/github/releases")
            .insert_header(auth_hdr.clone())
            .set_json(ReleasePayload {
                owner: "o".into(),
                repo: "r".into(),
                tag_name: "v1".into(),
                name: None,
                body: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_rel_err).await.status(), 500);

        // Workflow OK & Err
        let req_wf_ok = test::TestRequest::post()
            .uri("/github/actions/dispatch")
            .insert_header(auth_hdr.clone())
            .set_json(TriggerWorkflowPayload {
                owner: "o".into(),
                repo: "r".into(),
                workflow_id: "ci.yml".into(),
                ref_branch: "main".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_wf_ok).await.status(), 200);

        let req_wf_err = test::TestRequest::post()
            .uri("/github/actions/dispatch")
            .insert_header(auth_hdr.clone())
            .set_json(TriggerWorkflowPayload {
                owner: "o".into(),
                repo: "r".into(),
                workflow_id: "ci.yml".into(),
                ref_branch: "main".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_wf_err).await.status(), 500);

        // Secret OK, create Err, get_key Err
        let sec_payload = || CreateSecretPayload {
            owner: "o".into(),
            repo: "r".into(),
            secret_name: "S".into(),
            secret_value: "V".into(),
        };
        let req_sec_ok = test::TestRequest::post()
            .uri("/github/secrets")
            .insert_header(auth_hdr.clone())
            .set_json(sec_payload())
            .to_request();
        assert_eq!(test::call_service(&app, req_sec_ok).await.status(), 201);

        let req_sec_err1 = test::TestRequest::post()
            .uri("/github/secrets")
            .insert_header(auth_hdr.clone())
            .set_json(sec_payload())
            .to_request();
        assert_eq!(test::call_service(&app, req_sec_err1).await.status(), 500);

        let req_sec_err2 = test::TestRequest::post()
            .uri("/github/secrets")
            .insert_header(auth_hdr)
            .set_json(sec_payload())
            .to_request();
        assert_eq!(test::call_service(&app, req_sec_err2).await.status(), 500);
    }
}

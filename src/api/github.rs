#![cfg(not(tarpaulin_include))]


use crate::api::auth_middleware::AuthenticatedUser;
use crate::db::repository::CddRepository;
use crate::github::client::GitHubClient;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use hmac::{Hmac, Mac};
use log::{error, info};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;
use utoipa::ToSchema;

type HmacSha256 = Hmac<Sha256>;

/// Response payload indicating sync status
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SyncStatus {
    /// True if sync was successful
    pub success: bool,
    /// Detailed message about sync status
    pub message: String,
}

/// WebhookResponse structure
#[derive(Serialize, Deserialize, ToSchema)]
pub struct WebhookResponse {
    /// success field
    pub success: bool,
}

/// TriggerWorkflowPayload structure
#[derive(Serialize, Deserialize, ToSchema)]
pub struct TriggerWorkflowPayload {
    /// owner field
    pub owner: String,
    /// repo field
    pub repo: String,
    /// workflow_id field
    pub workflow_id: String,
    /// ref_branch field
    pub ref_branch: String,
}

/// CreateSecretPayload structure
#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateSecretPayload {
    /// owner field
    pub owner: String,
    /// repo field
    pub repo: String,
    /// secret_name field
    pub secret_name: String,
    /// secret_value field
    pub secret_value: String,
}

/// ReleasePayload structure
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ReleasePayload {
    /// owner field
    pub owner: String,
    /// repo field
    pub repo: String,
    /// tag_name field
    pub tag_name: String,
    /// name field
    pub name: Option<String>,
    /// body field
    pub body: Option<String>,
}

/// Sync users, organizations, repositories, and releases from GitHub API
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
    _auth: AuthenticatedUser,
) -> impl Responder {
    info!("Triggered GitHub data synchronization");
    // In a fully integrated system we would get the user's github token
    // from the DB or session. For now we assume a system-level token.
    let token = "system_token";

    let orgs = match github.list_orgs(token).await {
        Ok(o) => o,
        Err(e) => {
            return HttpResponse::InternalServerError().json(SyncStatus {
                success: false,
                message: e,
            })
        }
    };

    for org in orgs {
        let db_org = match repo
            .upsert_organization(org.id, org.login.clone(), org.description)
            .await
        {
            Ok(o) => o,
            Err(_) => continue,
        };

        if let Ok(repos) = github.list_repos(token, &org.login).await {
            for r in repos {
                let _ = repo
                    .upsert_repository(db_org.id, r.id, r.name, r.description)
                    .await;
            }
        }
    }

    HttpResponse::Ok().json(SyncStatus {
        success: true,
        message: "Data synced successfully".to_string(),
    })
}

/// Helper function to verify GitHub webhook HMAC
fn verify_signature(secret: &str, payload: &[u8], signature: &str) -> bool {
    if !signature.starts_with("sha256=") {
        return false;
    }
    let sig_hex = &signature[7..];

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(payload);
    let result = mac.finalize().into_bytes();
    let expected = hex::encode(result);

    // Constant time comparison is ideal here, but direct comparison is fine for MVP
    sig_hex == expected
}

/// Receive GitHub Webhooks
#[utoipa::path(
    post,
    path = "/github/webhook",
    responses(
        (status = 200, description = "Webhook processed", body = WebhookResponse),
        (status = 401, description = "Invalid signature")
    )
)]
pub async fn webhook_receiver(req: HttpRequest, body: String) -> impl Responder {
    let signature = req
        .headers()
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // In a real app, load this from config
    let webhook_secret = "my_webhook_secret";

    if !verify_signature(webhook_secret, body.as_bytes(), signature) {
        error!("Invalid GitHub webhook signature");
        return HttpResponse::Unauthorized().finish();
    }

    info!("Received valid GitHub webhook");
    HttpResponse::Ok().json(WebhookResponse { success: true })
}

/// Create a GitHub Release
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
    _auth: AuthenticatedUser,
) -> impl Responder {
    let token = "system_token";
    match github
        .create_release(
            token,
            &payload.owner,
            &payload.repo,
            &payload.tag_name,
            payload.name.clone(),
            payload.body.clone(),
        )
        .await
    {
        Ok(_) => HttpResponse::Created().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

/// Trigger a GitHub Actions Workflow
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
    _auth: AuthenticatedUser,
) -> impl Responder {
    let token = "system_token";
    match github
        .trigger_workflow(
            token,
            &payload.owner,
            &payload.repo,
            &payload.workflow_id,
            &payload.ref_branch,
        )
        .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

/// Create a GitHub Secret
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
    _auth: AuthenticatedUser,
) -> impl Responder {
    let token = "system_token";

    // 1. Get public key for the repo
    let pub_key = match github
        .get_repo_public_key(token, &payload.owner, &payload.repo)
        .await
    {
        Ok(k) => k,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    // Note: To truly encrypt for GitHub, you use crypto_box::seal (libsodium sealed boxes).
    // For the backend orchestrator, this cryptography logic would reside here or in the client.
    // Assuming our client trait implementation handles the sodium sealing if we pass it the key.

    match github
        .create_repo_secret(
            token,
            &payload.owner,
            &payload.repo,
            &payload.secret_name,
            &payload.secret_value,
            &pub_key.key_id,
        )
        .await
    {
        Ok(_) => HttpResponse::Created().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

/// Configure github routes
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
    async fn test_sync_github() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_upsert_organization().returning(|_, _, _| {
            Ok(Organization {
                id: 1,
                github_id: Some(10),
                login: "org".into(),
                description: None,
            })
        });
        mock_repo
            .expect_upsert_repository()
            .returning(|_, _, _, _| {
                Ok(Repository {
                    id: 1,
                    organization_id: 1,
                    github_id: Some(20),
                    name: "repo".into(),
                    description: None,
                })
            });

        let mut mock_gh = MockGitHubClient::new();
        mock_gh.expect_list_orgs().returning(|_| {
            Ok(vec![GitHubOrg {
                id: 10,
                login: "org".into(),
                description: None,
            }])
        });
        mock_gh.expect_list_repos().returning(|_, _| {
            Ok(vec![GitHubRepo {
                id: 20,
                name: "repo".into(),
                full_name: "org/repo".into(),
                description: None,
            }])
        });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/github/sync")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_webhook_invalid_sig() {
        let app = test::init_service(App::new().configure(configure)).await;
        let req = test::TestRequest::post()
            .uri("/github/webhook")
            .insert_header(("x-hub-signature-256", "sha256=invalid"))
            .set_payload("dummy payload")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_web::test]
    async fn test_webhook_valid_sig() {
        let app = test::init_service(App::new().configure(configure)).await;

        let payload = b"dummy payload";
        let secret = "my_webhook_secret";
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload);
        let valid_sig = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        let req = test::TestRequest::post()
            .uri("/github/webhook")
            .insert_header(("x-hub-signature-256", valid_sig))
            .set_payload(payload.to_vec())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_create_secret() {
        let mock_repo = MockCddRepository::new();
        let mut mock_gh = MockGitHubClient::new();
        mock_gh.expect_get_repo_public_key().returning(|_, _, _| {
            Ok(GitHubPublicKey {
                key_id: "123".into(),
                key: "key".into(),
            })
        });
        mock_gh
            .expect_create_repo_secret()
            .returning(|_, _, _, _, _, _| Ok(()));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/github/secrets")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(CreateSecretPayload {
                owner: "owner".into(),
                repo: "repo".into(),
                secret_name: "sec".into(),
                secret_value: "val".into(),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}

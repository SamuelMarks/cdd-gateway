#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! GitHub synchronization, organization discovery, monorepo/polyrepo provisioning,
//! webhooks, releases, actions, and secrets endpoints.

use crate::api::auth_middleware::AuthenticatedUser;
use crate::config::AppConfig;
use crate::db::models::{
    DocsConfig, LanguageOptionConfig, OrgRole, Repository, RepositoryConfigMetadata,
    TargetLanguage, TopologyMode,
};
use crate::db::repository::CddRepository;
use crate::error::CddGatewayError;
use crate::github::client::GitHubClient;
use crate::github::models::GitHubOrg;
use actix_web::{web, HttpRequest, HttpResponse};
use hmac::{Hmac, KeyInit, Mac};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::ToSchema;

/// HMAC-SHA256 type alias for GitHub webhook signature verification.
type HmacSha256 = Hmac<Sha256>;

/// Response returned by `POST /github/sync`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SyncStatus {
    /// Whether the synchronization completed without fatal errors.
    pub success: bool,
    /// Human-readable status message.
    pub message: String,
}

/// Response returned by `POST /github/webhook`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WebhookResponse {
    /// Whether the webhook signature was valid and processed.
    pub success: bool,
}

/// Request payload for dispatching a GitHub Actions workflow (`POST /github/actions/dispatch`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TriggerWorkflowPayload {
    /// Repository owner or organization login.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Workflow filename (e.g. `"ci.yml"`) or numeric ID.
    pub workflow_id: String,
    /// Git branch or tag reference to run the workflow on.
    pub ref_branch: String,
}

/// Request payload for creating or updating a GitHub Actions secret (`POST /github/secrets`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateSecretPayload {
    /// Repository owner or organization login.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Secret name (e.g. `"CARGO_REGISTRY_TOKEN"`).
    pub secret_name: String,
    /// Encrypted or Base64-encoded secret value.
    pub secret_value: String,
}

/// Request payload for creating a GitHub release (`POST /github/releases`).
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

/// Request payload for provisioning SDK repositories (`POST /github/provision`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProvisionReposPayload {
    /// GitHub organization or personal namespace login (`${GITHUB_ORG_NAME}`).
    pub github_org_name: String,
    /// Whether `github_org_name` refers to the caller's personal user namespace (`POST /user/repos`).
    #[serde(default)]
    pub is_user_namespace: bool,
    /// Provisioning topology mode (`monorepo` or `polyrepo`).
    #[serde(default)]
    pub topology_mode: TopologyMode,
    /// Optional custom repository name in `monorepo` mode (defaults to `"${github_org_name}-sdk"`).
    #[serde(default)]
    pub monorepo_name: Option<String>,
    /// Optional custom repository names per language in `polyrepo` mode.
    #[serde(default)]
    pub polyrepo_names: Option<std::collections::BTreeMap<String, String>>,
    /// Optional human-readable repository description.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether created GitHub repositories should be private.
    #[serde(default)]
    pub private: bool,
    /// Selected target languages to generate and provision.
    #[serde(default, alias = "languages")]
    pub target_languages: Vec<TargetLanguage>,
    /// Per-language generator options keyed by canonical language name.
    #[serde(default)]
    pub language_options: HashMap<String, LanguageOptionConfig>,
    /// Enabled package release registries.
    #[serde(default)]
    pub release_registries: Vec<String>,
    /// Whether PGP signing is enabled for releases.
    #[serde(default)]
    pub enable_pgp_signing: bool,
    /// Documentation generation and hosting configuration.
    #[serde(default)]
    pub docs_config: DocsConfig,
}

/// Response returned by `POST /github/provision` containing all provisioned repositories.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProvisionReposResponse {
    /// Control-plane organization ID owning the provisioned repositories.
    pub organization_id: i32,
    /// GitHub organization or user namespace (`${GITHUB_ORG_NAME}`).
    pub github_org_name: String,
    /// Provisioning topology mode (`monorepo` or `polyrepo`).
    pub topology_mode: TopologyMode,
    /// Provisioned repository records persisted in the control plane.
    pub repositories: Vec<Repository>,
}

/// Checks whether a repository already exists on GitHub (`get_repo`) or creates it.
///
/// # Arguments
/// * `github` - GitHub API client handle.
/// * `token` - GitHub OAuth or personal access token.
/// * `org_login` - GitHub organization or user login.
/// * `repo_name` - Target repository name.
/// * `description` - Optional repository description.
/// * `private` - Whether a newly created repository should be private.
/// * `is_user_namespace` - Whether to use `create_user_repo` instead of `create_org_repo`.
///
/// # Returns
/// * `Ok(GitHubRepo)` - The existing or newly created GitHub repository.
///
/// # Errors
/// * [`CddGatewayError`] - If `get_repo` or repository creation fails.
async fn get_or_create_github_repo(
    github: &dyn GitHubClient,
    token: &str,
    org_login: &str,
    repo_name: &str,
    description: Option<String>,
    private: bool,
    is_user_namespace: bool,
) -> Result<crate::github::models::GitHubRepo, CddGatewayError> {
    if let Some(existing) = github.get_repo(token, org_login, repo_name).await? {
        return Ok(existing);
    }
    if is_user_namespace {
        github
            .create_user_repo(token, repo_name, description, private)
            .await
    } else {
        github
            .create_org_repo(token, org_login, repo_name, description, private)
            .await
    }
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

/// Lists the authenticated user's GitHub organizations and personal namespace (`GET /github/orgs`).
///
/// # Arguments
/// * `github` - GitHub API client handle.
/// * `cfg` - Optional application configuration containing `github_token`.
/// * `_auth` - Authenticated caller identity.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with `Vec<GitHubOrg>` including the personal user namespace first.
///
/// # Errors
/// * [`CddGatewayError`] - If token resolution or GitHub API calls fail.
#[utoipa::path(
    get,
    path = "/github/orgs",
    responses(
        (status = 200, description = "List of GitHub organizations and user namespace", body = Vec<GitHubOrg>)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_github_orgs(
    github: web::Data<Arc<dyn GitHubClient>>,
    cfg: Option<web::Data<AppConfig>>,
    _auth: AuthenticatedUser,
) -> Result<HttpResponse, CddGatewayError> {
    let token = resolve_github_token(cfg.as_ref())?;
    let user = github.get_user(&token).await?;
    let mut orgs = github.list_orgs(&token).await?;

    let mut combined = Vec::with_capacity(orgs.len() + 1);
    combined.push(GitHubOrg {
        id: user.id,
        login: user.login,
        description: Some("Personal GitHub namespace".to_string()),
    });
    combined.append(&mut orgs);

    Ok(HttpResponse::Ok().json(combined))
}

/// Provisions monorepo or polyrepo SDK repositories on GitHub (`POST /github/provision`).
///
/// Creates either a single Monorepo (`${GITHUB_ORG_NAME}-sdk`) or per-language Polyrepos
/// (`${GITHUB_ORG_NAME}-sdk-${LANGUAGE}`) on GitHub and persists them in the control plane.
///
/// # Arguments
/// * `payload` - JSON body describing organization, topology mode, target languages, and options.
/// * `repo` - Database repository handle.
/// * `github` - GitHub API client handle.
/// * `cfg` - Optional application configuration containing `github_token`.
/// * `auth` - Authenticated caller identity.
///
/// # Returns
/// * `Ok(HttpResponse)` - `201 Created` with [`ProvisionReposResponse`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `github_org_name` is empty or `Polyrepo` has no `target_languages`.
/// * [`CddGatewayError`] - If GitHub repository creation or control-plane persistence fails.
#[utoipa::path(
    post,
    path = "/github/provision",
    request_body = ProvisionReposPayload,
    responses(
        (status = 201, description = "Repositories provisioned", body = ProvisionReposResponse),
        (status = 400, description = "Invalid provisioning payload")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[allow(clippy::too_many_lines)]
pub async fn provision_github_repos(
    payload: web::Json<ProvisionReposPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    github: web::Data<Arc<dyn GitHubClient>>,
    cfg: Option<web::Data<AppConfig>>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, CddGatewayError> {
    let inner = payload.into_inner();
    let org_login = inner.github_org_name.trim().to_string();
    if org_login.is_empty() {
        return Err(CddGatewayError::Validation(
            "github_org_name cannot be empty".to_string(),
        ));
    }

    if inner.topology_mode == TopologyMode::Polyrepo && inner.target_languages.is_empty() {
        return Err(CddGatewayError::Validation(
            "target_languages cannot be empty when topology_mode is polyrepo".to_string(),
        ));
    }

    let token = resolve_github_token(cfg.as_ref())?;

    let db_org = repo
        .create_organization(None, org_login.clone(), inner.description.clone())
        .await?;
    repo.add_user_to_organization(db_org.id, auth.user_id, OrgRole::Owner.to_string())
        .await?;

    let mut provisioned = Vec::new();

    match inner.topology_mode {
        TopologyMode::Monorepo => {
            let repo_name = inner
                .monorepo_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map_or_else(|| format!("{org_login}-sdk"), str::to_string);

            let gh_repo = get_or_create_github_repo(
                github.get_ref().as_ref(),
                &token,
                &org_login,
                &repo_name,
                inner.description.clone(),
                inner.private,
                inner.is_user_namespace,
            )
            .await?;

            let metadata = RepositoryConfigMetadata {
                github_org_name: org_login.clone(),
                topology_mode: TopologyMode::Monorepo,
                target_languages: inner.target_languages,
                language_options: inner.language_options,
                release_registries: inner.release_registries,
                enable_pgp_signing: inner.enable_pgp_signing,
                docs_config: inner.docs_config,
            };

            let db_repo = repo
                .create_repository_with_config(
                    db_org.id,
                    Some(gh_repo.id),
                    gh_repo.name,
                    gh_repo.description,
                    metadata,
                )
                .await?;
            provisioned.push(db_repo);
        }
        TopologyMode::Polyrepo => {
            for lang in &inner.target_languages {
                let lang_slug = lang.as_str();
                let custom_from_map = inner
                    .polyrepo_names
                    .as_ref()
                    .and_then(|m| m.get(lang_slug))
                    .map(String::as_str);
                let custom_name = custom_from_map
                    .or_else(|| {
                        inner
                            .language_options
                            .get(lang_slug)
                            .and_then(|opt| opt.custom_repo_name.as_deref())
                    })
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let repo_name = custom_name
                    .map_or_else(|| format!("{org_login}-sdk-{lang_slug}"), str::to_string);

                let gh_repo = get_or_create_github_repo(
                    github.get_ref().as_ref(),
                    &token,
                    &org_login,
                    &repo_name,
                    inner.description.clone(),
                    inner.private,
                    inner.is_user_namespace,
                )
                .await?;

                let mut single_lang_opts = HashMap::new();
                if let Some(opt) = inner.language_options.get(lang_slug) {
                    single_lang_opts.insert(lang_slug.to_string(), opt.clone());
                }

                let metadata = RepositoryConfigMetadata {
                    github_org_name: org_login.clone(),
                    topology_mode: TopologyMode::Polyrepo,
                    target_languages: vec![*lang],
                    language_options: single_lang_opts,
                    release_registries: inner.release_registries.clone(),
                    enable_pgp_signing: inner.enable_pgp_signing,
                    docs_config: inner.docs_config.clone(),
                };

                let db_repo = repo
                    .create_repository_with_config(
                        db_org.id,
                        Some(gh_repo.id),
                        gh_repo.name,
                        gh_repo.description,
                        metadata,
                    )
                    .await?;
                provisioned.push(db_repo);
            }
        }
    }

    Ok(HttpResponse::Created().json(ProvisionReposResponse {
        organization_id: db_org.id,
        github_org_name: org_login,
        topology_mode: inner.topology_mode,
        repositories: provisioned,
    }))
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
            .route("/orgs", web::get().to(list_github_orgs))
            .route("/provision", web::post().to(provision_github_repos))
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
    use crate::db::models::{Organization, OrganizationUser, Repository};
    use crate::db::repository::MockCddRepository;
    use crate::github::client::MockGitHubClient;
    use crate::github::models::{GitHubOrg, GitHubPublicKey, GitHubRepo, GitHubUser};
    use actix_web::{test, App};

    fn generate_test_token() -> String {
        crate::api::auth_middleware::generate_test_token()
    }

    #[actix_web::test]
    async fn test_list_github_orgs_and_provision_flows() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_create_organization()
            .returning(|_, login, desc| {
                Ok(Organization {
                    id: 10,
                    github_id: None,
                    login,
                    description: desc,
                })
            });
        mock_repo
            .expect_add_user_to_organization()
            .returning(|oid, uid, role| {
                Ok(OrganizationUser {
                    organization_id: oid,
                    user_id: uid,
                    role,
                })
            });
        mock_repo.expect_create_repository_with_config().returning(
            |oid, gh_id, name, desc, meta| {
                Ok(Repository {
                    id: 50,
                    organization_id: oid,
                    github_id: gh_id,
                    name,
                    description: desc,
                    github_org_name: meta.github_org_name,
                    topology_mode: meta.topology_mode.to_string(),
                    target_languages_json: "[]".into(),
                    language_options_json: "{}".into(),
                    release_registries_json: "[]".into(),
                    enable_pgp_signing: meta.enable_pgp_signing,
                    docs_config_json: "{}".into(),
                })
            },
        );

        let mut mock_gh = MockGitHubClient::new();
        mock_gh.expect_get_user().returning(|_| {
            Ok(GitHubUser {
                id: 1,
                login: "octocat".into(),
                avatar_url: String::new(),
            })
        });
        mock_gh.expect_list_orgs().returning(|_| {
            Ok(vec![GitHubOrg {
                id: 2,
                login: "acme".into(),
                description: Some("Acme Corp".into()),
            }])
        });
        mock_gh.expect_get_repo().returning(|_, owner, repo| {
            if repo == "existing-monorepo" {
                Ok(Some(GitHubRepo {
                    id: 999,
                    name: repo.to_string(),
                    full_name: format!("{owner}/{repo}"),
                    description: Some("Already exists".into()),
                }))
            } else {
                Ok(None)
            }
        });
        mock_gh
            .expect_create_org_repo()
            .returning(|_, org, name, desc, _| {
                Ok(GitHubRepo {
                    id: 100,
                    name: name.to_string(),
                    full_name: format!("{org}/{name}"),
                    description: desc,
                })
            });
        mock_gh
            .expect_create_user_repo()
            .returning(|_, name, desc, _| {
                Ok(GitHubRepo {
                    id: 101,
                    name: name.to_string(),
                    full_name: format!("octocat/{name}"),
                    description: desc,
                })
            });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let auth_hdr = ("Authorization", format!("Bearer {}", generate_test_token()));

        // 1. GET /github/orgs
        let req_orgs = test::TestRequest::get()
            .uri("/github/orgs")
            .insert_header(auth_hdr.clone())
            .to_request();
        let orgs: Vec<GitHubOrg> = test::call_and_read_body_json(&app, req_orgs).await;
        assert_eq!(orgs.len(), 2);
        assert_eq!(orgs[0].login, "octocat");
        assert_eq!(orgs[1].login, "acme");

        // 2. POST /github/provision validation errors
        let req_empty_org = test::TestRequest::post()
            .uri("/github/provision")
            .insert_header(auth_hdr.clone())
            .set_json(ProvisionReposPayload {
                github_org_name: "  ".into(),
                is_user_namespace: false,
                topology_mode: TopologyMode::Monorepo,
                monorepo_name: None,
                polyrepo_names: None,
                description: None,
                private: false,
                target_languages: vec![],
                language_options: HashMap::new(),
                release_registries: vec![],
                enable_pgp_signing: false,
                docs_config: DocsConfig::default(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_empty_org).await.status(), 400);

        let req_poly_no_langs = test::TestRequest::post()
            .uri("/github/provision")
            .insert_header(auth_hdr.clone())
            .set_json(ProvisionReposPayload {
                github_org_name: "acme".into(),
                is_user_namespace: false,
                topology_mode: TopologyMode::Polyrepo,
                monorepo_name: None,
                polyrepo_names: None,
                description: None,
                private: false,
                target_languages: vec![],
                language_options: HashMap::new(),
                release_registries: vec![],
                enable_pgp_signing: false,
                docs_config: DocsConfig::default(),
            })
            .to_request();
        assert_eq!(
            test::call_service(&app, req_poly_no_langs).await.status(),
            400
        );

        // 3. Monorepo default name on org + existing repo idempotency
        let req_mono_org = test::TestRequest::post()
            .uri("/github/provision")
            .insert_header(auth_hdr.clone())
            .set_json(ProvisionReposPayload {
                github_org_name: "acme".into(),
                is_user_namespace: false,
                topology_mode: TopologyMode::Monorepo,
                monorepo_name: None,
                polyrepo_names: None,
                description: Some("SDK Monorepo".into()),
                private: false,
                target_languages: vec![TargetLanguage::Rust, TargetLanguage::PythonAll],
                language_options: HashMap::new(),
                release_registries: vec!["crates.io".into()],
                enable_pgp_signing: true,
                docs_config: DocsConfig::default(),
            })
            .to_request();
        let mono_resp: ProvisionReposResponse =
            test::call_and_read_body_json(&app, req_mono_org).await;
        assert_eq!(mono_resp.repositories.len(), 1);
        assert_eq!(mono_resp.repositories[0].name, "acme-sdk");

        let req_mono_existing = test::TestRequest::post()
            .uri("/github/provision")
            .insert_header(auth_hdr.clone())
            .set_json(ProvisionReposPayload {
                github_org_name: "acme".into(),
                is_user_namespace: false,
                topology_mode: TopologyMode::Monorepo,
                monorepo_name: Some("existing-monorepo".into()),
                polyrepo_names: None,
                description: None,
                private: false,
                target_languages: vec![TargetLanguage::Rust],
                language_options: HashMap::new(),
                release_registries: vec![],
                enable_pgp_signing: false,
                docs_config: DocsConfig::default(),
            })
            .to_request();
        let mono_ex_resp: ProvisionReposResponse =
            test::call_and_read_body_json(&app, req_mono_existing).await;
        assert_eq!(mono_ex_resp.repositories[0].github_id, Some(999));

        // 4. Monorepo custom name on user namespace
        let req_mono_user = test::TestRequest::post()
            .uri("/github/provision")
            .insert_header(auth_hdr.clone())
            .set_json(ProvisionReposPayload {
                github_org_name: "octocat".into(),
                is_user_namespace: true,
                topology_mode: TopologyMode::Monorepo,
                monorepo_name: Some("custom-monorepo".into()),
                polyrepo_names: None,
                description: None,
                private: true,
                target_languages: vec![TargetLanguage::Ts],
                language_options: HashMap::new(),
                release_registries: vec![],
                enable_pgp_signing: false,
                docs_config: DocsConfig::default(),
            })
            .to_request();
        let mono_user_resp: ProvisionReposResponse =
            test::call_and_read_body_json(&app, req_mono_user).await;
        assert_eq!(mono_user_resp.repositories[0].name, "custom-monorepo");

        // 5. Polyrepo on org and user namespace (with custom per-language repo name & polyrepo_names map)
        let mut lang_opts = HashMap::new();
        lang_opts.insert(
            "rust".to_string(),
            LanguageOptionConfig {
                sdk: true,
                cli: true,
                mcp: true,
                angular: false,
                framework: Some("actix".into()),
                orm: Some("diesel".into()),
                custom_repo_name: Some("acme-rust-custom".into()),
            },
        );
        let req_poly_org = test::TestRequest::post()
            .uri("/github/provision")
            .insert_header(auth_hdr.clone())
            .set_json(ProvisionReposPayload {
                github_org_name: "acme".into(),
                is_user_namespace: false,
                topology_mode: TopologyMode::Polyrepo,
                monorepo_name: None,
                polyrepo_names: None,
                description: None,
                private: false,
                target_languages: vec![TargetLanguage::Rust, TargetLanguage::Go],
                language_options: lang_opts,
                release_registries: vec![],
                enable_pgp_signing: false,
                docs_config: DocsConfig::default(),
            })
            .to_request();
        let poly_org_resp: ProvisionReposResponse =
            test::call_and_read_body_json(&app, req_poly_org).await;
        assert_eq!(poly_org_resp.repositories.len(), 2);
        assert_eq!(poly_org_resp.repositories[0].name, "acme-rust-custom");
        assert_eq!(poly_org_resp.repositories[1].name, "acme-sdk-go");

        let mut poly_map = std::collections::BTreeMap::new();
        poly_map.insert("python-all".to_string(), "octocat-py-custom".to_string());
        let req_poly_user = test::TestRequest::post()
            .uri("/github/provision")
            .insert_header(auth_hdr)
            .set_json(ProvisionReposPayload {
                github_org_name: "octocat".into(),
                is_user_namespace: true,
                topology_mode: TopologyMode::Polyrepo,
                monorepo_name: None,
                polyrepo_names: Some(poly_map),
                description: None,
                private: false,
                target_languages: vec![TargetLanguage::PythonAll],
                language_options: HashMap::new(),
                release_registries: vec![],
                enable_pgp_signing: false,
                docs_config: DocsConfig::default(),
            })
            .to_request();
        let poly_user_resp: ProvisionReposResponse =
            test::call_and_read_body_json(&app, req_poly_user).await;
        assert_eq!(poly_user_resp.repositories.len(), 1);
        assert_eq!(poly_user_resp.repositories[0].name, "octocat-py-custom");
    }

    #[actix_web::test]
    async fn test_sync_github_success_and_partial_failures() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_upsert_organization()
            .times(2)
            .returning(|id, login, desc| {
                Ok(Organization {
                    id: 1,
                    github_id: Some(id),
                    login,
                    description: desc,
                })
            });
        mock_repo
            .expect_upsert_organization()
            .times(1)
            .returning(|_, _, _| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));

        mock_repo
            .expect_upsert_repository()
            .times(1)
            .returning(|org_id, gh_id, name, desc| {
                Ok(Repository {
                    id: 1,
                    organization_id: org_id,
                    github_id: Some(gh_id),
                    name,
                    description: desc,
                    github_org_name: String::new(),
                    topology_mode: "monorepo".into(),
                    target_languages_json: "[]".into(),
                    language_options_json: "{}".into(),
                    release_registries_json: "[]".into(),
                    enable_pgp_signing: false,
                    docs_config_json: "{}".into(),
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
                    id: 100,
                    login: "org_ok".into(),
                    description: None,
                },
                GitHubOrg {
                    id: 150,
                    login: "org_repos_err".into(),
                    description: None,
                },
                GitHubOrg {
                    id: 200,
                    login: "org_fail".into(),
                    description: None,
                },
            ])
        });
        mock_gh.expect_list_repos().returning(|_, org| {
            if org == "org_repos_err" {
                Err(CddGatewayError::Internal("list repos err".into()))
            } else {
                Ok(vec![
                    GitHubRepo {
                        id: 10,
                        name: "repo_ok".into(),
                        full_name: "org_ok/repo_ok".into(),
                        description: None,
                    },
                    GitHubRepo {
                        id: 20,
                        name: "repo_fail".into(),
                        full_name: "org_ok/repo_fail".into(),
                        description: None,
                    },
                ])
            }
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
        let resp: SyncStatus = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
    }

    #[actix_web::test]
    async fn test_sync_github_no_token_and_list_orgs_error() {
        let mock_repo = MockCddRepository::new();
        let mock_gh = MockGitHubClient::new();

        let cfg = AppConfig {
            github_token: None,
            ..Default::default()
        };

        let app_no_tok = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(cfg))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/github/sync")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .to_request();
        assert_eq!(test::call_service(&app_no_tok, req).await.status(), 401);

        let mock_repo2 = MockCddRepository::new();
        let mut mock_gh2 = MockGitHubClient::new();
        mock_gh2
            .expect_list_orgs()
            .returning(|_| Err(CddGatewayError::Internal("gh error".into())));

        let app_err = test::init_service(
            App::new()
                .app_data(web::Data::new(
                    Arc::new(mock_repo2) as Arc<dyn CddRepository>
                ))
                .app_data(web::Data::new(Arc::new(mock_gh2) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(AppConfig::default()))
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
    async fn test_webhook_receiver_valid_and_invalid() -> Result<(), hmac::digest::InvalidLength> {
        let mock_repo = MockCddRepository::new();
        let mock_gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(AppConfig::default()))
                .configure(configure),
        )
        .await;

        let payload = r#"{"action":"published"}"#;
        let mut mac = HmacSha256::new_from_slice(b"my_webhook_secret")?;
        mac.update(payload.as_bytes());
        let sig = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        let req_ok = test::TestRequest::post()
            .uri("/github/webhook")
            .insert_header(("x-hub-signature-256", sig))
            .set_payload(payload)
            .to_request();
        assert_eq!(test::call_service(&app, req_ok).await.status(), 200);

        let req_no_prefix = test::TestRequest::post()
            .uri("/github/webhook")
            .insert_header(("x-hub-signature-256", "invalid_prefix"))
            .set_payload(payload)
            .to_request();
        assert_eq!(test::call_service(&app, req_no_prefix).await.status(), 401);

        let req_bad_hex = test::TestRequest::post()
            .uri("/github/webhook")
            .insert_header(("x-hub-signature-256", "sha256=zzzz"))
            .set_payload(payload)
            .to_request();
        assert_eq!(test::call_service(&app, req_bad_hex).await.status(), 401);

        let req_wrong_sig = test::TestRequest::post()
            .uri("/github/webhook")
            .insert_header(("x-hub-signature-256", "sha256=0000"))
            .set_payload(payload)
            .to_request();
        assert_eq!(test::call_service(&app, req_wrong_sig).await.status(), 401);

        Ok(())
    }

    #[actix_web::test]
    async fn test_releases_actions_and_secrets_endpoints() {
        use crate::github::models::GitHubRelease;

        let mock_repo = MockCddRepository::new();
        let mut mock_gh = MockGitHubClient::new();

        mock_gh
            .expect_create_release()
            .times(1)
            .returning(|_, _, _, _, _, _| {
                Ok(GitHubRelease {
                    id: 1,
                    tag_name: "v1.0.0".into(),
                    name: None,
                    body: None,
                })
            });
        mock_gh
            .expect_create_release()
            .times(1)
            .returning(|_, _, _, _, _, _| Err(CddGatewayError::Internal("err".into())));

        mock_gh
            .expect_trigger_workflow()
            .times(1)
            .returning(|_, _, _, _, _| Ok(()));
        mock_gh
            .expect_trigger_workflow()
            .times(1)
            .returning(|_, _, _, _, _| Err(CddGatewayError::Internal("err".into())));

        mock_gh
            .expect_get_repo_public_key()
            .times(1)
            .returning(|_, _, _| {
                Ok(GitHubPublicKey {
                    key_id: "k1".into(),
                    key: "pk".into(),
                })
            });
        mock_gh
            .expect_create_repo_secret()
            .times(1)
            .returning(|_, _, _, _, _, _| Ok(()));

        mock_gh
            .expect_get_repo_public_key()
            .times(1)
            .returning(|_, _, _| Err(CddGatewayError::Internal("pk err".into())));

        mock_gh
            .expect_get_repo_public_key()
            .times(1)
            .returning(|_, _, _| {
                Ok(GitHubPublicKey {
                    key_id: "k1".into(),
                    key: "pk".into(),
                })
            });
        mock_gh
            .expect_create_repo_secret()
            .times(1)
            .returning(|_, _, _, _, _, _| Err(CddGatewayError::Internal("sec err".into())));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let auth_hdr = ("Authorization", format!("Bearer {}", generate_test_token()));

        let rel_payload = || ReleasePayload {
            owner: "o".into(),
            repo: "r".into(),
            tag_name: "v1.0.0".into(),
            name: None,
            body: None,
        };
        let req_rel_ok = test::TestRequest::post()
            .uri("/github/releases")
            .insert_header(auth_hdr.clone())
            .set_json(rel_payload())
            .to_request();
        assert_eq!(test::call_service(&app, req_rel_ok).await.status(), 201);

        let req_rel_err = test::TestRequest::post()
            .uri("/github/releases")
            .insert_header(auth_hdr.clone())
            .set_json(rel_payload())
            .to_request();
        assert_eq!(test::call_service(&app, req_rel_err).await.status(), 500);

        let wf_payload = || TriggerWorkflowPayload {
            owner: "o".into(),
            repo: "r".into(),
            workflow_id: "ci.yml".into(),
            ref_branch: "main".into(),
        };
        let req_wf_ok = test::TestRequest::post()
            .uri("/github/actions/dispatch")
            .insert_header(auth_hdr.clone())
            .set_json(wf_payload())
            .to_request();
        assert_eq!(test::call_service(&app, req_wf_ok).await.status(), 200);

        let req_wf_err = test::TestRequest::post()
            .uri("/github/actions/dispatch")
            .insert_header(auth_hdr.clone())
            .set_json(wf_payload())
            .to_request();
        assert_eq!(test::call_service(&app, req_wf_err).await.status(), 500);

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

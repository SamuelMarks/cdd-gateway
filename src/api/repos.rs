#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Repository creation and update endpoints with organization RBAC enforcement (`POST /repos`, `PATCH /repos/{id}`).

use crate::api::auth_middleware::AuthenticatedUser;
use crate::db::models::{
    DocsConfig, LanguageOptionConfig, OrgRole, RepositoryConfigMetadata, TargetLanguage,
    TopologyMode,
};
use crate::db::repository::CddRepository;
use crate::error::CddGatewayError;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::ToSchema;

/// Request payload for creating a new repository within an organization.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateRepoPayload {
    /// Primary key ID of the organization that will own the repository.
    pub organization_id: i32,
    /// Name of the new repository.
    pub name: String,
    /// Optional human-readable description of the repository.
    #[serde(default)]
    pub description: Option<String>,
    /// GitHub organization or user namespace (`${GITHUB_ORG_NAME}`).
    #[serde(default)]
    pub github_org_name: String,
    /// Topology mode (`monorepo` or `polyrepo`).
    #[serde(default)]
    pub topology_mode: TopologyMode,
    /// Target languages configured for code generation.
    #[serde(default)]
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

/// Request payload for updating an existing repository (`PATCH /repos/{id}`).
#[derive(Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct UpdateRepoPayload {
    /// Optional updated name of the repository.
    #[serde(default)]
    pub name: Option<String>,
    /// Optional updated description of the repository.
    #[serde(default)]
    pub description: Option<String>,
    /// GitHub organization or user namespace (`${GITHUB_ORG_NAME}`).
    #[serde(default)]
    pub github_org_name: String,
    /// Topology mode (`monorepo` or `polyrepo`).
    #[serde(default)]
    pub topology_mode: TopologyMode,
    /// Target languages configured for code generation.
    #[serde(default)]
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

/// Creates a repository inside an organization (`POST /repos`).
///
/// Requires the authenticated caller to hold [`OrgRole::Owner`] in the organization.
///
/// # Arguments
/// * `payload` - JSON body containing `organization_id`, `name`, `description`, and topology metadata.
/// * `repo` - Database repository handle.
/// * `auth_user` - Authenticated caller identity.
///
/// # Returns
/// * `Ok(HttpResponse)` - `201 Created` with the created [`crate::db::models::Repository`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Forbidden`] - If the caller does not have the `owner` role in the organization.
/// * [`CddGatewayError`] - On database lookup or insertion failure.
#[utoipa::path(
    post,
    path = "/repos",
    request_body = CreateRepoPayload,
    responses(
        (status = 201, description = "Repository created successfully", body = crate::db::models::Repository),
        (status = 403, description = "Forbidden: Insufficient permissions"),
        (status = 500, description = "Internal Server Error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_repo(
    payload: web::Json<CreateRepoPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    auth_user: AuthenticatedUser,
) -> Result<HttpResponse, CddGatewayError> {
    let role_opt = repo
        .get_user_role(payload.organization_id, auth_user.user_id)
        .await?;

    let is_owner = role_opt
        .as_deref()
        .and_then(|r| r.parse::<OrgRole>().ok())
        .is_some_and(|role| role == OrgRole::Owner);

    if !is_owner {
        return Err(CddGatewayError::Forbidden(
            "Only organization owners can create repositories".to_string(),
        ));
    }

    let inner = payload.into_inner();
    let has_custom_config = !inner.github_org_name.is_empty()
        || inner.topology_mode != TopologyMode::Monorepo
        || !inner.target_languages.is_empty()
        || !inner.language_options.is_empty()
        || !inner.release_registries.is_empty()
        || inner.enable_pgp_signing
        || inner.docs_config != DocsConfig::default();

    let created_repo = if has_custom_config {
        let metadata = RepositoryConfigMetadata {
            github_org_name: inner.github_org_name,
            topology_mode: inner.topology_mode,
            target_languages: inner.target_languages,
            language_options: inner.language_options,
            release_registries: inner.release_registries,
            enable_pgp_signing: inner.enable_pgp_signing,
            docs_config: inner.docs_config,
        };
        repo.create_repository_with_config(
            inner.organization_id,
            None,
            inner.name,
            inner.description,
            metadata,
        )
        .await?
    } else {
        repo.create_repository(inner.organization_id, None, inner.name, inner.description)
            .await?
    };

    Ok(HttpResponse::Created().json(created_repo))
}

/// Updates an existing repository's topology, target languages, registries, and docs configuration
/// (`PATCH /repos/{id}`).
///
/// Requires the authenticated caller to hold [`OrgRole::Owner`] or [`OrgRole::Admin`] in the owning organization.
///
/// # Arguments
/// * `path` - Path parameter containing the numeric repository ID.
/// * `payload` - JSON body containing updated repository metadata.
/// * `repo` - Database repository handle.
/// * `auth_user` - Authenticated caller identity.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with the updated [`crate::db::models::Repository`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::NotFound`] - If the repository does not exist.
/// * [`CddGatewayError::Forbidden`] - If the caller is not an `owner` or `admin` of the organization.
/// * [`CddGatewayError`] - On database lookup or update failure.
#[utoipa::path(
    patch,
    path = "/repos/{id}",
    params(
        ("id" = i32, Path, description = "Repository ID")
    ),
    request_body = UpdateRepoPayload,
    responses(
        (status = 200, description = "Repository updated successfully", body = crate::db::models::Repository),
        (status = 403, description = "Forbidden: Insufficient permissions"),
        (status = 404, description = "Repository not found")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_repo(
    path: web::Path<i32>,
    payload: web::Json<UpdateRepoPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    auth_user: AuthenticatedUser,
) -> Result<HttpResponse, CddGatewayError> {
    let repo_id = path.into_inner();
    let existing = repo
        .get_repository(repo_id)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound(format!("Repository {repo_id} not found")))?;

    let role_opt = repo
        .get_user_role(existing.organization_id, auth_user.user_id)
        .await?;

    let is_authorized = role_opt
        .as_deref()
        .and_then(|r| r.parse::<OrgRole>().ok())
        .is_some_and(|role| matches!(role, OrgRole::Owner | OrgRole::Admin));

    if !is_authorized {
        return Err(CddGatewayError::Forbidden(
            "Only organization owners or admins can update repositories".to_string(),
        ));
    }

    let inner = payload.into_inner();
    let name = inner.name.unwrap_or(existing.name);
    let description = inner.description.or(existing.description);
    let github_org_name = if inner.github_org_name.is_empty() {
        existing.github_org_name
    } else {
        inner.github_org_name
    };

    let metadata = RepositoryConfigMetadata {
        github_org_name,
        topology_mode: inner.topology_mode,
        target_languages: inner.target_languages,
        language_options: inner.language_options,
        release_registries: inner.release_registries,
        enable_pgp_signing: inner.enable_pgp_signing,
        docs_config: inner.docs_config,
    };

    let updated = repo
        .update_repository_with_config(repo_id, name, description, metadata)
        .await?;

    Ok(HttpResponse::Ok().json(updated))
}

/// Registers `/repos` scope routes on `cfg`.
///
/// # Arguments
/// * `cfg` - Actix-web service configuration builder.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/repos")
            .route("", web::post().to(create_repo))
            .route("/{id}", web::patch().to(update_repo)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::Repository;
    use crate::db::repository::MockCddRepository;
    use actix_web::{test, App};

    fn generate_test_token() -> String {
        crate::api::auth_middleware::generate_test_token()
    }

    fn base_payload() -> CreateRepoPayload {
        CreateRepoPayload {
            organization_id: 1,
            name: "new_repo".to_string(),
            description: None,
            github_org_name: String::new(),
            topology_mode: TopologyMode::Monorepo,
            target_languages: Vec::new(),
            language_options: HashMap::new(),
            release_registries: Vec::new(),
            enable_pgp_signing: false,
            docs_config: DocsConfig::default(),
        }
    }

    #[actix_web::test]
    async fn test_create_and_update_repo_owner() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_get_user_role()
            .returning(|_, _| Ok(Some("owner".to_string())));
        mock_repo
            .expect_create_repository()
            .returning(|_, _, _, _| {
                Ok(Repository {
                    id: 1,
                    organization_id: 1,
                    github_id: None,
                    name: "new_repo".into(),
                    description: None,
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
            .expect_create_repository_with_config()
            .returning(|_, _, name, _, meta| {
                Ok(Repository {
                    id: 2,
                    organization_id: 1,
                    github_id: None,
                    name,
                    description: None,
                    github_org_name: meta.github_org_name,
                    topology_mode: meta.topology_mode.to_string(),
                    target_languages_json: r#"["rust"]"#.into(),
                    language_options_json: "{}".into(),
                    release_registries_json: r#"["crates.io"]"#.into(),
                    enable_pgp_signing: meta.enable_pgp_signing,
                    docs_config_json: "{}".into(),
                })
            });
        mock_repo.expect_get_repository().returning(|id| {
            if id == 2 {
                Ok(Some(Repository {
                    id: 2,
                    organization_id: 1,
                    github_id: None,
                    name: "old_name".into(),
                    description: Some("old".into()),
                    github_org_name: "acme".into(),
                    topology_mode: "monorepo".into(),
                    target_languages_json: "[]".into(),
                    language_options_json: "{}".into(),
                    release_registries_json: "[]".into(),
                    enable_pgp_signing: false,
                    docs_config_json: "{}".into(),
                }))
            } else {
                Ok(None)
            }
        });
        mock_repo
            .expect_update_repository_with_config()
            .returning(|id, name, desc, meta| {
                Ok(Repository {
                    id,
                    organization_id: 1,
                    github_id: None,
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
            });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let auth_hdr = ("Authorization", format!("Bearer {}", generate_test_token()));

        let req = test::TestRequest::post()
            .uri("/repos")
            .insert_header(auth_hdr.clone())
            .set_json(base_payload())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::CREATED);

        let mut custom_payload = base_payload();
        custom_payload.github_org_name = "my-org".into();
        custom_payload.topology_mode = TopologyMode::Polyrepo;
        custom_payload.target_languages = vec![TargetLanguage::Rust];
        custom_payload.release_registries = vec!["crates.io".into()];
        custom_payload.enable_pgp_signing = true;

        let req_custom = test::TestRequest::post()
            .uri("/repos")
            .insert_header(auth_hdr.clone())
            .set_json(custom_payload)
            .to_request();
        let resp_custom = test::call_service(&app, req_custom).await;
        assert_eq!(resp_custom.status(), actix_web::http::StatusCode::CREATED);

        let mut p_topo = base_payload();
        p_topo.topology_mode = TopologyMode::Polyrepo;
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/repos")
                    .insert_header(auth_hdr.clone())
                    .set_json(p_topo)
                    .to_request()
            )
            .await
            .status(),
            actix_web::http::StatusCode::CREATED
        );

        let mut p_langs = base_payload();
        p_langs.target_languages = vec![TargetLanguage::Rust];
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/repos")
                    .insert_header(auth_hdr.clone())
                    .set_json(p_langs)
                    .to_request()
            )
            .await
            .status(),
            actix_web::http::StatusCode::CREATED
        );

        let mut p_opts = base_payload();
        p_opts
            .language_options
            .insert("rust".into(), LanguageOptionConfig::default());
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/repos")
                    .insert_header(auth_hdr.clone())
                    .set_json(p_opts)
                    .to_request()
            )
            .await
            .status(),
            actix_web::http::StatusCode::CREATED
        );

        let mut p_regs = base_payload();
        p_regs.release_registries = vec!["crates.io".into()];
        assert_eq!(
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/repos")
                    .insert_header(auth_hdr.clone())
                    .set_json(p_regs)
                    .to_request()
            )
            .await
            .status(),
            actix_web::http::StatusCode::CREATED
        );

        // PATCH /repos/2 with default github_org_name fallback & explicit github_org_name
        let req_patch_ok = test::TestRequest::patch()
            .uri("/repos/2")
            .insert_header(auth_hdr.clone())
            .set_json(UpdateRepoPayload::default())
            .to_request();
        assert_eq!(test::call_service(&app, req_patch_ok).await.status(), 200);

        let req_patch_explicit = test::TestRequest::patch()
            .uri("/repos/2")
            .insert_header(auth_hdr.clone())
            .set_json(UpdateRepoPayload {
                name: Some("explicit_name".into()),
                description: Some("explicit_desc".into()),
                github_org_name: "new-org".into(),
                topology_mode: TopologyMode::Polyrepo,
                target_languages: vec![TargetLanguage::Rust],
                language_options: HashMap::new(),
                release_registries: vec!["crates.io".into()],
                enable_pgp_signing: true,
                docs_config: DocsConfig::default(),
            })
            .to_request();
        assert_eq!(
            test::call_service(&app, req_patch_explicit).await.status(),
            200
        );

        // PATCH /repos/999 -> 404 Not Found
        let req_patch_404 = test::TestRequest::patch()
            .uri("/repos/999")
            .insert_header(auth_hdr)
            .set_json(UpdateRepoPayload::default())
            .to_request();
        assert_eq!(test::call_service(&app, req_patch_404).await.status(), 404);
    }

    #[actix_web::test]
    async fn test_create_and_update_repo_forbidden() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_get_user_role()
            .returning(|_, _| Ok(Some("member".to_string())));
        mock_repo.expect_get_repository().returning(|_| {
            Ok(Some(Repository {
                id: 1,
                organization_id: 1,
                github_id: None,
                name: "r1".into(),
                description: None,
                github_org_name: String::new(),
                topology_mode: "monorepo".into(),
                target_languages_json: "[]".into(),
                language_options_json: "{}".into(),
                release_registries_json: "[]".into(),
                enable_pgp_signing: false,
                docs_config_json: "{}".into(),
            }))
        });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let auth_hdr = ("Authorization", format!("Bearer {}", generate_test_token()));

        let req = test::TestRequest::post()
            .uri("/repos")
            .insert_header(auth_hdr.clone())
            .set_json(base_payload())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::FORBIDDEN);

        let req_patch = test::TestRequest::patch()
            .uri("/repos/1")
            .insert_header(auth_hdr)
            .set_json(UpdateRepoPayload::default())
            .to_request();
        let resp_patch = test::call_service(&app, req_patch).await;
        assert_eq!(resp_patch.status(), actix_web::http::StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn test_create_repo_db_fail_and_role_fail() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_get_user_role()
            .times(1)
            .returning(|_, _| Ok(Some("owner".to_string())));
        mock_repo
            .expect_create_repository()
            .times(1)
            .returning(|_, _, _, _| {
                Err(CddGatewayError::Database(diesel::result::Error::NotFound))
            });
        mock_repo
            .expect_get_user_role()
            .times(1)
            .returning(|_, _| Ok(Some("owner".to_string())));
        mock_repo
            .expect_create_repository_with_config()
            .times(1)
            .returning(|_, _, _, _, _| {
                Err(CddGatewayError::Database(diesel::result::Error::NotFound))
            });
        mock_repo
            .expect_get_user_role()
            .times(1)
            .returning(|_, _| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let req1 = test::TestRequest::post()
            .uri("/repos")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(base_payload())
            .to_request();
        assert_eq!(
            test::call_service(&app, req1).await.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        let mut custom = base_payload();
        custom.enable_pgp_signing = true;
        let req_custom_err = test::TestRequest::post()
            .uri("/repos")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(custom)
            .to_request();
        assert_eq!(
            test::call_service(&app, req_custom_err).await.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        let req2 = test::TestRequest::post()
            .uri("/repos")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(base_payload())
            .to_request();
        assert_eq!(
            test::call_service(&app, req2).await.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}

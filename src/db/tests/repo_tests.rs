#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Integration tests for repository CRUD, topology metadata, GitHub ID upserts, and connection pool errors.

use crate::db::models::{RepositoryConfigMetadata, TargetLanguage, TopologyMode};
use crate::db::repository::CddRepository;
use crate::db::tests::setup_test_db;
use crate::error::CddGatewayError;
use uuid::Uuid;

/// Verifies creating a repository (both default and with topology config) and retrieving it by its primary key ID.
#[tokio::test]
async fn test_create_and_get_repository() -> Result<(), CddGatewayError> {
    let repo = setup_test_db()?;

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login.clone(), None).await?;

    let repo_name = format!("repo_{}", Uuid::new_v4());
    let created_repo = repo
        .create_repository(
            org.id,
            None,
            repo_name.clone(),
            Some("Repo Desc".to_string()),
        )
        .await?;
    assert_eq!(created_repo.name, repo_name);
    assert_eq!(created_repo.organization_id, org.id);

    let found_repo = repo
        .get_repository(created_repo.id)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound("repo not found".to_string()))?;
    assert_eq!(found_repo.id, created_repo.id);
    assert_eq!(found_repo.name, repo_name);

    let custom_name = format!("repo_cfg_{}", Uuid::new_v4());
    let meta = RepositoryConfigMetadata {
        github_org_name: login,
        topology_mode: TopologyMode::Polyrepo,
        target_languages: vec![TargetLanguage::Rust],
        enable_pgp_signing: true,
        ..Default::default()
    };
    let cfg_repo = repo
        .create_repository_with_config(org.id, None, custom_name.clone(), None, meta.clone())
        .await?;
    assert_eq!(cfg_repo.name, custom_name);
    assert_eq!(cfg_repo.topology_mode, "polyrepo");
    assert!(cfg_repo.enable_pgp_signing);

    let updated_name = format!("{custom_name}_patched");
    let patched_repo = repo
        .update_repository_with_config(
            cfg_repo.id,
            updated_name.clone(),
            Some("Patched".to_string()),
            meta,
        )
        .await?;
    assert_eq!(patched_repo.name, updated_name);
    assert_eq!(patched_repo.description.as_deref(), Some("Patched"));

    Ok(())
}

/// Verifies inserting and updating a repository keyed by its GitHub ID.
#[tokio::test]
async fn test_upsert_repository() -> Result<(), CddGatewayError> {
    let repo = setup_test_db()?;

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login, None).await?;

    let github_id =
        i64::try_from(Uuid::new_v4().as_u128() % 1_000_000_000).unwrap_or(0) + 3_000_000;
    let repo_name = format!("repo_{}", Uuid::new_v4());

    let repo1 = repo
        .upsert_repository(org.id, github_id, repo_name.clone(), None)
        .await?;
    assert_eq!(repo1.github_id, Some(github_id));
    assert_eq!(repo1.name, repo_name);

    let new_name = format!("repo_{}_new", Uuid::new_v4());
    let repo2 = repo
        .upsert_repository(
            org.id,
            github_id,
            new_name.clone(),
            Some("Updated Repo".to_string()),
        )
        .await?;
    assert_eq!(repo2.id, repo1.id);
    assert_eq!(repo2.name, new_name);
    assert_eq!(repo2.description, Some("Updated Repo".to_string()));
    Ok(())
}

/// Verifies that `PgRepository::get_conn` returns a [`crate::error::CddGatewayError::Pool`]
/// error when checking out a connection from an invalid pool fails.
#[tokio::test]
async fn test_pg_repository_get_conn_error() {
    use crate::db::repository::PgRepository;
    use diesel::pg::PgConnection;
    use diesel::r2d2::{ConnectionManager, Pool};

    let manager =
        ConnectionManager::<PgConnection>::new("postgres://invalid:invalid@127.0.0.1:1/invalid");
    let pool = Pool::builder()
        .max_size(1)
        .min_idle(Some(0))
        .connection_timeout(std::time::Duration::from_millis(1))
        .test_on_check_out(false)
        .build_unchecked(manager);

    let repo = PgRepository { pool };
    let result = repo.get_conn();
    assert!(result.is_err());
}

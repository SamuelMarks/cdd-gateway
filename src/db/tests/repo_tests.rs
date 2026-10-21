#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Integration tests for repository CRUD, GitHub ID upserts, and connection pool errors.

use crate::db::repository::CddRepository;
use crate::db::tests::{setup_test_db, TestError};
use uuid::Uuid;

/// Verifies creating a repository and retrieving it by its primary key ID.
#[tokio::test]
async fn test_create_and_get_repository() -> Result<(), TestError> {
    let repo = setup_test_db()?;

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login, None).await?;

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
        .ok_or(TestError::NoneError)?;
    assert_eq!(found_repo.id, created_repo.id);
    assert_eq!(found_repo.name, repo_name);
    Ok(())
}

/// Verifies inserting and updating a repository keyed by its GitHub ID.
#[tokio::test]
async fn test_upsert_repository() -> Result<(), TestError> {
    let repo = setup_test_db()?;

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login, None).await?;

    let github_id = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
            % 1_000_000_000,
    )
    .unwrap_or(0)
        + 3_000_000;
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

    let manager = ConnectionManager::<PgConnection>::new("postgres://invalid:5432/db");
    let pool = Pool::builder()
        .max_size(1)
        .connection_timeout(std::time::Duration::from_millis(1))
        .test_on_check_out(false)
        .build_unchecked(manager);

    let repo = PgRepository { pool };
    let result = repo.get_conn();
    assert!(result.is_err());
}

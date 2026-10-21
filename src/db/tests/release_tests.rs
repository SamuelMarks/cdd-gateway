#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Integration tests for repository release creation and GitHub ID upserts.

use crate::db::repository::CddRepository;
use crate::db::tests::{setup_test_db, TestError};
use uuid::Uuid;

/// Verifies creating a release and upserting a release by its GitHub ID.
#[tokio::test]
async fn test_create_and_upsert_release() -> Result<(), TestError> {
    let repo = setup_test_db()?;

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login, None).await?;

    let repo_name = format!("repo_{}", Uuid::new_v4());
    let created_repo = repo
        .create_repository(org.id, None, repo_name, None)
        .await?;

    let tag_name = format!("v1.0.0-{}", Uuid::new_v4());
    let release1 = repo
        .create_release(
            created_repo.id,
            None,
            tag_name.clone(),
            Some("Release 1".to_string()),
            Some("Body 1".to_string()),
        )
        .await?;
    assert_eq!(release1.tag_name, tag_name);

    let github_id = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
            % 1_000_000_000,
    )
    .unwrap_or(0)
        + 4_000_000;
    let tag_name2 = format!("v2.0.0-{}", Uuid::new_v4());

    let release2 = repo
        .upsert_release(
            created_repo.id,
            github_id,
            tag_name2.clone(),
            Some("Release 2".to_string()),
            None,
        )
        .await?;
    assert_eq!(release2.github_id, Some(github_id));
    assert_eq!(release2.tag_name, tag_name2);

    let release3 = repo
        .upsert_release(
            created_repo.id,
            github_id,
            tag_name2.clone(),
            Some("Release 3".to_string()),
            Some("Body 3".to_string()),
        )
        .await?;
    assert_eq!(release3.id, release2.id);
    assert_eq!(release3.name, Some("Release 3".to_string()));
    Ok(())
}

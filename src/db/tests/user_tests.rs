#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Integration tests for user creation, lookup, and GitHub ID upserts.

use crate::db::repository::CddRepository;
use crate::db::tests::{setup_test_db, TestError};
use uuid::Uuid;

/// Verifies creating a user and looking them up by username and primary key ID.
#[tokio::test]
async fn test_create_and_find_user() -> Result<(), TestError> {
    let repo = setup_test_db()?;
    let username = format!("user_{}", Uuid::new_v4());
    let email = format!("{username}@example.com");

    let created = repo
        .create_user(
            None,
            username.clone(),
            email.clone(),
            Some("hash".to_string()),
        )
        .await?;
    assert_eq!(created.username, username);
    assert_eq!(created.email, email);

    let found_by_name = repo
        .find_user_by_username(username.clone())
        .await?
        .ok_or(TestError::NoneError)?;
    assert_eq!(found_by_name.id, created.id);

    let found_by_id = repo
        .find_user_by_id(created.id)
        .await?
        .ok_or(TestError::NoneError)?;
    assert_eq!(found_by_id.username, username);
    Ok(())
}

/// Verifies inserting and updating a user keyed by their GitHub ID.
#[tokio::test]
async fn test_upsert_user() -> Result<(), TestError> {
    let repo = setup_test_db()?;
    let github_id = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
            % 1_000_000_000,
    )
    .unwrap_or(0)
        + 1_000_000;
    let username = format!("user_{}", Uuid::new_v4());
    let email = format!("{username}@example.com");

    let user1 = repo
        .upsert_user(github_id, username.clone(), email.clone())
        .await?;
    assert_eq!(user1.github_id, Some(github_id));
    assert_eq!(user1.username, username);

    let new_username = format!("user_{}_new", Uuid::new_v4());
    let user2 = repo
        .upsert_user(github_id, new_username.clone(), email.clone())
        .await?;
    assert_eq!(user2.id, user1.id);
    assert_eq!(user2.username, new_username);
    Ok(())
}

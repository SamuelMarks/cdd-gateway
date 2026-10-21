#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Integration tests for user creation, lookup, and GitHub ID upserts.

use crate::db::repository::CddRepository;
use crate::db::tests::setup_test_db;
use crate::error::CddGatewayError;
use uuid::Uuid;

/// Verifies creating a user and looking them up by username and primary key ID.
#[tokio::test]
async fn test_create_and_find_user() -> Result<(), CddGatewayError> {
    let repo = setup_test_db()?;

    let username = format!("user_{}", Uuid::new_v4());
    let email = format!("{username}@example.com");

    let created_user = repo
        .create_user(
            None,
            username.clone(),
            email.clone(),
            Some("hashed_pw".to_string()),
        )
        .await?;
    assert_eq!(created_user.username, username);
    assert_eq!(created_user.email, email);

    let found_by_name = repo
        .find_user_by_username(username)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound("user not found".to_string()))?;
    assert_eq!(found_by_name.id, created_user.id);

    let found_by_id = repo
        .find_user_by_id(created_user.id)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound("user not found".to_string()))?;
    assert_eq!(found_by_id.id, created_user.id);
    Ok(())
}

/// Verifies inserting and updating a user keyed by their GitHub ID.
#[tokio::test]
async fn test_upsert_user() -> Result<(), CddGatewayError> {
    let repo = setup_test_db()?;

    let github_id =
        i64::try_from(Uuid::new_v4().as_u128() % 1_000_000_000).unwrap_or(0) + 1_000_000;
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

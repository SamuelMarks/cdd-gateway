#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Integration tests for user creation, lookup, Google OAuth, account linking, and GitHub ID upserts.

use crate::db::repository::CddRepository;
use crate::db::tests::setup_test_db;
use crate::error::CddGatewayError;
use uuid::Uuid;

/// Verifies creating a user and looking them up by username, email, and primary key ID.
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

    let found_by_email = repo
        .find_user_by_email(email)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound("user not found".to_string()))?;
    assert_eq!(found_by_email.id, created_user.id);

    let found_by_id = repo
        .find_user_by_id(created_user.id)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound("user not found".to_string()))?;
    assert_eq!(found_by_id.id, created_user.id);
    Ok(())
}

/// Verifies inserting and updating a user keyed by their GitHub ID, Google ID, and provider linking/unlinking.
#[tokio::test]
async fn test_upsert_and_link_user_providers() -> Result<(), CddGatewayError> {
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

    // Link Google ID and password hash
    let google_sub = format!("google_sub_{}", Uuid::new_v4());
    let linked_google = repo.link_google_id(user2.id, google_sub.clone()).await?;
    assert_eq!(
        linked_google.google_id.as_deref(),
        Some(google_sub.as_str())
    );

    let found_google = repo
        .find_user_by_google_id(google_sub)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound("google user not found".to_string()))?;
    assert_eq!(found_google.id, user2.id);

    let linked_pw = repo
        .set_password_hash(user2.id, "argon2_hash".to_string())
        .await?;
    assert_eq!(linked_pw.password_hash.as_deref(), Some("argon2_hash"));

    // Unlink GitHub, then re-link GitHub, then unlink Google and password
    let unlinked_gh = repo.unlink_provider(user2.id, "github".to_string()).await?;
    assert!(unlinked_gh.github_id.is_none());

    let relinked_gh = repo.link_github_id(user2.id, github_id).await?;
    assert_eq!(relinked_gh.github_id, Some(github_id));

    let unlinked_google = repo.unlink_provider(user2.id, "google".to_string()).await?;
    assert!(unlinked_google.google_id.is_none());

    let unlinked_pw = repo
        .unlink_provider(user2.id, "password".to_string())
        .await?;
    assert!(unlinked_pw.password_hash.is_none());

    // Cannot unlink last remaining provider (github) or non-existent user
    assert!(repo
        .unlink_provider(user2.id, "github".to_string())
        .await
        .is_err());
    assert!(repo
        .unlink_provider(-999_999, "github".to_string())
        .await
        .is_err());

    // Upsert Google user directly
    let g_sub_2 = format!("gsub2_{}", Uuid::new_v4());
    let g_uname = format!("guser_{}", Uuid::new_v4());
    let g_email = format!("{g_uname}@example.com");
    let gu1 = repo
        .upsert_google_user(g_sub_2.clone(), g_uname.clone(), g_email.clone())
        .await?;
    assert_eq!(gu1.google_id.as_deref(), Some(g_sub_2.as_str()));

    let g_uname_new = format!("{g_uname}_up");
    let gu2 = repo
        .upsert_google_user(g_sub_2, g_uname_new.clone(), g_email)
        .await?;
    assert_eq!(gu2.id, gu1.id);
    assert_eq!(gu2.username, g_uname_new);

    Ok(())
}

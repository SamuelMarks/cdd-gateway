use crate::db::repository::CddRepository;
use crate::db::tests::{setup_test_db, TestError};
use uuid::Uuid;

#[tokio::test]
async fn test_create_and_find_user() -> Result<(), TestError> {
    let repo = setup_test_db()?;
    let username = format!("user_{}", Uuid::new_v4());
    let email = format!("{username}@example.com");
    let github_id = rand::random::<i64>().abs();

    let user = repo
        .create_user(
            Some(github_id),
            username.clone(),
            email.clone(),
            Some("hash".to_string()),
        )
        .await?;
    assert_eq!(user.username, username);
    assert_eq!(user.email, email);

    let found = repo
        .find_user_by_username(username.clone())
        .await?
        .ok_or(TestError::NoneError)?;
    assert_eq!(found.id, user.id);

    let found_by_id = repo
        .find_user_by_id(user.id)
        .await?
        .ok_or(TestError::NoneError)?;
    assert_eq!(found_by_id.username, username);
    Ok(())
}

#[tokio::test]
async fn test_upsert_user() -> Result<(), TestError> {
    let repo = setup_test_db()?;
    let username = format!("user_{}", Uuid::new_v4());
    let email = format!("{username}@example.com");
    let github_id = rand::random::<i64>().abs();

    let user1 = repo
        .upsert_user(github_id, username.clone(), email.clone())
        .await?;
    assert_eq!(user1.username, username);

    // Upsert again with new username
    let new_username = format!("user_{}_new", Uuid::new_v4());
    let user2 = repo
        .upsert_user(github_id, new_username.clone(), email.clone())
        .await?;
    assert_eq!(user2.id, user1.id);
    assert_eq!(user2.username, new_username);
    Ok(())
}

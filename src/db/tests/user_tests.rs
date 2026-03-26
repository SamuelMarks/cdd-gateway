use crate::db::establish_connection_pool;
use crate::db::repository::{CddRepository, PgRepository};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn get_repo() -> PgRepository {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/cdd_ctl_test".to_string());
    let pool = establish_connection_pool(&database_url);
    PgRepository { pool }
}

#[tokio::test]
async fn test_create_and_find_user() {
    let repo = get_repo();
    let username = format!("user_{}", Uuid::new_v4());
    let email = format!("{}@example.com", username);
    let github_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let user = repo
        .create_user(
            Some(github_id),
            username.clone(),
            email.clone(),
            Some("hash".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(user.username, username);
    assert_eq!(user.email, email);

    let found = repo
        .find_user_by_username(username.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.id, user.id);

    let found_by_id = repo.find_user_by_id(user.id).await.unwrap().unwrap();
    assert_eq!(found_by_id.username, username);
}

#[tokio::test]
async fn test_upsert_user() {
    let repo = get_repo();
    let username = format!("user_{}", Uuid::new_v4());
    let email = format!("{}@example.com", username);
    let github_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
        + 1;

    let user1 = repo
        .upsert_user(github_id, username.clone(), email.clone())
        .await
        .unwrap();
    assert_eq!(user1.username, username);

    // Upsert again with new username
    let new_username = format!("user_{}_new", Uuid::new_v4());
    let user2 = repo
        .upsert_user(github_id, new_username.clone(), email.clone())
        .await
        .unwrap();
    assert_eq!(user2.id, user1.id);
    assert_eq!(user2.username, new_username);
}

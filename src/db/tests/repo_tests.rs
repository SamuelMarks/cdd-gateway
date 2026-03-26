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
async fn test_create_and_get_repository() {
    let repo = get_repo();

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo
        .create_organization(None, login.clone(), None)
        .await
        .unwrap();

    let name = format!("repo_{}", Uuid::new_v4());

    let repository = repo
        .create_repository(
            org.id,
            Some(rand::random::<i64>().abs()),
            name.clone(),
            Some("repo desc".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(repository.name, name);
    assert_eq!(repository.description, Some("repo desc".to_string()));

    let found = repo.get_repository(repository.id).await.unwrap().unwrap();
    assert_eq!(found.name, name);
}

#[tokio::test]
async fn test_upsert_repository() {
    let repo = get_repo();

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo
        .create_organization(None, login.clone(), None)
        .await
        .unwrap();

    let name = format!("repo_{}", Uuid::new_v4());
    let github_id = rand::random::<i64>().abs();

    let repo1 = repo
        .upsert_repository(org.id, github_id, name.clone(), Some("desc1".to_string()))
        .await
        .unwrap();
    assert_eq!(repo1.description.unwrap(), "desc1");

    let new_name = format!("repo_{}_new", Uuid::new_v4());
    let repo2 = repo
        .upsert_repository(
            org.id,
            github_id,
            new_name.clone(),
            Some("desc2".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(repo2.id, repo1.id);
    assert_eq!(repo2.description.unwrap(), "desc2");
    assert_eq!(repo2.name, new_name);
}

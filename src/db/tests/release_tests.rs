use crate::db::repository::{CddRepository, PgRepository};
use crate::db::establish_connection_pool;
use uuid::Uuid;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

fn get_repo() -> PgRepository {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost/cdd_ctl_test".to_string());
    let pool = establish_connection_pool(&database_url);
    PgRepository { pool }
}

#[tokio::test]
async fn test_create_and_upsert_release() {
    let repo = get_repo();
    
    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login.clone(), None).await.unwrap();

    let name = format!("repo_{}", Uuid::new_v4());
    let repository = repo.create_repository(org.id, None, name.clone(), None).await.unwrap();

    let tag = format!("v1.0.{}", Uuid::new_v4());
    let release1 = repo.create_release(repository.id, Some(rand::random::<i64>().abs()), tag.clone(), Some("Release 1".to_string()), Some("Body 1".to_string())).await.unwrap();
    assert_eq!(release1.tag_name, tag);
    assert_eq!(release1.name, Some("Release 1".to_string()));

    let github_id = rand::random::<i64>().abs();
    let tag2 = format!("v2.0.{}", Uuid::new_v4());
    let release2 = repo.upsert_release(repository.id, github_id, tag2.clone(), Some("Release 2".to_string()), Some("Body 2".to_string())).await.unwrap();
    assert_eq!(release2.tag_name, tag2);

    let release3 = repo.upsert_release(repository.id, github_id, tag2.clone(), Some("Release 3".to_string()), Some("Body 3".to_string())).await.unwrap();
    assert_eq!(release3.id, release2.id);
    assert_eq!(release3.name, Some("Release 3".to_string()));
}

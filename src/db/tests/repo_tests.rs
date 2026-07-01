use crate::db::repository::CddRepository;
use crate::db::tests::{setup_test_db, TestError};
use uuid::Uuid;

#[tokio::test]
async fn test_create_and_get_repository() -> Result<(), TestError> {
    let repo = setup_test_db()?;

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login.clone(), None).await?;

    let name = format!("repo_{}", Uuid::new_v4());

    let repository = repo
        .create_repository(
            org.id,
            Some(rand::random::<i64>().abs()),
            name.clone(),
            Some("repo desc".to_string()),
        )
        .await?;
    assert_eq!(repository.name, name);
    assert_eq!(repository.description, Some("repo desc".to_string()));

    let found = repo
        .get_repository(repository.id)
        .await?
        .ok_or(TestError::NoneError)?;
    assert_eq!(found.name, name);
    Ok(())
}

#[tokio::test]
async fn test_upsert_repository() -> Result<(), TestError> {
    let repo = setup_test_db()?;

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login.clone(), None).await?;

    let name = format!("repo_{}", Uuid::new_v4());
    let github_id = rand::random::<i64>().abs();

    let repo1 = repo
        .upsert_repository(org.id, github_id, name.clone(), Some("desc1".to_string()))
        .await?;
    assert_eq!(repo1.description.ok_or(TestError::NoneError)?, "desc1");

    let new_name = format!("repo_{}_new", Uuid::new_v4());
    let repo2 = repo
        .upsert_repository(
            org.id,
            github_id,
            new_name.clone(),
            Some("desc2".to_string()),
        )
        .await?;
    assert_eq!(repo2.id, repo1.id);
    assert_eq!(repo2.description.ok_or(TestError::NoneError)?, "desc2");
    assert_eq!(repo2.name, new_name);
    Ok(())
}

#[test]
fn test_get_conn_error() {
    use crate::db::repository::PgRepository;
    use diesel::r2d2::{ConnectionManager, Pool};
    use diesel::PgConnection;

    // Create a pool that times out instantly
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

use crate::db::repository::CddRepository;
use crate::db::tests::{setup_test_db, TestError};
use uuid::Uuid;

#[tokio::test]
async fn test_create_and_upsert_release() -> Result<(), TestError> {
    let repo = setup_test_db()?;

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login.clone(), None).await?;

    let name = format!("repo_{}", Uuid::new_v4());
    let repository = repo
        .create_repository(org.id, None, name.clone(), None)
        .await?;

    let tag = format!("v1.0.{}", Uuid::new_v4());
    let release1 = repo
        .create_release(
            repository.id,
            Some(rand::random::<i64>().abs()),
            tag.clone(),
            Some("Release 1".to_string()),
            Some("Body 1".to_string()),
        )
        .await?;
    assert_eq!(release1.tag_name, tag);
    assert_eq!(release1.name, Some("Release 1".to_string()));

    let github_id = rand::random::<i64>().abs();
    let tag2 = format!("v2.0.{}", Uuid::new_v4());
    let release2 = repo
        .upsert_release(
            repository.id,
            github_id,
            tag2.clone(),
            Some("Release 2".to_string()),
            Some("Body 2".to_string()),
        )
        .await?;
    assert_eq!(release2.tag_name, tag2);

    let release3 = repo
        .upsert_release(
            repository.id,
            github_id,
            tag2.clone(),
            Some("Release 3".to_string()),
            Some("Body 3".to_string()),
        )
        .await?;
    assert_eq!(release3.id, release2.id);
    assert_eq!(release3.name, Some("Release 3".to_string()));
    Ok(())
}

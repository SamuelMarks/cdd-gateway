use crate::db::repository::CddRepository;
use crate::db::tests::{setup_test_db, TestError};
use uuid::Uuid;

#[tokio::test]
async fn test_create_and_get_org() -> Result<(), TestError> {
    let repo = setup_test_db()?;
    let login = format!("org_{}", Uuid::new_v4());

    let org = repo
        .create_organization(
            Some(rand::random::<i64>().abs()),
            login.clone(),
            Some("desc".to_string()),
        )
        .await?;
    assert_eq!(org.login, login);
    assert_eq!(org.description, Some("desc".to_string()));

    let found = repo
        .get_organization(org.id)
        .await?
        .ok_or(TestError::NoneError)?;
    assert_eq!(found.login, login);
    Ok(())
}

#[tokio::test]
async fn test_upsert_org() -> Result<(), TestError> {
    let repo = setup_test_db()?;
    let login = format!("org_{}", Uuid::new_v4());
    let github_id = rand::random::<i64>().abs();

    let org1 = repo
        .upsert_organization(github_id, login.clone(), Some("desc1".to_string()))
        .await?;
    assert_eq!(org1.description.ok_or(TestError::NoneError)?, "desc1");

    let new_login = format!("org_{}_new", Uuid::new_v4());
    let org2 = repo
        .upsert_organization(github_id, new_login.clone(), Some("desc2".to_string()))
        .await?;
    assert_eq!(org2.id, org1.id);
    assert_eq!(org2.description.ok_or(TestError::NoneError)?, "desc2");
    assert_eq!(org2.login, new_login);
    Ok(())
}

#[tokio::test]
async fn test_user_organization_link() -> Result<(), TestError> {
    let repo = setup_test_db()?;
    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login.clone(), None).await?;

    let username = format!("user_{}", Uuid::new_v4());
    let email = format!("{}@example.com", username);
    let user = repo
        .create_user(None, username.clone(), email.clone(), None)
        .await?;

    let link = repo
        .add_user_to_organization(org.id, user.id, "admin".to_string())
        .await?;
    assert_eq!(link.role, "admin");

    let role = repo
        .get_user_role(org.id, user.id)
        .await?
        .ok_or(TestError::NoneError)?;
    assert_eq!(role, "admin");

    // Test role update via conflict
    let link_update = repo
        .add_user_to_organization(org.id, user.id, "member".to_string())
        .await?;
    assert_eq!(link_update.role, "member");

    let role_update = repo
        .get_user_role(org.id, user.id)
        .await?
        .ok_or(TestError::NoneError)?;
    assert_eq!(role_update, "member");
    Ok(())
}

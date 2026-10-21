#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Integration tests for organization CRUD, upserts, and user role memberships.

use crate::db::repository::CddRepository;
use crate::db::tests::setup_test_db;
use crate::error::CddGatewayError;
use uuid::Uuid;

/// Verifies creating an organization and retrieving it by ID.
#[tokio::test]
async fn test_create_and_get_org() -> Result<(), CddGatewayError> {
    let repo = setup_test_db()?;

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo
        .create_organization(None, login.clone(), Some("Desc".to_string()))
        .await?;
    assert_eq!(org.login, login);
    assert_eq!(org.description, Some("Desc".to_string()));

    let found_org = repo
        .get_organization(org.id)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound("org not found".to_string()))?;
    assert_eq!(found_org.id, org.id);
    assert_eq!(found_org.login, login);
    Ok(())
}

/// Verifies inserting and updating an organization by its GitHub ID.
#[tokio::test]
async fn test_upsert_org() -> Result<(), CddGatewayError> {
    let repo = setup_test_db()?;

    let github_id =
        i64::try_from(Uuid::new_v4().as_u128() % 1_000_000_000).unwrap_or(0) + 2_000_000;
    let login = format!("org_{}", Uuid::new_v4());

    let org1 = repo
        .upsert_organization(github_id, login.clone(), None)
        .await?;
    assert_eq!(org1.github_id, Some(github_id));
    assert_eq!(org1.login, login);

    let new_login = format!("org_{}_new", Uuid::new_v4());
    let org2 = repo
        .upsert_organization(
            github_id,
            new_login.clone(),
            Some("Updated Desc".to_string()),
        )
        .await?;
    assert_eq!(org2.id, org1.id);
    assert_eq!(org2.login, new_login);
    assert_eq!(org2.description, Some("Updated Desc".to_string()));
    Ok(())
}

/// Verifies adding a user to an organization, querying their role, and updating the role.
#[tokio::test]
async fn test_org_user_roles() -> Result<(), CddGatewayError> {
    let repo = setup_test_db()?;

    let username = format!("user_{}", Uuid::new_v4());
    let email = format!("{username}@example.com");
    let user = repo.create_user(None, username, email, None).await?;

    let login = format!("org_{}", Uuid::new_v4());
    let org = repo.create_organization(None, login, None).await?;

    let role_none = repo.get_user_role(org.id, user.id).await?;
    assert!(role_none.is_none());

    let link = repo
        .add_user_to_organization(org.id, user.id, "owner".to_string())
        .await?;
    assert_eq!(link.role, "owner");

    let role_some = repo
        .get_user_role(org.id, user.id)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound("role not found".to_string()))?;
    assert_eq!(role_some, "owner");

    let link_update = repo
        .add_user_to_organization(org.id, user.id, "member".to_string())
        .await?;
    assert_eq!(link_update.role, "member");

    let role_update = repo
        .get_user_role(org.id, user.id)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound("role not found".to_string()))?;
    assert_eq!(role_update, "member");
    Ok(())
}

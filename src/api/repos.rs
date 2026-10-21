#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Repository creation endpoint with organization RBAC enforcement (`POST /repos`).

use crate::api::auth_middleware::AuthenticatedUser;
use crate::db::models::OrgRole;
use crate::db::repository::CddRepository;
use crate::error::CddGatewayError;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Request payload for creating a new repository within an organization.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateRepoPayload {
    /// Primary key ID of the organization that will own the repository.
    pub organization_id: i32,
    /// Name of the new repository.
    pub name: String,
    /// Optional human-readable description of the repository.
    pub description: Option<String>,
}

/// Creates a repository inside an organization (`POST /repos`).
///
/// Requires the authenticated caller to hold [`OrgRole::Owner`] in the organization.
///
/// # Arguments
/// * `payload` - JSON body containing `organization_id`, `name`, and `description`.
/// * `repo` - Database repository handle.
/// * `auth_user` - Authenticated caller identity.
///
/// # Returns
/// * `Ok(HttpResponse)` - `201 Created` with the created [`crate::db::models::Repository`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Forbidden`] - If the caller does not have the `owner` role in the organization.
/// * [`CddGatewayError`] - On database lookup or insertion failure.
#[utoipa::path(
    post,
    path = "/repos",
    request_body = CreateRepoPayload,
    responses(
        (status = 201, description = "Repository created successfully", body = crate::db::models::Repository),
        (status = 403, description = "Forbidden: Insufficient permissions"),
        (status = 500, description = "Internal Server Error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_repo(
    payload: web::Json<CreateRepoPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    auth_user: AuthenticatedUser,
) -> Result<HttpResponse, CddGatewayError> {
    let role_opt = repo
        .get_user_role(payload.organization_id, auth_user.user_id)
        .await?;

    let is_owner = role_opt
        .as_deref()
        .and_then(|r| r.parse::<OrgRole>().ok())
        .is_some_and(|role| role == OrgRole::Owner);

    if !is_owner {
        return Err(CddGatewayError::Forbidden(
            "Only organization owners can create repositories".to_string(),
        ));
    }

    let created_repo = repo
        .create_repository(
            payload.organization_id,
            None,
            payload.name.clone(),
            payload.description.clone(),
        )
        .await?;

    Ok(HttpResponse::Created().json(created_repo))
}

/// Registers `/repos` scope routes on `cfg`.
///
/// # Arguments
/// * `cfg` - Actix-web service configuration builder.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/repos").route("", web::post().to(create_repo)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::Repository;
    use crate::db::repository::MockCddRepository;
    use actix_web::{test, App};

    fn generate_test_token() -> String {
        crate::api::auth_middleware::generate_test_token()
    }

    #[actix_web::test]
    async fn test_create_repo_owner() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_get_user_role()
            .returning(|_, _| Ok(Some("owner".to_string())));
        mock_repo
            .expect_create_repository()
            .returning(|_, _, _, _| {
                Ok(Repository {
                    id: 1,
                    organization_id: 1,
                    github_id: None,
                    name: "new_repo".into(),
                    description: None,
                })
            });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/repos")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(CreateRepoPayload {
                organization_id: 1,
                name: "new_repo".to_string(),
                description: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::CREATED);
    }

    #[actix_web::test]
    async fn test_create_repo_forbidden() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_get_user_role()
            .returning(|_, _| Ok(Some("member".to_string())));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/repos")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(CreateRepoPayload {
                organization_id: 1,
                name: "new_repo".to_string(),
                description: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn test_create_repo_db_fail_and_role_fail() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_get_user_role()
            .times(1)
            .returning(|_, _| Ok(Some("owner".to_string())));
        mock_repo
            .expect_create_repository()
            .times(1)
            .returning(|_, _, _, _| {
                Err(CddGatewayError::Database(diesel::result::Error::NotFound))
            });
        mock_repo
            .expect_get_user_role()
            .times(1)
            .returning(|_, _| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let req1 = test::TestRequest::post()
            .uri("/repos")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(CreateRepoPayload {
                organization_id: 1,
                name: "new_repo".to_string(),
                description: None,
            })
            .to_request();
        assert_eq!(
            test::call_service(&app, req1).await.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        let req2 = test::TestRequest::post()
            .uri("/repos")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(CreateRepoPayload {
                organization_id: 1,
                name: "new_repo".to_string(),
                description: None,
            })
            .to_request();
        assert_eq!(
            test::call_service(&app, req2).await.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}

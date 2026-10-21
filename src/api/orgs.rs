#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Organization creation endpoint (`POST /orgs`).

use crate::api::auth_middleware::AuthenticatedUser;
use crate::db::models::OrgRole;
use crate::db::repository::CddRepository;
use crate::error::CddGatewayError;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Request payload for creating a new organization.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateOrgPayload {
    /// Unique organization handle (login).
    pub login: String,
    /// Optional human-readable description of the organization.
    pub description: Option<String>,
}

/// Creates an organization and assigns the caller as owner (`POST /orgs`).
///
/// # Arguments
/// * `payload` - JSON body containing the organization `login` and `description`.
/// * `repo` - Database repository handle.
/// * `auth_user` - Authenticated user creating the organization.
///
/// # Returns
/// * `Ok(HttpResponse)` - `201 Created` with the created [`crate::db::models::Organization`] JSON body.
///
/// # Errors
/// * [`CddGatewayError`] - If creating the organization or linking the owner role fails.
#[utoipa::path(
    post,
    path = "/orgs",
    request_body = CreateOrgPayload,
    responses(
        (status = 201, description = "Organization created successfully", body = crate::db::models::Organization),
        (status = 500, description = "Internal Server Error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_org(
    payload: web::Json<CreateOrgPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    auth_user: AuthenticatedUser,
) -> Result<HttpResponse, CddGatewayError> {
    let created_org = repo
        .create_organization(None, payload.login.clone(), payload.description.clone())
        .await?;

    repo.add_user_to_organization(
        created_org.id,
        auth_user.user_id,
        OrgRole::Owner.to_string(),
    )
    .await?;

    Ok(HttpResponse::Created().json(created_org))
}

/// Registers `/orgs` scope routes on `cfg`.
///
/// # Arguments
/// * `cfg` - Actix-web service configuration builder.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/orgs").route("", web::post().to(create_org)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{Organization, OrganizationUser};
    use crate::db::repository::MockCddRepository;
    use actix_web::{test, App};

    fn generate_test_token() -> String {
        crate::api::auth_middleware::generate_test_token()
    }

    #[actix_web::test]
    async fn test_create_org_success() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_create_organization().returning(|_, _, _| {
            Ok(Organization {
                id: 1,
                github_id: None,
                login: "new_org".into(),
                description: None,
            })
        });
        mock_repo
            .expect_add_user_to_organization()
            .returning(|_, _, _| {
                Ok(OrganizationUser {
                    organization_id: 1,
                    user_id: 1,
                    role: "owner".into(),
                })
            });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/orgs")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(CreateOrgPayload {
                login: "new_org".to_string(),
                description: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::CREATED);
    }

    #[actix_web::test]
    async fn test_create_org_fail_and_add_owner_fail() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_create_organization()
            .times(1)
            .returning(|_, _, _| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));
        mock_repo
            .expect_create_organization()
            .times(1)
            .returning(|_, _, _| {
                Ok(Organization {
                    id: 1,
                    github_id: None,
                    login: "new_org".into(),
                    description: None,
                })
            });
        mock_repo
            .expect_add_user_to_organization()
            .times(1)
            .returning(|_, _, _| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let req1 = test::TestRequest::post()
            .uri("/orgs")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(CreateOrgPayload {
                login: "new_org".to_string(),
                description: Some("desc".to_string()),
            })
            .to_request();
        assert_eq!(
            test::call_service(&app, req1).await.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        let req2 = test::TestRequest::post()
            .uri("/orgs")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .set_json(CreateOrgPayload {
                login: "new_org".to_string(),
                description: None,
            })
            .to_request();
        assert_eq!(
            test::call_service(&app, req2).await.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}

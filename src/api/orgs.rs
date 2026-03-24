#![cfg(not(tarpaulin_include))]


use crate::api::auth_middleware::AuthenticatedUser;
use crate::db::repository::CddRepository;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Payload for creating an organization
#[derive(Serialize, Deserialize, ToSchema)]
pub struct OrgPayload {
    /// Login/name for the organization
    pub login: String,
    /// Optional description
    pub description: Option<String>,
}

/// Create a new organization
#[utoipa::path(
    post,
    path = "/orgs",
    request_body = OrgPayload,
    responses(
        (status = 201, description = "Organization created successfully")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_org(
    payload: web::Json<OrgPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    auth_user: AuthenticatedUser,
) -> impl Responder {
    match repo
        .create_organization(None, payload.login.clone(), payload.description.clone())
        .await
    {
        Ok(created_org) => {
            // Give creator owner role
            let _ = repo
                .add_user_to_organization(created_org.id, auth_user.user_id, "owner".to_string())
                .await;
            HttpResponse::Created().json(created_org)
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

/// Configure organization routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/orgs").route("", web::post().to(create_org)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::Organization;
    use crate::db::repository::MockCddRepository;
    use actix_web::{test, App};

    #[actix_web::test]
    async fn test_create_org() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_create_organization().returning(|_, _, _| {
            Ok(Organization {
                id: 1,
                github_id: None,
                login: "testorg".to_string(),
                description: None,
            })
        });
        mock_repo
            .expect_add_user_to_organization()
            .returning(|_, _, _| {
                Ok(crate::db::models::OrganizationUser {
                    organization_id: 1,
                    user_id: 1,
                    role: "owner".to_string(),
                })
            });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let token = crate::api::auth_middleware::generate_test_token();

        let req = test::TestRequest::post()
            .uri("/orgs")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(OrgPayload {
                login: "testorg".to_string(),
                description: None,
            })
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}

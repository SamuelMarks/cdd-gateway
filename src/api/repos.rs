#![cfg(not(tarpaulin_include))]

use crate::api::auth_middleware::AuthenticatedUser;
use crate::db::repository::CddRepository;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Payload for creating a repository
#[derive(Serialize, Deserialize, ToSchema)]
pub struct RepoPayload {
    /// ID of the organization to own this repository
    pub organization_id: i32,
    /// Name of the repository
    pub name: String,
    /// Optional description
    pub description: Option<String>,
}

/// Create a new Repository (SDK project)
#[utoipa::path(
    post,
    path = "/repos",
    request_body = RepoPayload,
    responses(
        (status = 201, description = "Repository created successfully"),
        (status = 403, description = "Forbidden - Not an owner of the organization"),
        (status = 500, description = "Internal Server Error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_repo(
    payload: web::Json<RepoPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    auth_user: AuthenticatedUser,
) -> impl Responder {
    // RBAC: Check if user is an owner of the organization
    match repo
        .get_user_role(payload.organization_id, auth_user.user_id)
        .await
    {
        Ok(Some(role)) if role == "owner" => {
            match repo
                .create_repository(
                    payload.organization_id,
                    None,
                    payload.name.clone(),
                    payload.description.clone(),
                )
                .await
            {
                Ok(created_repo) => HttpResponse::Created().json(created_repo),
                Err(_) => HttpResponse::InternalServerError().finish(),
            }
        }
        Ok(_) => HttpResponse::Forbidden().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

/// Configure repository routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/repos").route("", web::post().to(create_repo)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::Repository;
    use crate::db::repository::MockCddRepository;
    use actix_web::{test, App};

    #[actix_web::test]
    async fn test_create_repo_success() {
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
                    name: "testrepo".to_string(),
                    description: None,
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
            .uri("/repos")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(RepoPayload {
                organization_id: 1,
                name: "testrepo".to_string(),
                description: None,
            })
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_create_repo_forbidden() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_get_user_role()
            .returning(|_, _| Ok(Some("member".to_string()))); // Not an owner

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .configure(configure),
        )
        .await;

        let token = crate::api::auth_middleware::generate_test_token();

        let req = test::TestRequest::post()
            .uri("/repos")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(RepoPayload {
                organization_id: 1,
                name: "testrepo".to_string(),
                description: None,
            })
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }
}

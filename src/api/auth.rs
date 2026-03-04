use crate::db::repository::CddRepository;
use crate::github::client::GitHubClient;
use actix_web::{web, HttpResponse, Responder};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Response containing the JWT token
#[derive(Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    /// JWT token
    pub token: String,
}

/// Payload for logging in with username/password
#[derive(Serialize, Deserialize, ToSchema)]
pub struct LoginPayload {
    /// Username
    pub username: String,
    /// Password
    pub password: Option<String>,
}

/// Payload for OAuth login
#[derive(Serialize, Deserialize, ToSchema)]
pub struct OAuthPayload {
    /// OAuth authorization code
    pub code: String,
}

/// Payload for registering a new user
#[derive(Serialize, Deserialize, ToSchema)]
pub struct RegisterPayload {
    /// Desired username
    pub username: String,
    /// Email address
    pub email: String,
    /// Password
    pub password: Option<String>,
}

fn generate_token(user_id: i32, username: &str) -> String {
    let claims = crate::api::auth_middleware::Claims {
        sub: user_id,
        exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
        username: username.to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(b"super-secret-key"),
    )
    .unwrap()
}

fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string()
}

fn verify_password(password: &str, hash: &str) -> bool {
    if let Ok(parsed_hash) = PasswordHash::new(hash) {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    } else {
        false
    }
}

/// Register a new user
#[utoipa::path(
    post,
    path = "/auth/register",
    request_body = RegisterPayload,
    responses(
        (status = 201, description = "Successfully registered", body = AuthResponse),
        (status = 400, description = "Bad Request")
    )
)]
pub async fn register(
    payload: web::Json<RegisterPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
) -> impl Responder {
    let hashed_pw = payload.password.as_ref().map(|pw| hash_password(pw));

    match repo
        .create_user(
            None,
            payload.username.clone(),
            payload.email.clone(),
            hashed_pw,
        )
        .await
    {
        Ok(user) => HttpResponse::Created().json(AuthResponse {
            token: generate_token(user.id, &user.username),
        }),
        Err(_) => HttpResponse::BadRequest().finish(),
    }
}

/// Login with username/password
#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginPayload,
    responses(
        (status = 200, description = "Successfully authenticated", body = AuthResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn login_password(
    payload: web::Json<LoginPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
) -> impl Responder {
    match repo.find_user_by_username(payload.username.clone()).await {
        Ok(Some(user)) => {
            if let Some(pw) = &payload.password {
                if let Some(h) = &user.password_hash {
                    if verify_password(pw, h) {
                        return HttpResponse::Ok().json(AuthResponse {
                            token: generate_token(user.id, &user.username),
                        });
                    }
                }
            }
            HttpResponse::Unauthorized().finish()
        }
        _ => HttpResponse::Unauthorized().finish(),
    }
}

/// Login with GitHub OAuth
#[utoipa::path(
    post,
    path = "/auth/github",
    request_body = OAuthPayload,
    responses(
        (status = 200, description = "Successfully authenticated via GitHub", body = AuthResponse),
        (status = 400, description = "OAuth failed"),
        (status = 500, description = "Internal Server Error")
    )
)]
pub async fn login_github(
    payload: web::Json<OAuthPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    github: web::Data<Arc<dyn GitHubClient>>,
) -> impl Responder {
    if payload.code.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    // Exchange code for access token
    let token = match github.exchange_code(&payload.code).await {
        Ok(t) => t,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    // Get user profile
    let gh_user = match github.get_user(&token).await {
        Ok(u) => u,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    // Get primary email
    let gh_emails = match github.get_emails(&token).await {
        Ok(e) => e,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let primary_email = gh_emails
        .into_iter()
        .find(|e| e.primary)
        .map(|e| e.email)
        .unwrap_or_else(|| "".to_string());

    // Upsert into local database
    match repo
        .upsert_user(gh_user.id, gh_user.login.clone(), primary_email)
        .await
    {
        Ok(user) => HttpResponse::Ok().json(AuthResponse {
            token: generate_token(user.id, &user.username),
        }),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

/// Configure auth routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/register", web::post().to(register))
            .route("/login", web::post().to(login_password))
            .route("/github", web::post().to(login_github)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::User;
    use crate::db::repository::MockCddRepository;
    use crate::github::client::MockGitHubClient;
    use crate::github::models::{GitHubEmail, GitHubUser};
    use actix_web::{test, App};

    #[actix_web::test]
    async fn test_register() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_create_user().returning(|_, _, _, _| {
            Ok(User {
                id: 1,
                github_id: None,
                username: "test".into(),
                email: "test@example.com".into(),
                password_hash: None,
            })
        });

        let mock_gh = MockGitHubClient::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/register")
            .set_json(RegisterPayload {
                username: "test".to_string(),
                email: "test@example.com".to_string(),
                password: Some("pwd".to_string()),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_login_password_fail() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_find_user_by_username()
            .returning(|_| Ok(None));

        let mock_gh = MockGitHubClient::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "notfound".to_string(),
                password: Some("pwd".to_string()),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_web::test]
    async fn test_login_github_success() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_upsert_user().returning(|_, _, _| {
            Ok(User {
                id: 1,
                github_id: Some(123),
                username: "gh_user".into(),
                email: "gh@example.com".into(),
                password_hash: None,
            })
        });

        let mut mock_gh = MockGitHubClient::new();
        mock_gh
            .expect_exchange_code()
            .returning(|_| Ok("fake_token".to_string()));
        mock_gh.expect_get_user().returning(|_| {
            Ok(GitHubUser {
                id: 123,
                login: "gh_user".to_string(),
                avatar_url: "".to_string(),
            })
        });
        mock_gh.expect_get_emails().returning(|_| {
            Ok(vec![GitHubEmail {
                email: "gh@example.com".to_string(),
                primary: true,
                verified: true,
            }])
        });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "code123".to_string(),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}

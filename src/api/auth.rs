use crate::config::AppConfig;
use crate::db::repository::CddRepository;
use crate::github::client::GitHubClient;
use actix_web::{web, HttpResponse};
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

/// Generate a signed JWT for the given user, using the secret from `AppConfig`.
fn generate_token(user_id: i32, username: &str, jwt_secret: &[u8]) -> String {
    let claims = crate::api::auth_middleware::Claims {
        sub: user_id,
        exp: (chrono::Utc::now() + chrono::Duration::hours(24))
            .timestamp()
            .try_into()
            .unwrap_or(0),
        username: username.to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret),
    )
    .unwrap_or_else(|_| unreachable!("JWT encode cannot fail"))
}

/// Hash a password using Argon2
fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .unwrap_or_else(|_| unreachable!("Hashing cannot fail"));
    hash.to_string()
}

/// Verify a password against an Argon2 hash
fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed_hash| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    })
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
/// # Errors
/// Returns an error on database failure
/// # Panics
/// panics
pub async fn register(
    payload: web::Json<RegisterPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    cfg: web::Data<AppConfig>,
) -> Result<HttpResponse, crate::error::CddGatewayError> {
    let hashed_pw = payload.password.as_ref().map(|pw| hash_password(pw));

    let user = repo
        .create_user(
            None,
            payload.username.clone(),
            payload.email.clone(),
            hashed_pw,
        )
        .await?;

    let token = generate_token(user.id, &user.username, cfg.jwt_secret.as_bytes());

    Ok(HttpResponse::Created().json(AuthResponse { token }))
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
/// # Errors
/// Returns an error on database failure
/// # Panics
/// panics
pub async fn login_password(
    payload: web::Json<LoginPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    cfg: web::Data<AppConfig>,
) -> Result<HttpResponse, crate::error::CddGatewayError> {
    match repo.find_user_by_username(payload.username.clone()).await {
        Ok(Some(user)) => {
            if let Some(pw) = &payload.password {
                if let Some(h) = &user.password_hash {
                    if verify_password(pw, h) {
                        let token =
                            generate_token(user.id, &user.username, cfg.jwt_secret.as_bytes());
                        return Ok(HttpResponse::Ok().json(AuthResponse { token }));
                    }
                }
            }
            Ok(HttpResponse::Unauthorized().finish())
        }
        _ => Ok(HttpResponse::Unauthorized().finish()),
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
/// # Errors
/// Returns an error on database failure
/// # Panics
/// panics
pub async fn login_github(
    payload: web::Json<OAuthPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    github: web::Data<Arc<dyn GitHubClient>>,
    cfg: web::Data<AppConfig>,
) -> Result<HttpResponse, crate::error::CddGatewayError> {
    if payload.code.is_empty() {
        return Ok(HttpResponse::BadRequest().finish());
    }

    // Exchange code for access token
    let Ok(token) = github.exchange_code(&payload.code).await else {
        return Ok(HttpResponse::BadRequest().finish());
    };

    // Get user profile
    let Ok(gh_user) = github.get_user(&token).await else {
        return Ok(HttpResponse::InternalServerError().finish());
    };

    // Get primary email
    let Ok(gh_emails) = github.get_emails(&token).await else {
        return Ok(HttpResponse::InternalServerError().finish());
    };

    let primary_email = gh_emails
        .into_iter()
        .find(|e| e.primary)
        .map_or_else(String::new, |e| e.email);

    // Upsert into local database
    match repo
        .upsert_user(gh_user.id, gh_user.login.clone(), primary_email)
        .await
    {
        Ok(user) => {
            let token = generate_token(user.id, &user.username, cfg.jwt_secret.as_bytes());
            Ok(HttpResponse::Ok().json(AuthResponse { token }))
        }
        Err(_) => Ok(HttpResponse::InternalServerError().finish()),
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
    use actix_web::{test, App};

    fn test_config() -> AppConfig {
        AppConfig::load(None).unwrap_or_else(|_| panic!("expected value"))
    }

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
                .app_data(web::Data::new(test_config()))
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
                .app_data(web::Data::new(test_config()))
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
            Ok(crate::github::models::GitHubUser {
                id: 123,
                login: "gh_user".to_string(),
                avatar_url: String::new(),
            })
        });
        mock_gh.expect_get_emails().returning(|_| {
            Ok(vec![crate::github::models::GitHubEmail {
                email: "gh@example.com".to_string(),
                primary: true,
                verified: true,
            }])
        });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
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

    #[actix_web::test]
    async fn test_register_fail() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_create_user()
            .returning(|_, _, _, _| Err(diesel::result::Error::NotFound));

        let mock_gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
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
        assert_eq!(resp.status(), 500);
    }

    #[actix_web::test]
    async fn test_login_password_success() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_find_user_by_username().returning(|_| {
            Ok(Some(User {
                id: 1,
                github_id: None,
                username: "test".into(),
                email: "test@example.com".into(),
                password_hash: Some(hash_password("pwd")),
            }))
        });

        let mock_gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "test".to_string(),
                password: Some("pwd".to_string()),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_login_password_no_password() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_find_user_by_username().returning(|_| {
            Ok(Some(User {
                id: 1,
                github_id: None,
                username: "test".into(),
                email: "test@example.com".into(),
                password_hash: Some(hash_password("pwd")),
            }))
        });

        let mock_gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "test".to_string(),
                password: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_web::test]
    async fn test_login_password_wrong_password() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_find_user_by_username().returning(|_| {
            Ok(Some(User {
                id: 1,
                github_id: None,
                username: "test".into(),
                email: "test@example.com".into(),
                password_hash: Some(hash_password("pwd2")),
            }))
        });

        let mock_gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "test".to_string(),
                password: Some("pwd".to_string()),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_web::test]
    async fn test_login_github_empty_code() {
        let mock_repo = MockCddRepository::new();
        let mock_gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: String::new(),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 400);
    }

    #[actix_web::test]
    async fn test_login_github_exchange_fail() {
        let mock_repo = MockCddRepository::new();
        let mut mock_gh = MockGitHubClient::new();
        mock_gh
            .expect_exchange_code()
            .returning(|_| Err("err".to_string()));
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload { code: "abc".into() })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 400);
    }

    #[actix_web::test]
    async fn test_login_github_get_user_fail_but_emails_succeed() {
        let mock_repo = MockCddRepository::new();

        let mut mock_gh = MockGitHubClient::new();
        mock_gh
            .expect_exchange_code()
            .returning(|_| Ok("token".to_string()));
        mock_gh
            .expect_get_user()
            .returning(|_| Err("failed".to_string()));

        let config = AppConfig::load(None).unwrap_or_else(|_| panic!("expected value"));
        let repo: Arc<dyn CddRepository> = Arc::new(mock_repo);
        let gh: Arc<dyn GitHubClient> = Arc::new(mock_gh);

        let req = web::Json(OAuthPayload {
            code: "valid_code".to_string(),
        });
        let resp = login_github(
            req,
            web::Data::new(repo),
            web::Data::new(gh),
            web::Data::new(config),
        )
        .await;

        assert!(resp.is_ok());
    }

    #[actix_web::test]
    async fn test_register_no_password() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_create_user().returning(|_, _, _, _| {
            Ok(crate::db::models::User {
                id: 1,
                github_id: None,
                username: "test_user".to_string(),
                email: "test@example.com".to_string(),
                password_hash: None,
            })
        });

        let config = AppConfig::load(None).unwrap_or_else(|_| panic!("expected value"));
        let repo: Arc<dyn CddRepository> = Arc::new(mock_repo);

        let req = web::Json(RegisterPayload {
            username: "test_user".to_string(),
            email: "test@example.com".to_string(),
            password: None,
        });

        let resp = register(req, web::Data::new(repo), web::Data::new(config)).await;
        assert!(resp.is_ok());
    }

    #[actix_web::test]
    async fn test_login_github_get_user_fail() {
        let mock_repo = MockCddRepository::new();
        let mut mock_gh = MockGitHubClient::new();
        mock_gh
            .expect_exchange_code()
            .returning(|_| Ok("token".into()));
        mock_gh
            .expect_get_user()
            .returning(|_| Err("err".to_string()));
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload { code: "abc".into() })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 500);
    }

    #[actix_web::test]
    async fn test_login_github_get_emails_fail() {
        let mock_repo = MockCddRepository::new();
        let mut mock_gh = MockGitHubClient::new();
        mock_gh
            .expect_exchange_code()
            .returning(|_| Ok("token".into()));
        mock_gh.expect_get_user().returning(|_| {
            Ok(crate::github::models::GitHubUser {
                id: 1,
                login: "l".into(),
                avatar_url: "u".into(),
            })
        });
        mock_gh
            .expect_get_emails()
            .returning(|_| Err("err".to_string()));
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload { code: "abc".into() })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 500);
    }

    #[actix_web::test]
    async fn test_login_github_upsert_fail() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_upsert_user()
            .returning(|_, _, _| Err(diesel::result::Error::NotFound));
        let mut mock_gh = MockGitHubClient::new();
        mock_gh
            .expect_exchange_code()
            .returning(|_| Ok("token".into()));
        mock_gh.expect_get_user().returning(|_| {
            Ok(crate::github::models::GitHubUser {
                id: 1,
                login: "l".into(),
                avatar_url: "u".into(),
            })
        });
        mock_gh.expect_get_emails().returning(|_| Ok(vec![]));
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload { code: "abc".into() })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 500);
    }

    #[actix_web::test]
    async fn test_login_password_no_hash() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_find_user_by_username().returning(|_| {
            Ok(Some(crate::db::models::User {
                id: 1,
                github_id: Some(123),
                username: "test".into(),
                email: "test@example.com".into(),
                password_hash: None,
            }))
        });

        let mock_gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(
                    std::sync::Arc::new(mock_repo) as std::sync::Arc<dyn CddRepository>
                ))
                .app_data(web::Data::new(
                    std::sync::Arc::new(mock_gh) as std::sync::Arc<dyn GitHubClient>
                ))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "test".into(),
                password: Some("pwd".into()),
            })
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_login_password_verify_password_fail() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_find_user_by_username().returning(|_| {
            Ok(Some(User {
                id: 1,
                github_id: None,
                username: "test".into(),
                email: "test@example.com".into(),
                password_hash: Some("invalid_hash".into()),
            }))
        });

        let mock_gh = MockGitHubClient::new();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "test".to_string(),
                password: Some("pwd".to_string()),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }
}

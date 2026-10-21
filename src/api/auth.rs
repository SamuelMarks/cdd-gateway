#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! User registration, password login, and GitHub OAuth authentication endpoints.

use crate::config::AppConfig;
use crate::db::repository::CddRepository;
use crate::error::CddGatewayError;
use crate::github::client::GitHubClient;
use actix_web::{web, HttpResponse};
use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Response containing a signed JWT bearer token.
#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct AuthResponse {
    /// Signed JWT bearer token.
    pub token: String,
}

/// Request payload for logging in with a username and password.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginPayload {
    /// Account login username.
    pub username: String,
    /// Optional plaintext password to verify against the stored Argon2 hash.
    pub password: Option<String>,
}

/// Request payload for completing a GitHub OAuth login flow.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OAuthPayload {
    /// Temporary OAuth authorization code received from GitHub.
    pub code: String,
}

/// Request payload for registering a new user account.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterPayload {
    /// Desired unique login username.
    pub username: String,
    /// Primary email address for the account.
    pub email: String,
    /// Optional plaintext password to hash with Argon2.
    pub password: Option<String>,
}

/// Generates a signed 24-hour JWT for the given user using the secret from [`AppConfig`].
///
/// # Arguments
/// * `user_id` - Numeric primary key ID of the user.
/// * `username` - Account login username.
/// * `jwt_secret` - HMAC secret key bytes used to sign the token.
///
/// # Returns
/// * `Ok(String)` - Encoded JWT string.
///
/// # Errors
/// * [`CddGatewayError::Jwt`] - If JWT encoding fails.
fn generate_token(
    user_id: i32,
    username: &str,
    jwt_secret: &[u8],
) -> Result<String, CddGatewayError> {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let claims = crate::api::auth_middleware::Claims {
        sub: user_id,
        exp: usize::try_from(now_secs.saturating_add(86_400)).unwrap_or(0),
        username: username.to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret),
    )
    .map_err(CddGatewayError::Jwt)
}

/// Hashes a plaintext password using Argon2id with a random OS salt.
///
/// # Arguments
/// * `password` - Plaintext password slice to hash.
///
/// # Returns
/// * `Ok(String)` - PHC-formatted Argon2 password hash string.
///
/// # Errors
/// * [`CddGatewayError::Hash`] - If Argon2 hashing fails.
fn hash_password(password: &str) -> Result<String, CddGatewayError> {
    let argon2 = Argon2::default();
    let hash: PasswordHash = argon2
        .hash_password(password.as_bytes())
        .map_err(CddGatewayError::Hash)?;
    Ok(hash.to_string())
}

/// Verifies a plaintext password against a PHC-formatted Argon2 hash string.
///
/// # Arguments
/// * `password` - Plaintext password provided by the caller.
/// * `hash` - Stored Argon2 PHC hash string.
///
/// # Returns
/// * `bool` - `true` if `password` matches `hash`, `false` otherwise.
fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed_hash| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    })
}

/// Registers a new user account and returns a signed JWT (`POST /auth/register`).
///
/// # Arguments
/// * `payload` - JSON body containing `username`, `email`, and optional `password`.
/// * `repo` - Database repository handle.
/// * `cfg` - Application configuration containing the JWT signing secret.
///
/// # Returns
/// * `Ok(HttpResponse)` - `201 Created` with [`AuthResponse`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `username` or `email` is empty or invalid.
/// * [`CddGatewayError`] - On password hashing, database, or JWT encoding failure.
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
    cfg: web::Data<AppConfig>,
) -> Result<HttpResponse, CddGatewayError> {
    if payload.username.trim().is_empty() || !payload.email.contains('@') {
        return Err(CddGatewayError::Validation(
            "Valid username and email address are required".to_string(),
        ));
    }

    let hashed_pw = match &payload.password {
        Some(pw) => Some(hash_password(pw)?),
        None => None,
    };

    let user = repo
        .create_user(
            None,
            payload.username.clone(),
            payload.email.clone(),
            hashed_pw,
        )
        .await?;

    let token = generate_token(user.id, &user.username, cfg.jwt_secret.as_bytes())?;

    Ok(HttpResponse::Created().json(AuthResponse { token }))
}

/// Authenticates a user with username and password (`POST /auth/login`).
///
/// # Arguments
/// * `payload` - JSON body containing `username` and `password`.
/// * `repo` - Database repository handle.
/// * `cfg` - Application configuration containing the JWT signing secret.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with [`AuthResponse`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Unauthorized`] - If the user does not exist or the password does not match.
/// * [`CddGatewayError`] - On database or JWT encoding failure.
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
    cfg: web::Data<AppConfig>,
) -> Result<HttpResponse, CddGatewayError> {
    let user = repo
        .find_user_by_username(payload.username.clone())
        .await?
        .ok_or_else(|| CddGatewayError::Unauthorized("Invalid credentials".to_string()))?;

    let pw = payload
        .password
        .as_deref()
        .ok_or_else(|| CddGatewayError::Unauthorized("Password is required".to_string()))?;

    let hash = user.password_hash.as_deref().ok_or_else(|| {
        CddGatewayError::Unauthorized("Password login not configured".to_string())
    })?;

    if !verify_password(pw, hash) {
        return Err(CddGatewayError::Unauthorized(
            "Invalid credentials".to_string(),
        ));
    }

    let token = generate_token(user.id, &user.username, cfg.jwt_secret.as_bytes())?;
    Ok(HttpResponse::Ok().json(AuthResponse { token }))
}

/// Authenticates a user via GitHub OAuth code exchange (`POST /auth/github`).
///
/// # Arguments
/// * `payload` - JSON body containing the GitHub OAuth `code`.
/// * `repo` - Database repository handle.
/// * `github` - GitHub API client handle.
/// * `cfg` - Application configuration containing the JWT signing secret.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with [`AuthResponse`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `payload.code` is empty or OAuth exchange fails.
/// * [`CddGatewayError`] - On GitHub API, database upsert, or JWT encoding failure.
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
    cfg: web::Data<AppConfig>,
) -> Result<HttpResponse, CddGatewayError> {
    if payload.code.trim().is_empty() {
        return Err(CddGatewayError::Validation(
            "OAuth authorization code cannot be empty".to_string(),
        ));
    }

    let token = github
        .exchange_code(&payload.code)
        .await
        .map_err(|e| CddGatewayError::Validation(format!("OAuth exchange failed: {e}")))?;

    let gh_user = github
        .get_user(&token)
        .await
        .map_err(|e| CddGatewayError::Internal(format!("GitHub get_user failed: {e}")))?;

    let gh_emails = github
        .get_emails(&token)
        .await
        .map_err(|e| CddGatewayError::Internal(format!("GitHub get_emails failed: {e}")))?;

    let primary_email = gh_emails
        .into_iter()
        .find(|e| e.primary || e.verified)
        .map_or_else(
            || format!("{}@users.noreply.github.com", gh_user.login),
            |e| e.email,
        );

    let user = repo
        .upsert_user(gh_user.id, gh_user.login, primary_email)
        .await?;

    let jwt = generate_token(user.id, &user.username, cfg.jwt_secret.as_bytes())?;
    Ok(HttpResponse::Ok().json(AuthResponse { token: jwt }))
}

/// Registers `/auth` scope routes (`/register`, `/login`, `/github`) on `cfg`.
///
/// # Arguments
/// * `cfg` - Actix-web service configuration builder.
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
        AppConfig::default()
    }

    #[actix_web::test]
    async fn test_register_success_and_validation() {
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
        let resp: AuthResponse = test::call_and_read_body_json(&app, req).await;
        assert_ne!(resp.token, "");

        let req_bad = test::TestRequest::post()
            .uri("/auth/register")
            .set_json(RegisterPayload {
                username: String::new(),
                email: "invalid-email".to_string(),
                password: None,
            })
            .to_request();
        let resp_bad = test::call_service(&app, req_bad).await;
        assert_eq!(resp_bad.status(), actix_web::http::StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_register_no_password_and_fail() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_create_user()
            .times(1)
            .returning(|_, _, _, _| {
                Ok(User {
                    id: 1,
                    github_id: None,
                    username: "test_user".to_string(),
                    email: "test@example.com".to_string(),
                    password_hash: None,
                })
            });
        mock_repo
            .expect_create_user()
            .times(1)
            .returning(|_, _, _, _| {
                Err(CddGatewayError::Database(diesel::result::Error::NotFound))
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

        let req_ok = test::TestRequest::post()
            .uri("/auth/register")
            .set_json(RegisterPayload {
                username: "test_user".to_string(),
                email: "test@example.com".to_string(),
                password: None,
            })
            .to_request();
        let resp_ok = test::call_service(&app, req_ok).await;
        assert_eq!(resp_ok.status(), actix_web::http::StatusCode::CREATED);

        let req_err = test::TestRequest::post()
            .uri("/auth/register")
            .set_json(RegisterPayload {
                username: "test_user".to_string(),
                email: "test@example.com".to_string(),
                password: Some("pwd".to_string()),
            })
            .to_request();
        let resp_err = test::call_service(&app, req_err).await;
        assert_eq!(
            resp_err.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[actix_web::test]
    async fn test_login_password_branches() -> Result<(), CddGatewayError> {
        let valid_hash = hash_password("pwd")?;
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_find_user_by_username()
            .withf(|u| u == "notfound")
            .returning(|_| Ok(None));
        mock_repo
            .expect_find_user_by_username()
            .withf(|u| u == "nohash")
            .returning(|_| {
                Ok(Some(User {
                    id: 2,
                    github_id: Some(123),
                    username: "nohash".into(),
                    email: "nohash@example.com".into(),
                    password_hash: None,
                }))
            });
        mock_repo
            .expect_find_user_by_username()
            .withf(|u| u == "badhash")
            .returning(|_| {
                Ok(Some(User {
                    id: 3,
                    github_id: None,
                    username: "badhash".into(),
                    email: "badhash@example.com".into(),
                    password_hash: Some("invalid_hash".into()),
                }))
            });
        mock_repo
            .expect_find_user_by_username()
            .withf(|u| u == "test")
            .returning(move |_| {
                Ok(Some(User {
                    id: 1,
                    github_id: None,
                    username: "test".into(),
                    email: "test@example.com".into(),
                    password_hash: Some(valid_hash.clone()),
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

        // 1. User not found
        let req1 = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "notfound".to_string(),
                password: Some("pwd".to_string()),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req1).await.status(), 401);

        // 2. Password missing
        let req2 = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "test".to_string(),
                password: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req2).await.status(), 401);

        // 3. Stored hash missing
        let req3 = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "nohash".to_string(),
                password: Some("pwd".to_string()),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req3).await.status(), 401);

        // 4. Stored hash malformed
        let req4 = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "badhash".to_string(),
                password: Some("pwd".to_string()),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req4).await.status(), 401);

        // 5. Wrong password
        let req5 = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "test".to_string(),
                password: Some("wrong".to_string()),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req5).await.status(), 401);

        // 6. Correct password
        let req6 = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "test".to_string(),
                password: Some("pwd".to_string()),
            })
            .to_request();
        let resp6: AuthResponse = test::call_and_read_body_json(&app, req6).await;
        assert_ne!(resp6.token, "");

        Ok(())
    }

    #[actix_web::test]
    async fn test_login_github_branches() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_upsert_user()
            .times(1)
            .returning(|_, _, email| {
                assert_eq!(email, "gh@example.com");
                Ok(User {
                    id: 1,
                    github_id: Some(123),
                    username: "gh_user".into(),
                    email,
                    password_hash: None,
                })
            });
        mock_repo
            .expect_upsert_user()
            .times(1)
            .returning(|_, _, _| Err(CddGatewayError::Database(diesel::result::Error::NotFound)));

        let mut mock_gh = MockGitHubClient::new();
        mock_gh
            .expect_exchange_code()
            .withf(|c| c == "bad_code")
            .returning(|_| Err(CddGatewayError::Unauthorized("bad code".into())));
        mock_gh
            .expect_exchange_code()
            .withf(|c| c == "user_err")
            .returning(|_| Ok("tok_user_err".to_string()));
        mock_gh
            .expect_exchange_code()
            .withf(|c| c == "email_err")
            .returning(|_| Ok("tok_email_err".to_string()));
        mock_gh
            .expect_exchange_code()
            .withf(|c| c == "ok_code" || c == "db_err")
            .returning(|_| Ok("tok_ok".to_string()));

        mock_gh
            .expect_get_user()
            .withf(|t| t == "tok_user_err")
            .returning(|_| Err(CddGatewayError::Internal("user fail".into())));
        mock_gh.expect_get_user().returning(|_| {
            Ok(crate::github::models::GitHubUser {
                id: 123,
                login: "gh_user".to_string(),
                avatar_url: String::new(),
            })
        });

        mock_gh
            .expect_get_emails()
            .withf(|t| t == "tok_email_err")
            .returning(|_| Err(CddGatewayError::Internal("email fail".into())));
        mock_gh.expect_get_emails().times(1).returning(|_| {
            Ok(vec![crate::github::models::GitHubEmail {
                email: "gh@example.com".to_string(),
                primary: true,
                verified: true,
            }])
        });
        mock_gh
            .expect_get_emails()
            .times(1)
            .returning(|_| Ok(vec![]));

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        // 1. Empty code -> 400
        let req_empty = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: String::new(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_empty).await.status(), 400);

        // 2. Exchange fail -> 400
        let req_ex = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "bad_code".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_ex).await.status(), 400);

        // 3. get_user fail -> 500
        let req_usr = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "user_err".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_usr).await.status(), 500);

        // 4. get_emails fail -> 500
        let req_em = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "email_err".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_em).await.status(), 500);

        // 5. Success -> 200
        let req_ok = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "ok_code".into(),
            })
            .to_request();
        let resp_ok: AuthResponse = test::call_and_read_body_json(&app, req_ok).await;
        assert_ne!(resp_ok.token, "");

        // 6. Upsert fail -> 500
        let req_db = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "db_err".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_db).await.status(), 500);
    }
}

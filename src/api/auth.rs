#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! User registration, password login, GitHub OAuth, Google OAuth, and account linking endpoints.

use crate::api::auth_middleware::AuthenticatedUser;
use crate::config::AppConfig;
use crate::db::models::User;
use crate::db::repository::CddRepository;
use crate::error::CddGatewayError;
use crate::github::client::GitHubClient;
use actix_web::{web, HttpResponse};
use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use reqwest::Client;
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

/// Request payload for completing a GitHub OAuth login or account linking flow.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OAuthPayload {
    /// Temporary OAuth authorization code received from GitHub.
    pub code: String,
}

/// Request payload for completing a Google OAuth 2.0 login or account linking flow.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GoogleOAuthPayload {
    /// Temporary OAuth 2.0 authorization code received from Google.
    pub code: String,
    /// Optional redirect URI used when requesting the authorization code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
}

/// Request payload for setting or updating a password on an authenticated account.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConnectPasswordPayload {
    /// New plaintext password to hash with Argon2id.
    pub password: String,
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

/// Authenticated user profile response describing identity and linked authentication providers.
#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct UserProfileResponse {
    /// Numeric primary key ID of the user.
    pub id: i32,
    /// Unique login username.
    pub username: String,
    /// Primary email address.
    pub email: String,
    /// Linked GitHub numeric user ID, if connected.
    pub github_id: Option<i64>,
    /// Linked Google OAuth subject ID (`sub`), if connected.
    pub google_id: Option<String>,
    /// Whether a password hash is configured on the account.
    pub has_password: bool,
    /// List of active authentication providers (`"github"`, `"google"`, `"password"`).
    pub connected_providers: Vec<String>,
}

impl From<User> for UserProfileResponse {
    /// Converts a database [`User`] into a [`UserProfileResponse`].
    ///
    /// # Arguments
    /// * `user` - The database user model.
    ///
    /// # Returns
    /// * [`UserProfileResponse`] - Public user profile with connected provider metadata.
    fn from(user: User) -> Self {
        let mut connected_providers = Vec::new();
        if user.github_id.is_some() {
            connected_providers.push("github".to_string());
        }
        if user.google_id.is_some() {
            connected_providers.push("google".to_string());
        }
        let has_password = user.password_hash.is_some();
        if has_password {
            connected_providers.push("password".to_string());
        }
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            github_id: user.github_id,
            google_id: user.google_id,
            has_password,
            connected_providers,
        }
    }
}

/// Request body sent to the Google OAuth 2.0 token exchange endpoint.
#[derive(Serialize)]
struct GoogleTokenRequest<'a> {
    /// Authorization code received from Google.
    code: &'a str,
    /// Google OAuth 2.0 client ID.
    client_id: &'a str,
    /// Google OAuth 2.0 client secret.
    client_secret: &'a str,
    /// Redirect URI matching the initial authorization request.
    redirect_uri: &'a str,
    /// OAuth 2.0 grant type (`"authorization_code"`).
    grant_type: &'a str,
}

/// Response body returned by the Google OAuth 2.0 token exchange endpoint.
#[derive(Deserialize)]
struct GoogleTokenResponse {
    /// Issued OAuth 2.0 bearer access token, if exchange succeeded.
    access_token: Option<String>,
    /// Error code returned by Google, if exchange failed.
    error: Option<String>,
    /// Detailed error description returned by Google, if exchange failed.
    error_description: Option<String>,
}

/// Response body returned by the Google OAuth 2.0 `UserInfo` endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleUserInfo {
    /// Unique Google account subject identifier (`sub`).
    pub sub: String,
    /// Primary email address associated with the Google account.
    pub email: String,
    /// Whether Google has verified ownership of `email`.
    #[serde(default)]
    pub email_verified: bool,
    /// Optional display name of the user.
    #[serde(default)]
    pub name: Option<String>,
}

/// Exchanges a Google OAuth 2.0 authorization code for a [`GoogleUserInfo`] profile.
///
/// # Arguments
/// * `http` - Shared `reqwest::Client` instance.
/// * `cfg` - Application configuration containing Google OAuth credentials and URLs.
/// * `payload` - Google OAuth payload containing `code` and optional `redirect_uri`.
///
/// # Returns
/// * `Ok(GoogleUserInfo)` - Verified Google user profile.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `code` is empty, token exchange fails, or email is unverified.
/// * [`CddGatewayError::Http`] - If an HTTP request to Google fails.
async fn fetch_google_user_info(
    http: &Client,
    cfg: &AppConfig,
    payload: &GoogleOAuthPayload,
) -> Result<GoogleUserInfo, CddGatewayError> {
    if payload.code.trim().is_empty() {
        return Err(CddGatewayError::Validation(
            "Google OAuth authorization code cannot be empty".to_string(),
        ));
    }

    let redirect_uri = payload.redirect_uri.as_deref().unwrap_or("postmessage");
    let token_req = GoogleTokenRequest {
        code: &payload.code,
        client_id: &cfg.google_client_id,
        client_secret: &cfg.google_client_secret,
        redirect_uri,
        grant_type: "authorization_code",
    };

    let token_resp = http
        .post(&cfg.google_token_url)
        .json(&token_req)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| CddGatewayError::Validation(format!("Google token exchange failed: {e}")))?;

    let parsed_token: GoogleTokenResponse = token_resp.json().await?;
    let Some(access_token) = parsed_token.access_token else {
        let reason = parsed_token
            .error_description
            .or(parsed_token.error)
            .unwrap_or_else(|| "Unknown Google OAuth error".to_string());
        return Err(CddGatewayError::Validation(format!(
            "Google token exchange rejected: {reason}"
        )));
    };

    let info: GoogleUserInfo = http
        .get(&cfg.google_userinfo_url)
        .bearer_auth(&access_token)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| CddGatewayError::Internal(format!("Google userinfo request failed: {e}")))?
        .json()
        .await?;

    if !info.email_verified {
        return Err(CddGatewayError::Validation(
            "Google account email address is not verified".to_string(),
        ));
    }

    Ok(info)
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
    let mut found_user = repo.find_user_by_username(payload.username.clone()).await?;
    if found_user.is_none() {
        if let Some((prefix, _)) = payload.username.split_once('@') {
            found_user = repo.find_user_by_username(prefix.to_string()).await?;
        }
    }
    let user = found_user
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
/// If a verified GitHub email matches an existing account without a `github_id`,
/// links the `github_id` to that account automatically.
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

    let verified_entry = gh_emails.into_iter().find(|e| e.primary || e.verified);
    let is_email_verified = verified_entry.as_ref().is_some_and(|e| e.verified);
    let primary_email = verified_entry.map_or_else(
        || format!("{}@users.noreply.github.com", gh_user.login),
        |e| e.email,
    );

    let user = if is_email_verified {
        if let Some(existing) = repo.find_user_by_email(primary_email.clone()).await? {
            if existing.github_id.is_none() {
                repo.link_github_id(existing.id, gh_user.id).await?
            } else {
                repo.upsert_user(gh_user.id, gh_user.login, primary_email)
                    .await?
            }
        } else {
            repo.upsert_user(gh_user.id, gh_user.login, primary_email)
                .await?
        }
    } else {
        repo.upsert_user(gh_user.id, gh_user.login, primary_email)
            .await?
    };

    let jwt = generate_token(user.id, &user.username, cfg.jwt_secret.as_bytes())?;
    Ok(HttpResponse::Ok().json(AuthResponse { token: jwt }))
}

/// Authenticates a user via Google OAuth 2.0 code exchange (`POST /auth/google`).
///
/// Automatically links `google_id` to an existing user if the verified Google email matches.
///
/// # Arguments
/// * `payload` - JSON body containing the Google OAuth `code` and optional `redirect_uri`.
/// * `repo` - Database repository handle.
/// * `cfg` - Application configuration containing Google OAuth settings and JWT secret.
/// * `http` - Optional shared `reqwest::Client`.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with [`AuthResponse`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If code exchange fails or the Google email is unverified.
/// * [`CddGatewayError`] - On HTTP, database, or JWT encoding failure.
#[utoipa::path(
    post,
    path = "/auth/google",
    request_body = GoogleOAuthPayload,
    responses(
        (status = 200, description = "Successfully authenticated via Google", body = AuthResponse),
        (status = 400, description = "Google OAuth failed"),
        (status = 500, description = "Internal Server Error")
    )
)]
pub async fn login_google(
    payload: web::Json<GoogleOAuthPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    cfg: web::Data<AppConfig>,
    http: Option<web::Data<Client>>,
) -> Result<HttpResponse, CddGatewayError> {
    let default_client = Client::new();
    let client_ref: &Client = http.as_ref().map_or(&default_client, |c| c.get_ref());

    let info = fetch_google_user_info(client_ref, &cfg, &payload).await?;

    let derived_username = info
        .email
        .split_once('@')
        .map_or_else(|| info.email.clone(), |(prefix, _)| prefix.to_string());

    let user = if let Some(by_google) = repo.find_user_by_google_id(info.sub.clone()).await? {
        by_google
    } else if let Some(by_email) = repo.find_user_by_email(info.email.clone()).await? {
        repo.link_google_id(by_email.id, info.sub).await?
    } else {
        repo.upsert_google_user(info.sub, derived_username, info.email)
            .await?
    };

    let jwt = generate_token(user.id, &user.username, cfg.jwt_secret.as_bytes())?;
    Ok(HttpResponse::Ok().json(AuthResponse { token: jwt }))
}

/// Returns the authenticated caller's user profile and linked providers (`GET /auth/me`).
///
/// # Arguments
/// * `auth` - Authenticated caller identity.
/// * `repo` - Database repository handle.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with [`UserProfileResponse`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::NotFound`] - If the user record no longer exists.
/// * [`CddGatewayError`] - On database failure.
#[utoipa::path(
    get,
    path = "/auth/me",
    responses(
        (status = 200, description = "Current user profile", body = UserProfileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "User not found")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_me(
    auth: AuthenticatedUser,
    repo: web::Data<Arc<dyn CddRepository>>,
) -> Result<HttpResponse, CddGatewayError> {
    let user = repo
        .find_user_by_id(auth.user_id)
        .await?
        .ok_or_else(|| CddGatewayError::NotFound("User not found".to_string()))?;
    Ok(HttpResponse::Ok().json(UserProfileResponse::from(user)))
}

/// Links a GitHub account to the currently authenticated user (`POST /auth/me/connect/github`).
///
/// # Arguments
/// * `auth` - Authenticated caller identity.
/// * `payload` - JSON body containing the GitHub OAuth `code`.
/// * `repo` - Database repository handle.
/// * `github` - GitHub API client handle.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with updated [`UserProfileResponse`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `payload.code` is empty or OAuth exchange fails.
/// * [`CddGatewayError`] - On GitHub API or database update failure.
#[utoipa::path(
    post,
    path = "/auth/me/connect/github",
    request_body = OAuthPayload,
    responses(
        (status = 200, description = "GitHub account linked", body = UserProfileResponse),
        (status = 400, description = "Validation or OAuth error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn connect_github(
    auth: AuthenticatedUser,
    payload: web::Json<OAuthPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    github: web::Data<Arc<dyn GitHubClient>>,
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

    let updated = repo.link_github_id(auth.user_id, gh_user.id).await?;
    Ok(HttpResponse::Ok().json(UserProfileResponse::from(updated)))
}

/// Links a Google account to the currently authenticated user (`POST /auth/me/connect/google`).
///
/// # Arguments
/// * `auth` - Authenticated caller identity.
/// * `payload` - JSON body containing the Google OAuth `code` and optional `redirect_uri`.
/// * `repo` - Database repository handle.
/// * `cfg` - Application configuration containing Google OAuth credentials and URLs.
/// * `http` - Optional shared `reqwest::Client`.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with updated [`UserProfileResponse`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If code exchange fails or email is unverified.
/// * [`CddGatewayError`] - On HTTP or database update failure.
#[utoipa::path(
    post,
    path = "/auth/me/connect/google",
    request_body = GoogleOAuthPayload,
    responses(
        (status = 200, description = "Google account linked", body = UserProfileResponse),
        (status = 400, description = "Validation or OAuth error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn connect_google(
    auth: AuthenticatedUser,
    payload: web::Json<GoogleOAuthPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
    cfg: web::Data<AppConfig>,
    http: Option<web::Data<Client>>,
) -> Result<HttpResponse, CddGatewayError> {
    let default_client = Client::new();
    let client_ref: &Client = http.as_ref().map_or(&default_client, |c| c.get_ref());

    let info = fetch_google_user_info(client_ref, &cfg, &payload).await?;
    let updated = repo.link_google_id(auth.user_id, info.sub).await?;
    Ok(HttpResponse::Ok().json(UserProfileResponse::from(updated)))
}

/// Sets or updates a password on the currently authenticated account (`POST /auth/me/connect/password`).
///
/// # Arguments
/// * `auth` - Authenticated caller identity.
/// * `payload` - JSON body containing the new plaintext `password`.
/// * `repo` - Database repository handle.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with updated [`UserProfileResponse`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `payload.password` is empty.
/// * [`CddGatewayError`] - On password hashing or database update failure.
#[utoipa::path(
    post,
    path = "/auth/me/connect/password",
    request_body = ConnectPasswordPayload,
    responses(
        (status = 200, description = "Password configured", body = UserProfileResponse),
        (status = 400, description = "Invalid password")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn connect_password(
    auth: AuthenticatedUser,
    payload: web::Json<ConnectPasswordPayload>,
    repo: web::Data<Arc<dyn CddRepository>>,
) -> Result<HttpResponse, CddGatewayError> {
    if payload.password.trim().is_empty() {
        return Err(CddGatewayError::Validation(
            "Password cannot be empty".to_string(),
        ));
    }

    let hashed = hash_password(&payload.password)?;
    let updated = repo.set_password_hash(auth.user_id, hashed).await?;
    Ok(HttpResponse::Ok().json(UserProfileResponse::from(updated)))
}

/// Unlinks an authentication provider (`DELETE /auth/me/connect/{provider}`).
///
/// Supports `"github"`, `"google"`, or `"password"` and refuses if it is the caller's sole method.
///
/// # Arguments
/// * `auth` - Authenticated caller identity.
/// * `path` - Path parameter specifying the provider to unlink (`"github"`, `"google"`, or `"password"`).
/// * `repo` - Database repository handle.
///
/// # Returns
/// * `Ok(HttpResponse)` - `200 OK` with updated [`UserProfileResponse`] JSON body.
///
/// # Errors
/// * [`CddGatewayError::Validation`] - If `provider` is invalid or is the caller's last login method.
/// * [`CddGatewayError`] - On database update failure.
#[utoipa::path(
    delete,
    path = "/auth/me/connect/{provider}",
    params(
        ("provider" = String, Path, description = "Provider to unlink (github, google, password)")
    ),
    responses(
        (status = 200, description = "Provider unlinked", body = UserProfileResponse),
        (status = 400, description = "Cannot unlink sole login provider or unknown provider")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn disconnect_provider(
    auth: AuthenticatedUser,
    path: web::Path<String>,
    repo: web::Data<Arc<dyn CddRepository>>,
) -> Result<HttpResponse, CddGatewayError> {
    let provider = path.into_inner();
    let updated = repo.unlink_provider(auth.user_id, provider).await?;
    Ok(HttpResponse::Ok().json(UserProfileResponse::from(updated)))
}

/// Registers `/auth` scope routes on `cfg`.
///
/// # Arguments
/// * `cfg` - Actix-web service configuration builder.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/register", web::post().to(register))
            .route("/login", web::post().to(login_password))
            .route("/github", web::post().to(login_github))
            .route("/google", web::post().to(login_google))
            .route("/me", web::get().to(get_me))
            .route("/me/connect/github", web::post().to(connect_github))
            .route("/me/connect/google", web::post().to(connect_google))
            .route("/me/connect/password", web::post().to(connect_password))
            .route(
                "/me/connect/{provider}",
                web::delete().to(disconnect_provider),
            ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::User;
    use crate::db::repository::MockCddRepository;
    use crate::github::client::MockGitHubClient;
    use actix_web::{test, App};
    use httpmock::prelude::*;
    use httpmock::MockServer;

    fn test_config() -> AppConfig {
        AppConfig::default()
    }

    fn generate_test_token() -> String {
        crate::api::auth_middleware::generate_test_token()
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
                google_id: None,
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

        let req_bad_email = test::TestRequest::post()
            .uri("/auth/register")
            .set_json(RegisterPayload {
                username: "valid_user".to_string(),
                email: "invalid-email".to_string(),
                password: None,
            })
            .to_request();
        let resp_bad_email = test::call_service(&app, req_bad_email).await;
        assert_eq!(
            resp_bad_email.status(),
            actix_web::http::StatusCode::BAD_REQUEST
        );
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
                    google_id: None,
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
        assert_eq!(test::call_service(&app, req_ok).await.status(), 201);

        let req_err = test::TestRequest::post()
            .uri("/auth/register")
            .set_json(RegisterPayload {
                username: "test_user".to_string(),
                email: "test@example.com".to_string(),
                password: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_err).await.status(), 500);
    }

    #[actix_web::test]
    async fn test_login_password_flows() -> Result<(), CddGatewayError> {
        let hashed = hash_password("secret123")?;
        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_find_user_by_username()
            .returning(move |u| {
                if u == "valid" {
                    Ok(Some(User {
                        id: 1,
                        github_id: None,
                        username: "valid".into(),
                        email: "v@e.com".into(),
                        password_hash: Some(hashed.clone()),
                        google_id: None,
                    }))
                } else if u == "no_pw" {
                    Ok(Some(User {
                        id: 2,
                        github_id: None,
                        username: "no_pw".into(),
                        email: "n@e.com".into(),
                        password_hash: None,
                        google_id: None,
                    }))
                } else if u == "db_err" {
                    Err(CddGatewayError::Database(diesel::result::Error::NotFound))
                } else {
                    Ok(None)
                }
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
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "valid".into(),
                password: Some("secret123".into()),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_ok).await.status(), 200);

        let req_email_fallback = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "valid@example.com".into(),
                password: Some("secret123".into()),
            })
            .to_request();
        assert_eq!(
            test::call_service(&app, req_email_fallback).await.status(),
            200
        );

        let req_wrong = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "valid".into(),
                password: Some("wrong".into()),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_wrong).await.status(), 401);

        let req_missing_pw = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "valid".into(),
                password: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_missing_pw).await.status(), 401);

        let req_no_hash = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "no_pw".into(),
                password: Some("secret123".into()),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_no_hash).await.status(), 401);

        let req_not_found = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "unknown".into(),
                password: Some("secret123".into()),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_not_found).await.status(), 401);

        let req_db_err = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(LoginPayload {
                username: "db_err".into(),
                password: Some("secret123".into()),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_db_err).await.status(), 500);

        Ok(())
    }

    #[actix_web::test]
    async fn test_login_github_flows() {
        use crate::github::models::{GitHubEmail, GitHubUser};

        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_find_user_by_email().returning(|email| {
            if email == "link@gh.com" {
                Ok(Some(User {
                    id: 10,
                    github_id: None,
                    username: "existing_local".into(),
                    email,
                    password_hash: Some("hash".into()),
                    google_id: None,
                }))
            } else if email == "already@gh.com" {
                Ok(Some(User {
                    id: 11,
                    github_id: Some(123),
                    username: "already_gh".into(),
                    email,
                    password_hash: None,
                    google_id: None,
                }))
            } else {
                Ok(None)
            }
        });
        mock_repo.expect_link_github_id().returning(|uid, gh_id| {
            Ok(User {
                id: uid,
                github_id: Some(gh_id),
                username: "existing_local".into(),
                email: "link@gh.com".into(),
                password_hash: Some("hash".into()),
                google_id: None,
            })
        });
        mock_repo
            .expect_upsert_user()
            .returning(|id, login, email| {
                if login == "db_err" {
                    Err(CddGatewayError::Database(diesel::result::Error::NotFound))
                } else {
                    Ok(User {
                        id: 1,
                        github_id: Some(id),
                        username: login,
                        email,
                        password_hash: None,
                        google_id: None,
                    })
                }
            });

        let mut mock_gh = MockGitHubClient::new();
        mock_gh.expect_exchange_code().returning(|code| match code {
            "bad_code" => Err(CddGatewayError::Unauthorized("bad code".into())),
            other => Ok(format!("tok_{other}")),
        });
        mock_gh.expect_get_user().returning(|tok| match tok {
            "tok_user_err" => Err(CddGatewayError::Internal("user err".into())),
            "tok_db_err" => Ok(GitHubUser {
                id: 99,
                login: "db_err".into(),
                avatar_url: String::new(),
            }),
            _ => Ok(GitHubUser {
                id: 123,
                login: "octocat".into(),
                avatar_url: String::new(),
            }),
        });
        mock_gh.expect_get_emails().returning(|tok| match tok {
            "tok_email_err" => Err(CddGatewayError::Internal("email err".into())),
            "tok_no_emails" => Ok(vec![]),
            "tok_link" => Ok(vec![GitHubEmail {
                email: "link@gh.com".into(),
                primary: true,
                verified: true,
            }]),
            "tok_already" => Ok(vec![
                GitHubEmail {
                    email: "unverified@gh.com".into(),
                    primary: false,
                    verified: false,
                },
                GitHubEmail {
                    email: "already@gh.com".into(),
                    primary: false,
                    verified: true,
                },
            ]),
            _ => Ok(vec![GitHubEmail {
                email: "octocat@github.com".into(),
                primary: true,
                verified: true,
            }]),
        });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req_empty = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload { code: "   ".into() })
            .to_request();
        assert_eq!(test::call_service(&app, req_empty).await.status(), 400);

        let req_bad = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "bad_code".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_bad).await.status(), 400);

        let req_uerr = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "user_err".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_uerr).await.status(), 500);

        let req_eerr = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "email_err".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_eerr).await.status(), 500);

        let req_ok = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "valid".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_ok).await.status(), 200);

        let req_link = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "link".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_link).await.status(), 200);

        let req_already = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "already".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_already).await.status(), 200);

        let req_no_email = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "no_emails".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_no_email).await.status(), 200);

        let req_db = test::TestRequest::post()
            .uri("/auth/github")
            .set_json(OAuthPayload {
                code: "db_err".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_db).await.status(), 500);
    }

    #[actix_web::test]
    async fn test_login_google_and_account_linking_flows() {
        use crate::github::models::GitHubUser;

        let server = MockServer::start();
        let mut cfg = test_config();
        cfg.google_token_url = format!("{}/token", server.base_url());
        cfg.google_userinfo_url = format!("{}/userinfo", server.base_url());

        let mut mock_repo = MockCddRepository::new();
        mock_repo
            .expect_find_user_by_google_id()
            .returning(|gid| match gid.as_str() {
                "sub_existing" => Ok(Some(User {
                    id: 1,
                    github_id: None,
                    username: "g_existing".into(),
                    email: "existing@google.com".into(),
                    password_hash: None,
                    google_id: Some("sub_existing".into()),
                })),
                _ => Ok(None),
            });
        mock_repo
            .expect_find_user_by_email()
            .returning(|email| match email.as_str() {
                "match@google.com" => Ok(Some(User {
                    id: 2,
                    github_id: Some(55),
                    username: "matched_user".into(),
                    email,
                    password_hash: None,
                    google_id: None,
                })),
                _ => Ok(None),
            });
        mock_repo.expect_link_google_id().returning(|uid, gid| {
            Ok(User {
                id: uid,
                github_id: Some(55),
                username: "matched_user".into(),
                email: "match@google.com".into(),
                password_hash: Some("h".into()),
                google_id: Some(gid),
            })
        });
        mock_repo
            .expect_upsert_google_user()
            .returning(|gid, uname, email| {
                Ok(User {
                    id: 3,
                    github_id: None,
                    username: uname,
                    email,
                    password_hash: None,
                    google_id: Some(gid),
                })
            });
        mock_repo.expect_find_user_by_id().returning(|_| {
            Ok(Some(User {
                id: 1,
                github_id: Some(42),
                username: "me_user".into(),
                email: "me@example.com".into(),
                password_hash: Some("pw_hash".into()),
                google_id: Some("g_42".into()),
            }))
        });
        mock_repo.expect_link_github_id().returning(|uid, gh_id| {
            Ok(User {
                id: uid,
                github_id: Some(gh_id),
                username: "me_user".into(),
                email: "me@example.com".into(),
                password_hash: None,
                google_id: None,
            })
        });
        mock_repo.expect_set_password_hash().returning(|uid, hash| {
            Ok(User {
                id: uid,
                github_id: None,
                username: "me_user".into(),
                email: "me@example.com".into(),
                password_hash: Some(hash),
                google_id: None,
            })
        });
        mock_repo
            .expect_unlink_provider()
            .returning(|uid, provider| {
                if provider == "sole" {
                    Err(CddGatewayError::Validation(
                        "Cannot unlink last method".into(),
                    ))
                } else {
                    Ok(User {
                        id: uid,
                        github_id: None,
                        username: "me_user".into(),
                        email: "me@example.com".into(),
                        password_hash: Some("pw".into()),
                        google_id: None,
                    })
                }
            });

        let mut mock_gh = MockGitHubClient::new();
        mock_gh.expect_exchange_code().returning(|code| {
            if code == "bad" {
                Err(CddGatewayError::Unauthorized("bad".into()))
            } else {
                Ok(format!("tok_{code}"))
            }
        });
        mock_gh.expect_get_user().returning(|tok| {
            if tok == "tok_uerr" {
                Err(CddGatewayError::Internal("err".into()))
            } else {
                Ok(GitHubUser {
                    id: 777,
                    login: "octo".into(),
                    avatar_url: String::new(),
                })
            }
        });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(cfg))
                .app_data(web::Data::new(Client::new()))
                .configure(configure),
        )
        .await;

        // 1. Empty code on /auth/google
        let req_empty = test::TestRequest::post()
            .uri("/auth/google")
            .set_json(GoogleOAuthPayload {
                code: " ".into(),
                redirect_uri: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_empty).await.status(), 400);

        // 2. Google token endpoint returns 400 HTTP status
        let mut m_tok_400 = server.mock(|when, then| {
            when.method(POST).path("/token");
            then.status(400);
        });
        let req_t400 = test::TestRequest::post()
            .uri("/auth/google")
            .set_json(GoogleOAuthPayload {
                code: "c1".into(),
                redirect_uri: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_t400).await.status(), 400);
        m_tok_400.delete();

        // 3. Google token endpoint returns 200 without access_token
        let mut m_tok_err = server.mock(|when, then| {
            when.method(POST).path("/token");
            then.status(200)
                .json_body(serde_json::json!({ "error": "invalid_grant" }));
        });
        let req_terr = test::TestRequest::post()
            .uri("/auth/google")
            .set_json(GoogleOAuthPayload {
                code: "c2".into(),
                redirect_uri: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_terr).await.status(), 400);
        m_tok_err.delete();

        let mut m_tok_empty = server.mock(|when, then| {
            when.method(POST).path("/token");
            then.status(200).json_body(serde_json::json!({}));
        });
        let req_tempty = test::TestRequest::post()
            .uri("/auth/google")
            .set_json(GoogleOAuthPayload {
                code: "c2b".into(),
                redirect_uri: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_tempty).await.status(), 400);
        m_tok_empty.delete();

        // 4. Google userinfo endpoint 500
        let _m_tok_ok = server.mock(|when, then| {
            when.method(POST).path("/token");
            then.status(200)
                .json_body(serde_json::json!({ "access_token": "gtok" }));
        });
        let mut m_u_500 = server.mock(|when, then| {
            when.method(GET).path("/userinfo");
            then.status(500);
        });
        let req_u500 = test::TestRequest::post()
            .uri("/auth/google")
            .set_json(GoogleOAuthPayload {
                code: "c3".into(),
                redirect_uri: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_u500).await.status(), 500);
        m_u_500.delete();

        // 5. Unverified email rejected
        let mut m_u_unv = server.mock(|when, then| {
            when.method(GET).path("/userinfo");
            then.status(200).json_body(serde_json::json!({
                "sub": "sub_1",
                "email": "unv@google.com",
                "email_verified": false
            }));
        });
        let req_unv = test::TestRequest::post()
            .uri("/auth/google")
            .set_json(GoogleOAuthPayload {
                code: "c4".into(),
                redirect_uri: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_unv).await.status(), 400);
        m_u_unv.delete();

        // 6. Existing google_id login
        let mut m_u_ex = server.mock(|when, then| {
            when.method(GET).path("/userinfo");
            then.status(200).json_body(serde_json::json!({
                "sub": "sub_existing",
                "email": "existing@google.com",
                "email_verified": true
            }));
        });
        let req_ex = test::TestRequest::post()
            .uri("/auth/google")
            .set_json(GoogleOAuthPayload {
                code: "c5".into(),
                redirect_uri: Some("http://localhost/cb".into()),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_ex).await.status(), 200);
        m_u_ex.delete();

        // 7. Verified email auto-links to existing user
        let mut m_u_match = server.mock(|when, then| {
            when.method(GET).path("/userinfo");
            then.status(200).json_body(serde_json::json!({
                "sub": "sub_match",
                "email": "match@google.com",
                "email_verified": true
            }));
        });
        let req_match = test::TestRequest::post()
            .uri("/auth/google")
            .set_json(GoogleOAuthPayload {
                code: "c6".into(),
                redirect_uri: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_match).await.status(), 200);
        m_u_match.delete();

        // 8. Brand new Google user (and email without '@' fallback)
        let mut m_u_new = server.mock(|when, then| {
            when.method(GET).path("/userinfo");
            then.status(200).json_body(serde_json::json!({
                "sub": "sub_new",
                "email": "newuser_no_at",
                "email_verified": true
            }));
        });
        let req_new = test::TestRequest::post()
            .uri("/auth/google")
            .set_json(GoogleOAuthPayload {
                code: "c7".into(),
                redirect_uri: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_new).await.status(), 200);

        // 9. GET /auth/me & account linking routes
        let auth_hdr = ("Authorization", format!("Bearer {}", generate_test_token()));

        let req_me = test::TestRequest::get()
            .uri("/auth/me")
            .insert_header(auth_hdr.clone())
            .to_request();
        let me_resp: UserProfileResponse = test::call_and_read_body_json(&app, req_me).await;
        assert_eq!(me_resp.id, 1);
        assert_eq!(me_resp.connected_providers.len(), 3);

        // connect/github
        let req_cgh_empty = test::TestRequest::post()
            .uri("/auth/me/connect/github")
            .insert_header(auth_hdr.clone())
            .set_json(OAuthPayload {
                code: String::new(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_cgh_empty).await.status(), 400);

        let req_cgh_bad = test::TestRequest::post()
            .uri("/auth/me/connect/github")
            .insert_header(auth_hdr.clone())
            .set_json(OAuthPayload { code: "bad".into() })
            .to_request();
        assert_eq!(test::call_service(&app, req_cgh_bad).await.status(), 400);

        let req_cgh_uerr = test::TestRequest::post()
            .uri("/auth/me/connect/github")
            .insert_header(auth_hdr.clone())
            .set_json(OAuthPayload {
                code: "uerr".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_cgh_uerr).await.status(), 500);

        let req_cgh_ok = test::TestRequest::post()
            .uri("/auth/me/connect/github")
            .insert_header(auth_hdr.clone())
            .set_json(OAuthPayload {
                code: "good".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_cgh_ok).await.status(), 200);

        // connect/google
        let req_cg_ok = test::TestRequest::post()
            .uri("/auth/me/connect/google")
            .insert_header(auth_hdr.clone())
            .set_json(GoogleOAuthPayload {
                code: "c7".into(),
                redirect_uri: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_cg_ok).await.status(), 200);
        m_u_new.delete();

        // connect/password
        let req_cpw_empty = test::TestRequest::post()
            .uri("/auth/me/connect/password")
            .insert_header(auth_hdr.clone())
            .set_json(ConnectPasswordPayload {
                password: "   ".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_cpw_empty).await.status(), 400);

        let req_cpw_ok = test::TestRequest::post()
            .uri("/auth/me/connect/password")
            .insert_header(auth_hdr.clone())
            .set_json(ConnectPasswordPayload {
                password: "new_secure_password".into(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req_cpw_ok).await.status(), 200);

        // disconnect provider
        let req_del_ok = test::TestRequest::delete()
            .uri("/auth/me/connect/github")
            .insert_header(auth_hdr.clone())
            .to_request();
        assert_eq!(test::call_service(&app, req_del_ok).await.status(), 200);

        let req_del_err = test::TestRequest::delete()
            .uri("/auth/me/connect/sole")
            .insert_header(auth_hdr)
            .to_request();
        assert_eq!(test::call_service(&app, req_del_err).await.status(), 400);
    }

    #[actix_web::test]
    async fn test_get_me_not_found() {
        let mut mock_repo = MockCddRepository::new();
        mock_repo.expect_find_user_by_id().returning(|_| Ok(None));
        let mock_gh = MockGitHubClient::new();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Arc::new(mock_repo) as Arc<dyn CddRepository>))
                .app_data(web::Data::new(Arc::new(mock_gh) as Arc<dyn GitHubClient>))
                .app_data(web::Data::new(test_config()))
                .configure(configure),
        )
        .await;

        let req_me = test::TestRequest::get()
            .uri("/auth/me")
            .insert_header(("Authorization", format!("Bearer {}", generate_test_token())))
            .to_request();
        assert_eq!(test::call_service(&app, req_me).await.status(), 404);
    }
}

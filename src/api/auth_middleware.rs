#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Bearer JWT authentication extractor middleware for Actix-web handlers.

use crate::config::AppConfig;
use crate::error::CddGatewayError;
use actix_web::{dev::Payload, FromRequest, HttpRequest};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use futures_util::future::{ready, Ready};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

/// Claims stored inside a signed JWT issued by `cdd-gateway`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — the numeric user primary key ID.
    pub sub: i32,
    /// Expiration time (Unix timestamp in seconds).
    pub exp: usize,
    /// The authenticated user's login username.
    pub username: String,
}

/// Actix-web extractor that validates a Bearer JWT and yields the caller's identity.
///
/// The JWT secret is read from [`AppConfig`] registered as Actix `web::Data<AppConfig>`.
/// When no `AppConfig` is present in unit tests, the extractor falls back to the
/// development secret `"super-secret-key"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedUser {
    /// The authenticated user's database ID.
    pub user_id: i32,
    /// The authenticated user's login username.
    pub username: String,
}

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    /// Extracts and verifies the `Authorization: Bearer <token>` header from an [`HttpRequest`].
    ///
    /// # Arguments
    /// * `req` - Incoming HTTP request containing headers and application state.
    /// * `payload` - Request payload stream.
    ///
    /// # Returns
    /// * `Ready<Result<AuthenticatedUser, actix_web::Error>>` - Resolved caller identity or 401 error.
    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let secret: Vec<u8> = req
            .app_data::<actix_web::web::Data<AppConfig>>()
            .map_or_else(
                || b"super-secret-key".to_vec(),
                |cfg| cfg.jwt_secret.as_bytes().to_vec(),
            );

        let auth = BearerAuth::from_request(req, payload).into_inner();

        auth.map_or_else(
            |_| {
                ready(Err(CddGatewayError::Unauthorized(
                    "Missing or invalid Authorization header".to_string(),
                )
                .into()))
            },
            |bearer| {
                let token = bearer.token();
                match decode::<Claims>(
                    token,
                    &DecodingKey::from_secret(&secret),
                    &Validation::default(),
                ) {
                    Ok(token_data) => ready(Ok(Self {
                        user_id: token_data.claims.sub,
                        username: token_data.claims.username,
                    })),
                    Err(e) => ready(Err(CddGatewayError::Jwt(e).into())),
                }
            },
        )
    }
}

#[cfg(test)]
/// Generates a test JWT token signed with the default secret key (`"super-secret-key"`).
///
/// # Returns
/// * `String` - Signed JWT token valid for 1 hour for `user_id = 1` (`"testuser"`).
#[must_use]
pub fn generate_test_token() -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let claims = Claims {
        sub: 1,
        exp: usize::try_from(now_secs.saturating_add(3_600)).unwrap_or(0),
        username: "testuser".to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(b"super-secret-key"),
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse, Responder};

    async fn dummy_handler(user: AuthenticatedUser) -> impl Responder {
        HttpResponse::Ok().body(user.username)
    }

    #[actix_web::test]
    async fn test_auth_middleware_valid_token_no_config() {
        let app = test::init_service(App::new().route("/", web::get().to(dummy_handler))).await;

        let token = generate_test_token();
        let req = test::TestRequest::get()
            .uri("/")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_auth_middleware_invalid_token() {
        let app = test::init_service(App::new().route("/", web::get().to(dummy_handler))).await;

        let req = test::TestRequest::get()
            .uri("/")
            .insert_header(("Authorization", "Bearer invalid.token.value"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_auth_middleware_no_token() {
        let app = test::init_service(App::new().route("/", web::get().to(dummy_handler))).await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_auth_middleware_with_config() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppConfig::default()))
                .route("/", web::get().to(dummy_handler)),
        )
        .await;

        let token = generate_test_token();
        let req = test::TestRequest::get()
            .uri("/")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}

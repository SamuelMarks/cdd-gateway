#![cfg(not(tarpaulin_include))]

use crate::config::AppConfig;
use actix_web::{dev::Payload, FromRequest, HttpRequest};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use futures_util::future::{ready, Ready};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

/// Claims stored inside a JWT issued by this service.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — the numeric user ID.
    pub sub: i32,
    /// Expiration time (Unix timestamp).
    pub exp: usize,
    /// The authenticated user's username.
    pub username: String,
}

/// Actix-web extractor that validates a Bearer JWT and yields the caller's identity.
///
/// The JWT secret is read from [`AppConfig`] registered as Actix app data.  When
/// no `AppConfig` is present (e.g. in unit tests that do not register it) the
/// extractor falls back to the compile-time default `"super-secret-key"`.
pub struct AuthenticatedUser {
    /// The authenticated user's database ID.
    pub user_id: i32,
    /// The authenticated user's username.
    pub username: String,
}

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        // Read the JWT secret from AppConfig stored in app data, falling back to
        // the compile-time default so that tests without AppConfig still work.
        let secret: Vec<u8> = req
            .app_data::<actix_web::web::Data<AppConfig>>()
            .map(|cfg| cfg.jwt_secret.as_bytes().to_vec())
            .unwrap_or_else(|| b"super-secret-key".to_vec());

        let auth = BearerAuth::from_request(req, payload).into_inner();

        match auth {
            Ok(bearer) => {
                let token = bearer.token();
                match decode::<Claims>(
                    token,
                    &DecodingKey::from_secret(&secret),
                    &Validation::default(),
                ) {
                    Ok(token_data) => ready(Ok(AuthenticatedUser {
                        user_id: token_data.claims.sub,
                        username: token_data.claims.username,
                    })),
                    Err(_) => ready(Err(actix_web::error::ErrorUnauthorized("Invalid token"))),
                }
            }
            Err(e) => ready(Err(e.into())),
        }
    }
}

#[cfg(test)]
/// Generate a test JWT token signed with the default secret key.
///
/// Used by test helpers across the codebase to produce a valid Bearer token
/// without needing a running server.
pub fn generate_test_token() -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    let claims = Claims {
        sub: 1,
        exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
        username: "testuser".to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(b"super-secret-key"),
    )
    .unwrap()
}

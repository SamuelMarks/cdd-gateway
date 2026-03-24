#![cfg(not(tarpaulin_include))]


use actix_web::{dev::Payload, FromRequest, HttpRequest};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use futures_util::future::{ready, Ready};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

/// Claims stored inside the JWT
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (User ID)
    pub sub: i32,
    /// Expiration time
    pub exp: usize,
    /// Username
    pub username: String,
}

/// Extractor for an authenticated user
pub struct AuthenticatedUser {
    /// The user's ID
    pub user_id: i32,
    /// The user's username
    pub username: String,
}

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        // In a real application, you'd want to store the secret securely and maybe pass it in App state.
        // For simplicity and to not block testing, we use a static secret here.
        let secret = b"super-secret-key";

        let auth = BearerAuth::from_request(req, payload).into_inner();

        match auth {
            Ok(bearer) => {
                let token = bearer.token();
                match decode::<Claims>(
                    token,
                    &DecodingKey::from_secret(secret),
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
/// Generate a test token for use in tests
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

#![allow(missing_docs)]
use derive_more::Display;
use derive_more::Error;

/// The central Error enum for cdd-ctl.
#[derive(Debug, Display, Error)]
pub enum CddError {
    #[display("I/O Error: {_0}")]
    Io(std::io::Error),

    #[display("Database Error: {_0}")]
    Database(diesel::result::Error),

    #[display("Connection Pool Error: {_0}")]
    Pool(diesel::r2d2::PoolError),

    #[display("JSON Error: {_0}")]
    Json(serde_json::Error),

    #[display("Configuration Error: {_0}")]
    #[error(ignore)]
    Config(String),

    #[display("Environment Error: {_0}")]
    Env(dotenvy::Error),

    #[display("JWT Error: {_0}")]
    Jwt(jsonwebtoken::errors::Error),

    #[display("GitHub API Error: {_0}")]
    GitHub(reqwest::Error),

    #[display("Hash Error: {_0}")]
    Hash(argon2::password_hash::Error),

    #[display("UUID Error: {_0}")]
    Uuid(uuid::Error),

    #[display("WASM Error: {_0}")]
    #[error(ignore)]
    Wasm(String),

    #[display("System Command Failed: {_0}")]
    #[error(ignore)]
    Command(String),

    #[display("Validation Error: {_0}")]
    #[error(ignore)]
    Validation(String),

    #[display("Not Found: {_0}")]
    #[error(ignore)]
    NotFound(String),

    #[display("Internal Server Error: {_0}")]
    #[error(ignore)]
    Internal(String),
}

impl actix_web::error::ResponseError for CddError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::NotFound(_) => actix_web::http::StatusCode::NOT_FOUND,
            Self::Validation(_) => actix_web::http::StatusCode::BAD_REQUEST,
            Self::Config(_) | Self::Json(_) => actix_web::http::StatusCode::BAD_REQUEST,
            _ => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<std::io::Error> for CddError {
    fn from(err: std::io::Error) -> Self {
        CddError::Io(err)
    }
}

impl From<diesel::result::Error> for CddError {
    fn from(err: diesel::result::Error) -> Self {
        CddError::Database(err)
    }
}

impl From<diesel::r2d2::PoolError> for CddError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        CddError::Pool(err)
    }
}

impl From<serde_json::Error> for CddError {
    fn from(err: serde_json::Error) -> Self {
        CddError::Json(err)
    }
}

impl From<jsonwebtoken::errors::Error> for CddError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        CddError::Jwt(err)
    }
}

impl From<reqwest::Error> for CddError {
    fn from(err: reqwest::Error) -> Self {
        CddError::GitHub(err)
    }
}

impl From<argon2::password_hash::Error> for CddError {
    fn from(err: argon2::password_hash::Error) -> Self {
        CddError::Hash(err)
    }
}

impl From<uuid::Error> for CddError {
    fn from(err: uuid::Error) -> Self {
        CddError::Uuid(err)
    }
}

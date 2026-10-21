#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Central error handling module for `cdd-gateway`.
//!
//! Defines [`CddGatewayError`], the single unified error enumeration used across all
//! database repositories, external HTTP clients, configuration loaders, and Actix-web
//! HTTP handlers.

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use derive_more::{Display, Error, From};
use serde_json::json;

/// The central error enumeration for `cdd-gateway`.
///
/// Implements [`std::error::Error`], [`std::fmt::Display`], [`From`] conversions for
/// underlying library error types, and [`actix_web::ResponseError`] for automatic HTTP
/// status code and JSON error payload rendering.
#[derive(Debug, Display, Error, From)]
pub enum CddGatewayError {
    /// Standard I/O error.
    #[display("I/O Error: {_0}")]
    Io(std::io::Error),

    /// Database query or execution error from Diesel.
    #[display("Database Error: {_0}")]
    Database(diesel::result::Error),

    /// Database connection pool error from `r2d2`.
    #[display("Connection Pool Error: {_0}")]
    Pool(diesel::r2d2::PoolError),

    /// Blocking thread-pool execution error from Actix-web.
    #[display("Blocking Task Error: {_0}")]
    Blocking(actix_web::error::BlockingError),

    /// JSON serialization or deserialization error.
    #[display("JSON Error: {_0}")]
    Json(serde_json::Error),

    /// JWT token encoding or decoding error.
    #[display("JWT Error: {_0}")]
    Jwt(jsonwebtoken::errors::Error),

    /// HTTP request error when communicating with upstream services (e.g., GitHub).
    #[display("External API Error: {_0}")]
    Http(reqwest::Error),

    /// Password hashing or verification error from Argon2.
    #[display("Hash Error: {_0}")]
    Hash(argon2::password_hash::Error),

    /// ZIP archive reading or extraction error.
    #[display("ZIP Archive Error: {_0}")]
    Zip(zip::result::ZipError),

    /// Engine orchestration or WASM execution error.
    #[display("Engine Error: {_0}")]
    Engine(cdd_engine::error::CddEngineError),

    /// Error loading, parsing, or validating application configuration.
    #[display("Configuration Error: {_0}")]
    Config(config::ConfigError),

    /// Request payload or parameter validation error (HTTP 400).
    #[display("Validation Error: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    Validation(String),

    /// Requested resource was not found (HTTP 404).
    #[display("Not Found: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    NotFound(String),

    /// Authenticated user lacks permission to perform the action (HTTP 403).
    #[display("Forbidden: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    Forbidden(String),

    /// Upstream gateway or reverse-proxy target failed (HTTP 502).
    #[display("Bad Gateway: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    BadGateway(String),

    /// Client exceeded the configured rate limit (HTTP 429).
    #[display("Too Many Requests: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    TooManyRequests(String),

    /// Unexpected internal server error (HTTP 500).
    #[display("Internal Server Error: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    Internal(String),

    /// Missing or invalid authentication credentials (HTTP 401).
    #[display("Unauthorized: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    Unauthorized(String),
}

impl ResponseError for CddGatewayError {
    /// Maps each [`CddGatewayError`] variant to its corresponding HTTP [`StatusCode`].
    ///
    /// # Arguments
    /// * `&self` - The error instance being converted into an HTTP status code.
    ///
    /// # Returns
    /// * [`StatusCode`] - The appropriate HTTP status code for the error variant.
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Validation(_) | Self::Config(_) | Self::Json(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) | Self::Jwt(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::BadGateway(_) => StatusCode::BAD_GATEWAY,
            Self::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Io(_)
            | Self::Database(_)
            | Self::Pool(_)
            | Self::Blocking(_)
            | Self::Http(_)
            | Self::Hash(_)
            | Self::Zip(_)
            | Self::Engine(_)
            | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Builds a structured JSON [`HttpResponse`] representing the error.
    ///
    /// # Arguments
    /// * `&self` - The error instance being rendered into an HTTP response.
    ///
    /// # Returns
    /// * [`HttpResponse`] - A JSON response containing `error.message` and `error.status`.
    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        HttpResponse::build(status).json(json!({
            "error": {
                "message": self.to_string(),
                "status": status.as_u16()
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            CddGatewayError::NotFound("missing".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            CddGatewayError::Validation("invalid".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            CddGatewayError::Config(config::ConfigError::Message("bad cfg".to_string()))
                .status_code(),
            StatusCode::BAD_REQUEST
        );
        if let Err(e) = serde_json::from_str::<serde_json::Value>("invalid") {
            assert_eq!(
                CddGatewayError::Json(e).status_code(),
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(
            CddGatewayError::Unauthorized("unauth".to_string()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        if let Err(e) = jsonwebtoken::decode::<serde_json::Value>(
            "bad.jwt.token",
            &jsonwebtoken::DecodingKey::from_secret(b"secret"),
            &jsonwebtoken::Validation::default(),
        ) {
            assert_eq!(
                CddGatewayError::Jwt(e).status_code(),
                StatusCode::UNAUTHORIZED
            );
        }
        assert_eq!(
            CddGatewayError::Forbidden("denied".to_string()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            CddGatewayError::BadGateway("upstream down".to_string()).status_code(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            CddGatewayError::TooManyRequests("slow down".to_string()).status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            CddGatewayError::Internal("boom".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            CddGatewayError::Io(std::io::Error::other("io")).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            CddGatewayError::Database(diesel::result::Error::NotFound).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            CddGatewayError::Hash(argon2::password_hash::Error::PasswordInvalid).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            CddGatewayError::Zip(zip::result::ZipError::FileNotFound).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            CddGatewayError::Engine(cdd_engine::error::CddEngineError::Internal("eng".into()))
                .status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_error_response() {
        let err = CddGatewayError::NotFound("something missing".to_string());
        let resp = err.error_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}

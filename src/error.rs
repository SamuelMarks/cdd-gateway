#![deny(missing_docs)]
//! Error handling module for cdd-gateway.

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use derive_more::{Display, Error, From};
use serde_json::json;

/// The central Error enum for cdd-gateway.
#[derive(Debug, Display, Error, From)]
pub enum CddGatewayError {
    /// Standard I/O error.
    #[display("I/O Error: {_0}")]
    Io(std::io::Error),

    /// Database query or connection error.
    #[display("Database Error: {_0}")]
    Database(diesel::result::Error),

    /// Database connection pool error.
    #[display("Connection Pool Error: {_0}")]
    Pool(diesel::r2d2::PoolError),

    /// JSON serialization/deserialization error.
    #[display("JSON Error: {_0}")]
    Json(serde_json::Error),

    /// JWT token generation or validation error.
    #[display("JWT Error: {_0}")]
    Jwt(jsonwebtoken::errors::Error),

    /// HTTP request error to external APIs (e.g. GitHub).
    #[display("External API Error: {_0}")]
    Http(reqwest::Error),

    /// Password hashing error.
    #[display("Hash Error: {_0}")]
    Hash(argon2::password_hash::Error),

    /// UUID parsing error.
    #[display("UUID Error: {_0}")]
    Uuid(uuid::Error),

    /// Engine orchestration error.
    #[display("Engine Error: {_0}")]
    Engine(cdd_engine::error::CddEngineError),

    /// Error loading or parsing configuration.
    #[display("Configuration Error: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    Config(String),

    /// Data validation error.
    #[display("Validation Error: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    Validation(String),

    /// Requested resource not found.
    #[display("Not Found: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    NotFound(String),

    /// Internal server error.
    #[display("Internal Server Error: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    Internal(String),

    /// Unauthorized access.
    #[display("Unauthorized: {_0}")]
    #[error(ignore)]
    #[from(ignore)]
    Unauthorized(String),
}

impl ResponseError for CddGatewayError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Validation(_) | Self::Config(_) | Self::Json(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) | Self::Jwt(_) => StatusCode::UNAUTHORIZED,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

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
            CddGatewayError::NotFound("test".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );

        assert_eq!(
            CddGatewayError::Validation("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            CddGatewayError::Config("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        if let Err(e) = serde_json::from_str::<serde_json::Value>("invalid") {
            assert_eq!(
                CddGatewayError::Json(e).status_code(),
                StatusCode::BAD_REQUEST
            );
        }

        assert_eq!(
            CddGatewayError::Unauthorized("test".to_string()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        // Using a dummy internal error to cover the fallback branch
        assert_eq!(
            CddGatewayError::Internal("test".to_string()).status_code(),
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
